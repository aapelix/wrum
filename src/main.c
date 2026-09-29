#include "bytes.h"
#include "camera.h"
#include "proto.h"
#include "transport.h"
#include <stdint.h>
#define SDL_MAIN_USE_CALLBACKS
#include <SDL3/SDL.h>
#include <SDL3/SDL_main.h>
#include "SDL3/SDL_render.h"

#define STB_DS_IMPLEMENTATION
#include "stb_ds.h"

#include "car.h"
#include "sim.h"

#define SERVER_HOST "127.0.0.1"
#define SERVER_PORT 7777
#define PENDING_CAP 64

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

    Car cars[MAX_CARS];
    bool car_active[MAX_CARS];

    Transport *transport;
    Peer *server;
    bool connected;
    uint8_t my_id;
    uint32_t next_seq;
    uint32_t last_server_tick;
    InputRing pending;

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

    app->transport = transport_connect(SERVER_HOST, SERVER_PORT);
    if (!app->transport) {
        SDL_Log("could not start connecting to %s:%d", SERVER_HOST, SERVER_PORT);
        return false;
    }

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

            CarState s = e->car;
            for (int j = 0; j < app->pending.len; j++) {
                int idx = (app->pending.head + j) % PENDING_CAP;
                car_step(&s, app->pending.items[idx]);
            }

            Car *me = &app->cars[app->my_id];
            me->sim = s;
            me->x = s.x; me->y = s.y; me->a = s.a * 180.0f / SDL_PI_F;
        } else {
            Car *other = &app->cars[e->id];
            other->sim = e->car;   // todo: interp
            other->x = e->car.x; other->y = e->car.y;
            other->a = e->car.a * 180.0f / SDL_PI_F;
        }
    }
}
static void client_poll_network(App *app) {
    NetEvent ev;
    while (transport_poll(app->transport, &ev)) {
        switch (ev.type) {
        case NET_CONNECT:
            app->server = ev.peer;
            SDL_Log("transport connected, waiting for welcome");
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
                }
            }
            break;
        }
        }
    }
}

static void client_sim_tick(App *app) {
    Input in = sample_input();
    in.seq = app->next_seq++;

    Car *me = &app->cars[app->my_id];
    car_step(&me->sim, in);
    me->x = me->sim.x;
    me->y = me->sim.y;
    me->a = me->sim.a * 180.0f / SDL_PI_F;
    me->ta = me->sim.tire * 180.0f / SDL_PI_F;

    ring_push(&app->pending, in);

    MsgInput msg = { .last_server_tick = app->last_server_tick };
    build_input_msg(&app->pending, &msg);

    uint8_t buf[MAX_PACKET];
    Writer w = w_init(buf, sizeof buf);
    w_msg_input(&w, &msg);
    if (w.ok) transport_send(app->transport, app->server, CH_UNRELIABLE, buf, w.len);
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
