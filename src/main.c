#include "SDL3/SDL_stdinc.h"
#include "bytes.h"
#include "camera.h"
#include "interp.h"
#include "map.h"
#include "net/proto.h"
#include <math.h>
#include <stdint.h>
#include <stdio.h>
#define SDL_MAIN_USE_CALLBACKS
#include <SDL3/SDL.h>
#include <SDL3/SDL_main.h>
#include "SDL3/SDL_render.h"

#ifdef __EMSCRIPTEN__
#include "net/transport_ws.h"
#else
#include "net/transport_enet.h"
#endif

#define STB_DS_IMPLEMENTATION
#include "stb_ds.h"

#include "car.h"
#include "sim.h"

#define SERVER_HOST "127.0.0.1"
#define SERVER_PORT 7777
#define SERVER_WS_PORT 8888
#define PENDING_CAP 64

#define RECON_SNAP_DIST 100.0f
#define RECON_DECAY     12.0f

static float angle_diff(float from, float to) {
    float d = to - from;
    while (d > SDL_PI_F)  d -= 2.0f * SDL_PI_F;
    while (d < -SDL_PI_F) d += 2.0f * SDL_PI_F;
    return d;
}


typedef struct { Input items[PENDING_CAP]; int head, len; } InputRing;

static void ring_push(InputRing *r, Input in) {
    if (r->len == PENDING_CAP) { r->head = (r->head + 1) % PENDING_CAP; r->len--; }
    int idx = (r->head + r->len) % PENDING_CAP;
    r->items[idx] = in;
    r->len++;
}
static void ring_discard_upto(InputRing *r, uint32_t seq) {
    while (r->len > 0 && r->items[r->head].seq <= seq) {
        r->head = (r->head + 1) % PENDING_CAP;
        r->len--;
    }
}
static void build_input_msg(InputRing *r, MsgInput *out) {
    int n = r->len < INPUT_REDUNDANCY ? r->len : INPUT_REDUNDANCY;
    out->count = (uint8_t)n;
    int start = (r->head + r->len - n + PENDING_CAP) % PENDING_CAP;
    for (int i = 0; i < n; i++) out->inputs[i] = r->items[(start + i) % PENDING_CAP];
}

typedef struct App {
    SDL_Window *window;
    SDL_Renderer *renderer;
    SDL_Texture *canvas;

    Camera camera;
    Tile* tiles;

    Car cars[MAX_CARS];
    bool car_active[MAX_CARS];

    Transport *transport;
    Peer *server;
    bool connected;
    uint8_t my_id;
    uint32_t next_seq;
    uint32_t last_server_tick;
    InputRing pending;

    CarState prev_sim;
    float err_x, err_y, err_a;
    SnapshotBuffer snapbuf;
    float render_tick;

    Uint64 last_time;
    bool running;
    float accumulator;
} App;

static void ensure_car(App *app, uint8_t id, CarState spawn) {
    if (id >= MAX_CARS || app->car_active[id]) return;
    float deg = spawn.a * 180.0f / SDL_PI_F;
    if (!car_init(&app->cars[id], app->renderer, spawn.x, spawn.y, deg)) {
        SDL_Log("failed to init car for id %d", id);
        return;
    }
    app->cars[id].sim = spawn;
    app->car_active[id] = true;
}

static bool app_init(App *app) {
    if (!SDL_Init(SDL_INIT_VIDEO)) return false;

    app->window = SDL_CreateWindow("wrum", 1280, 720, 0);
    if (!app->window) return false;

    app->renderer = SDL_CreateRenderer(app->window, NULL);
    if (!app->renderer) return false;

    app->canvas = SDL_CreateTexture(app->renderer, SDL_PIXELFORMAT_RGBA8888,
                                    SDL_TEXTUREACCESS_TARGET, 320, 180);
    if (!app->canvas) return false;
    SDL_SetTextureScaleMode(app->canvas, SDL_SCALEMODE_NEAREST);

    app->camera = (Camera){0, 0, 0};

    #ifdef __EMSCRIPTEN__
        app->transport = transport_ws_connect(SERVER_HOST, SERVER_WS_PORT);
    #else
        app->transport = transport_enet_connect(SERVER_HOST, SERVER_PORT);
    #endif
    if (!app->transport) {
        SDL_Log("could not start connecting to %s:%d", SERVER_HOST, SERVER_PORT);
        return false;
    }

    // todo: get map from server and replace with this:
    // app->tiles = NULL;
    app->tiles = load_map("assets/maps/untitled.json");
    if (!app->tiles) return false;

    app->running = true;
    app->last_time = SDL_GetTicksNS();
    return true;
}

static Input sample_input(void) {
    const bool *k = SDL_GetKeyboardState(NULL);
    Input in = {0};
    in.steer    = (int8_t)(k[SDL_SCANCODE_D] - k[SDL_SCANCODE_A]);
    in.throttle = (int8_t)(k[SDL_SCANCODE_W] - k[SDL_SCANCODE_S]);
    return in;
}

static void client_apply_snapshot(App *app, const MsgSnapshot *snap) {
    app->last_server_tick = snap->server_tick;

    for (int i = 0; i < snap->n; i++) {
        const Entity *e = &snap->ents[i];
        if (e->id >= MAX_CARS) continue;
        ensure_car(app, e->id, e->car);

        if (e->id == app->my_id) {
            ring_discard_upto(&app->pending, snap->ack_seq);

            Car *me = &app->cars[app->my_id];
            CarState before = me->sim;

            CarState s = e->car;
            for (int j = 0; j < app->pending.len; j++) {
                int idx = (app->pending.head + j) % PENDING_CAP;
                car_step(&s, app->pending.items[idx]);
            }
            me->sim = s;

            float dx = before.x - s.x;
            float dy = before.y - s.y;
            float da = angle_diff(s.a, before.a);
            if (dx*dx + dy*dy > RECON_SNAP_DIST * RECON_SNAP_DIST) { dx = 0; dy = 0; }
            if (fabsf(da) > 1.57f) da = 0;
            app->err_x += dx;
            app->err_y += dy;
            app->err_a += da;
        }
    }

    snapbuf_push(&app->snapbuf, snap);
}
static void client_poll_network(App *app) {
    NetEvent ev;
    while (transport_poll(app->transport, &ev)) {
        switch (ev.type) {
        case NET_CONNECT:
            app->server = ev.peer;
            MsgHello hello = { .version = PROTOCOL_VERSION };
            SDL_strlcpy(hello.name, "test", sizeof hello.name);
            uint8_t buf[64];
            Writer w = w_init(buf, sizeof buf);
            w_hello(&w, &hello);
            if (w.ok) transport_send(app->transport, app->server, CH_RELIABLE, buf, w.len);
            SDL_Log("connected, sent hello");
            break;

        case NET_DISCONNECT:
            app->connected = false;
            app->server = NULL;
            SDL_Log("disconnected from server");
            break;

        case NET_RECEIVE: {
            Reader r = r_init(ev.data, ev.len);
            MsgView m;
            while (msg_next(&r, &m)) {
                if (m.type == MSG_WELCOME) {
                    MsgWelcome w;
                    if (r_welcome(&m.body, &w)) {
                        app->my_id = w.your_id;
                        ensure_car(app, w.your_id, w.spawn);
                        app->connected = true;
                        SDL_Log("connected as id %d", app->my_id);
                    }
                } else if (m.type == MSG_SNAPSHOT) {
                    MsgSnapshot snap;
                    if (r_snapshot(&m.body, &snap)) client_apply_snapshot(app, &snap);
                } else if (m.type == MSG_REJECT) {
                    MsgReject rej;
                    if (r_reject(&m.body, &rej))
                        SDL_Log("server rejected connection (reason=%d)", rej.reason);
                    app->running = false;
                }
            }
            break;
        }
        }
    }
}

#define GHOST_RESTITUTION CAR_RESTITUTION

static void client_predict_car_collisions(App *app, CarState *me_sim) {
    for (int id = 0; id < MAX_CARS; id++) {
        if (id == app->my_id || !app->car_active[id]) continue;
        Car *other = &app->cars[id];
        OBB ghost = { other->x, other->y, CAR_HALF_W, CAR_HALF_H, other->a * SDL_PI_F / 180.0f };
        car_resolve_vs_obstacle(me_sim, ghost, GHOST_RESTITUTION);
    }
}

static void client_sim_tick(App *app) {
    Input in = sample_input();
    in.seq = app->next_seq++;

    Car *me = &app->cars[app->my_id];
    app->prev_sim = me->sim;
    car_step(&me->sim, in);
    client_predict_car_collisions(app, &me->sim);

    ring_push(&app->pending, in);

    MsgInput msg = { .last_server_tick = app->last_server_tick };
    build_input_msg(&app->pending, &msg);

    uint8_t buf[MAX_PACKET];
    Writer w = w_init(buf, sizeof buf);
    w_msg_input(&w, &msg);
    if (w.ok) transport_send(app->transport, app->server, CH_UNRELIABLE, buf, w.len);
}

static void client_update_render_tick(App *app, float dt) {
    if (app->snapbuf.count == 0) return;
    SnapshotEntry *newest = snapbuf_at(&app->snapbuf, app->snapbuf.count - 1);

    app->render_tick += dt / TICK_DT;

    float target = (float)newest->tick - INTERP_DELAY_TICKS;
    float diff = target - app->render_tick;
    if (fabsf(diff) > INTERP_SNAP_TICKS) app->render_tick = target;
    else                                  app->render_tick += diff * INTERP_SMOOTH;
}
static void client_interpolate_others(App *app) {
    if (app->snapbuf.count == 0) return;

    SnapshotEntry *a, *c; float t;
    find_bracket(&app->snapbuf, app->render_tick, &a, &c, &t);

    for (int id = 0; id < MAX_CARS; id++) {
        if (id == app->my_id || !app->car_active[id]) continue;

        CarState sa, sc;
        bool have_a = entry_find(a, (uint8_t)id, &sa);
        bool have_c = entry_find(c, (uint8_t)id, &sc);

        CarState r;
        if (have_a && have_c) {
            r.x = flerp(sa.x, sc.x, t);
            r.y = flerp(sa.y, sc.y, t);
            r.a = lerp_angle(sa.a, sc.a, t);
            r.tire = lerp_angle(sa.tire, sc.tire, t);
        } else if (have_c) r = sc;
        else if (have_a) r = sa;
        else continue;

        Car *car = &app->cars[id];
        car->x = r.x; car->y = r.y;
        car->a = r.a * 180.0f / SDL_PI_F;
        car->ta = r.tire * 180.0f / SDL_PI_F;
    }
}
static void client_update_own_render(App *app, float dt) {
    Car *me = &app->cars[app->my_id];

    float alpha = SDL_clamp(app->accumulator / TICK_DT, 0.0f, 1.0f);
    float ix = flerp(app->prev_sim.x, me->sim.x, alpha);
    float iy = flerp(app->prev_sim.y, me->sim.y, alpha);
    float ia = lerp_angle(app->prev_sim.a, me->sim.a, alpha);
    float it = flerp(app->prev_sim.tire, me->sim.tire, alpha);

    float decay = expf(-RECON_DECAY * dt);
    app->err_x *= decay;
    app->err_y *= decay;
    app->err_a *= decay;

    me->x  = ix + app->err_x;
    me->y  = iy + app->err_y;
    me->a  = (ia + app->err_a) * 180.0f / SDL_PI_F;
    me->ta = it * 180.0f / SDL_PI_F;
}

static void app_update(App *app, float dt) {
    client_poll_network(app);
    if (!app->connected) return;

    app->accumulator += dt;
    if (app->accumulator > 0.25f) app->accumulator = 0.25f;
    while (app->accumulator >= TICK_DT) {
        app->accumulator -= TICK_DT;
        client_sim_tick(app);
    }

    client_update_own_render(app, dt);
    client_update_render_tick(app, dt);
    client_interpolate_others(app);

    Car *me = &app->cars[app->my_id];
    camera_update(&app->camera, (SDL_FPoint){me->x, me->y}, me->a, dt);
}
static void app_draw(App *app) {
    SDL_SetRenderTarget(app->renderer, app->canvas);
    SDL_SetRenderDrawColor(app->renderer, 20, 255, 20, 255);
    SDL_RenderClear(app->renderer);

    for (int i = 0; i < MAX_CARS; i++)
        if (app->car_active[i]) car_draw(&app->cars[i], &app->camera);

    SDL_SetRenderTarget(app->renderer, NULL);
    SDL_RenderTexture(app->renderer, app->canvas, NULL, NULL);
    SDL_RenderPresent(app->renderer);
}
static void app_event(App *app, const SDL_Event *event) {
    if (event->type == SDL_EVENT_QUIT) {
        app->running = false;
    }
}

static void app_quit(App *app) {
    arrfree(app->tiles);

    for (int i = 0; i < MAX_CARS; i++)
        if (app->car_active[i]) car_destroy(&app->cars[i]);

    if (app->transport) transport_destroy(app->transport);
    if (app->canvas) SDL_DestroyTexture(app->canvas);
    if (app->renderer) SDL_DestroyRenderer(app->renderer);
    if (app->window) SDL_DestroyWindow(app->window);
}

SDL_AppResult SDL_AppInit(void **appstate, int argc, char **argv) {
    App *app = SDL_calloc(1, sizeof(App));
    if (!app) return SDL_APP_FAILURE;

    if (!app_init(app)) {
        app_quit(app);
        SDL_free(app);
        return SDL_APP_FAILURE;
    }

    *appstate = app;
    return SDL_APP_CONTINUE;
}
SDL_AppResult SDL_AppIterate(void *appstate) {
    App *app = appstate;

    Uint64 now = SDL_GetTicksNS();
    float dt = (float)(now - app->last_time) / 1000000000.0f;
    app->last_time = now;

    app_update(app, dt);
    app_draw(app);

    return app->running ? SDL_APP_CONTINUE : SDL_APP_SUCCESS;
}
SDL_AppResult SDL_AppEvent(void *appstate, SDL_Event *event) {
    App *app = appstate;
    app_event(app, event);
    return app->running ? SDL_APP_CONTINUE : SDL_APP_SUCCESS;
}
void SDL_AppQuit(void *appstate, SDL_AppResult result) {
    App *app = appstate;
    if (app) {
        app_quit(app);
        SDL_free(app);
    }
}
