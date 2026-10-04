#include "bytes.h"
#include "camera.h"
#include "interp.h"
#include "level.h"
#include "map.h"
#include "net/proto.h"
#include <math.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include "raylib.h"
#include "raymath.h"

#ifdef __EMSCRIPTEN__
    #include "net/transport_ws.h"
    #include <emscripten/emscripten.h>
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
    while (d > PI)  d -= 2.0f * PI;
    while (d < -PI) d += 2.0f * PI;
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

/* -------------------------------------------------------------------------- */
/* Y-sort                                                                     */
/* -------------------------------------------------------------------------- */

typedef enum { DR_CAR, DR_PROP } DrawKind;

typedef struct {
    float key;
    DrawKind kind;
    int index;
} Drawable;

#define MAX_DRAWABLES 1024
static Drawable drawlist[MAX_DRAWABLES];
static int draw_n;

static void draw_push(float key, DrawKind kind, int index) {
    if (draw_n < MAX_DRAWABLES)
        drawlist[draw_n++] = (Drawable){ key, kind, index };
}
static int draw_cmp(const void *a, const void *b) {
    const Drawable *x = a, *y = b;
    if (x->key < y->key) return -1;
    if (x->key > y->key) return 1;
    if (x->kind != y->kind) return x->kind - y->kind;
    return x->index - y->index;
}

/* -------------------------------------------------------------------------- */
/* App                                                                        */
/* -------------------------------------------------------------------------- */

typedef struct App {
    RenderTexture2D canvas;
    RenderTexture2D shadow_canvas;

    GameCamera camera;
    Map* map;
    Level *level;

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

    bool running;
    float accumulator;
} App;

static App app;

/* -------------------------------------------------------------------------- */
/* Init                                                                       */
/* -------------------------------------------------------------------------- */

static void ensure_car(uint8_t id, CarState spawn) {
    if (id >= MAX_CARS || app.car_active[id]) return;
    float deg = spawn.a * 180.0f / PI;
    if (!car_init(&app.cars[id], spawn.x, spawn.y, deg)) {
        TraceLog(LOG_ERROR, "failed to init car for id %d", id);
        return;
    }
    app.cars[id].sim = spawn;
    app.car_active[id] = true;
}

static bool app_init(void) {
    InitWindow(1280, 720, "wrum");

    #ifdef __EMSCRIPTEN__
        app.transport = transport_ws_connect(SERVER_HOST, SERVER_WS_PORT);
    #else
        app.transport = transport_enet_connect(SERVER_HOST, SERVER_PORT);
    #endif
    if (!app.transport) {
        TraceLog(LOG_ERROR, "could not start connecting to %s:%d", SERVER_HOST, SERVER_PORT);
        return false;
    }

    app.canvas = LoadRenderTexture(320, 180);
    SetTextureFilter(app.canvas.texture, TEXTURE_FILTER_POINT);

    app.shadow_canvas = LoadRenderTexture(320, 180);

    app.camera = (GameCamera){0};

    // todo: get map from server and replace with this:
    // app->tiles = NULL;
    app.map = map_load("assets/maps/untitled.json");
    if (!app.map) return false;

    app.level = level_load("assets/maps/untitled.json");
    if (!app.level) return false;

    app.running = true;
    return true;
}

/* -------------------------------------------------------------------------- */
/* Input                                                                      */
/* -------------------------------------------------------------------------- */

static Input sample_input(void) {
    Input in = {0};
    in.steer =
        (int8_t)(IsKeyDown(KEY_D) - IsKeyDown(KEY_A));
    in.throttle =
        (int8_t)(IsKeyDown(KEY_W) - IsKeyDown(KEY_S));
    return in;
}

/* -------------------------------------------------------------------------- */
/* Sim                                                                        */
/* -------------------------------------------------------------------------- */

#define GHOST_RESTITUTION CAR_RESTITUTION

static void client_predict_car_collisions(CarState *me_sim) {
    for (int id = 0; id < MAX_CARS; id++) {
        if (id == app.my_id || !app.car_active[id]) continue;
        Car *other = &app.cars[id];
        OBB ghost = { other->x, other->y, CAR_HALF_W, CAR_HALF_H, other->a * PI / 180.0f };
        car_resolve_vs_obstacle(me_sim, ghost, GHOST_RESTITUTION);
    }
}
static void client_sim_tick(void) {
    Input in = sample_input();
    in.seq = app.next_seq++;

    Car *me = &app.cars[app.my_id];
    app.prev_sim = me->sim;
    car_step(&me->sim, in);
    level_resolve_car(app.level, &me->sim);
    client_predict_car_collisions(&me->sim);

    ring_push(&app.pending, in);

    MsgInput msg = { .last_server_tick = app.last_server_tick };
    build_input_msg(&app.pending, &msg);

    uint8_t buf[MAX_PACKET];
    Writer w = w_init(buf, sizeof buf);
    w_msg_input(&w, &msg);
    if (w.ok) transport_send(app.transport, app.server, CH_UNRELIABLE, buf, w.len);
}

/* -------------------------------------------------------------------------- */
/* Net                                                                        */
/* -------------------------------------------------------------------------- */

static void client_apply_snapshot(const MsgSnapshot *snap) {
    app.last_server_tick = snap->server_tick;

    for (int i = 0; i < snap->n; i++) {
        const Entity *e = &snap->ents[i];
        if (e->id >= MAX_CARS) continue;
        ensure_car(e->id, e->car);

        if (e->id == app.my_id) {
            ring_discard_upto(&app.pending, snap->ack_seq);

            Car *me = &app.cars[app.my_id];
            CarState before = me->sim;

            CarState s = e->car;
            for (int j = 0; j < app.pending.len; j++) {
                int idx = (app.pending.head + j) % PENDING_CAP;
                car_step(&s, app.pending.items[idx]);
                level_resolve_car(app.level, &s);
                client_predict_car_collisions(&s);
            }
            me->sim = s;

            float dx = before.x - s.x;
            float dy = before.y - s.y;
            float da = angle_diff(s.a, before.a);
            if (dx*dx + dy*dy > RECON_SNAP_DIST * RECON_SNAP_DIST) { dx = 0; dy = 0; }
            if (fabsf(da) > 1.57f) da = 0;
            app.err_x += dx;
            app.err_y += dy;
            app.err_a += da;
        }
    }

    snapbuf_push(&app.snapbuf, snap);
}
static void client_poll_network(void) {
    NetEvent ev;
    while (transport_poll(app.transport, &ev)) {
        switch (ev.type) {
        case NET_CONNECT:
            app.server = ev.peer;
            MsgHello hello = { .version = PROTOCOL_VERSION };
            TextCopy(hello.name, "test");
            uint8_t buf[64];
            Writer w = w_init(buf, sizeof buf);
            w_hello(&w, &hello);
            if (w.ok) transport_send(app.transport, app.server, CH_RELIABLE, buf, w.len);
            TraceLog(LOG_INFO, "connected, sent hello");
            break;
        case NET_DISCONNECT:
            app.connected = false;
            app.server = NULL;
            TraceLog(LOG_INFO, "disconnected from server");
            break;
        case NET_RECEIVE: {
            Reader r = r_init(ev.data, ev.len);
            MsgView m;
            while (msg_next(&r, &m)) {
                if (m.type == MSG_WELCOME) {
                    MsgWelcome w;
                    if (r_welcome(&m.body, &w)) {
                        app.my_id = w.your_id;
                        ensure_car(w.your_id, w.spawn);
                        app.connected = true;
                        TraceLog(LOG_INFO, "connected as id %d", app.my_id);
                    }
                } else if (m.type == MSG_SNAPSHOT) {
                    MsgSnapshot snap;
                    if (r_snapshot(&m.body, &snap)) client_apply_snapshot(&snap);
                } else if (m.type == MSG_REJECT) {
                    MsgReject rej;
                    if (r_reject(&m.body, &rej))
                        TraceLog(
                            LOG_ERROR,
                            "server rejected connection (reason=%d)",
                            rej.reason
                        );
                    app.running = false;
                }
            }
            break;
        }
        }
    }
}


/* -------------------------------------------------------------------------- */
/* Rendering state                                                            */
/* -------------------------------------------------------------------------- */

static void client_update_render_tick(float dt) {
    if (app.snapbuf.count == 0) return;
    SnapshotEntry *newest = snapbuf_at(&app.snapbuf, app.snapbuf.count - 1);

    app.render_tick += dt / TICK_DT;

    float target = (float)newest->tick - INTERP_DELAY_TICKS;
    float diff = target - app.render_tick;
    if (fabsf(diff) > INTERP_SNAP_TICKS) app.render_tick = target;
    else                                  app.render_tick += diff * INTERP_SMOOTH;
}
static void client_interpolate_others(void) {
    if (app.snapbuf.count == 0) return;

    SnapshotEntry *a, *c; float t;
    find_bracket(&app.snapbuf, app.render_tick, &a, &c, &t);

    for (int id = 0; id < MAX_CARS; id++) {
        if (id == app.my_id || !app.car_active[id]) continue;

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

        Car *car = &app.cars[id];
        car->x = r.x; car->y = r.y;
        car->a = r.a * 180.0f / PI;
        car->ta = r.tire * 180.0f / PI;
    }
}
static void client_update_own_render(float dt) {
    Car *me = &app.cars[app.my_id];

    float alpha = Clamp(app.accumulator / TICK_DT, 0.0f, 1.0f);
    float ix = flerp(app.prev_sim.x, me->sim.x, alpha);
    float iy = flerp(app.prev_sim.y, me->sim.y, alpha);
    float ia = lerp_angle(app.prev_sim.a, me->sim.a, alpha);
    float it = flerp(app.prev_sim.tire, me->sim.tire, alpha);

    float decay = expf(-RECON_DECAY * dt);
    app.err_x *= decay;
    app.err_y *= decay;
    app.err_a *= decay;

    me->x  = ix + app.err_x;
    me->y  = iy + app.err_y;
    me->a  = (ia + app.err_a) * 180.0f / PI;
    me->ta = it * 180.0f / PI;
}

/* -------------------------------------------------------------------------- */
/* Update                                                                     */
/* -------------------------------------------------------------------------- */

static void app_update(float dt) {
    client_poll_network();
    if (!app.connected) return;

    app.accumulator += dt;
    if (app.accumulator > 0.25f) app.accumulator = 0.25f;
    while (app.accumulator >= TICK_DT) {
        app.accumulator -= TICK_DT;
        client_sim_tick();
    }

    client_update_own_render(dt);
    client_update_render_tick(dt);
    client_interpolate_others();

    Car *me = &app.cars[app.my_id];
    camera_update(&app.camera, (Vector2){me->x, me->y}, me->a, dt);
}

/* -------------------------------------------------------------------------- */
/* Draw                                                                       */
/* -------------------------------------------------------------------------- */

static void app_draw(void) {
    BeginTextureMode(app.shadow_canvas);
        ClearBackground(BLANK);
        map_draw_shadows(app.map, &app.camera);

        for (int i = 0; i < MAX_CARS; i++) {
            if (app.car_active[i])
                car_draw_shadow(
                    &app.cars[i],
                    &app.camera
                );
        }
    EndTextureMode();

    BeginTextureMode(app.canvas);
        ClearBackground(BLACK);

        map_draw_layers(app.map, &app.camera);

        DrawTextureRec(
            app.shadow_canvas.texture,
            (Rectangle){ 0, 0, 320, -180 },
            (Vector2){ 0, 0 },
            (Color){ 255, 255, 255, 180 }
        );

        draw_n = 0;

        for (int i = 0; i < MAX_CARS; i++) {
            if (!app.car_active[i]) continue;
            Vector2 p = camera_apply_pos(&app.camera, (Vector2){ app.cars[i].x, app.cars[i].y });
            draw_push(p.y, DR_CAR, i);
        }

        for (int i = 0; i < map_prop_count(app.map); i++) {
            Vector2 w = map_prop_pos(app.map, i);
            Vector2 p = camera_apply_pos(&app.camera, w);
            // if (p.x < -64 || p.x > 384 || p.y < -64 || p.y > 244) continue;
            draw_push(p.y, DR_PROP, i);
        }

        qsort(drawlist, draw_n, sizeof(Drawable), draw_cmp);

        for (int i = 0; i < draw_n; i++) {
            Drawable *d = &drawlist[i];
            if (d->kind == DR_CAR) car_draw(&app.cars[d->index], &app.camera);
            else                   map_draw_prop(app.map, d->index, &app.camera);
        }
    EndTextureMode();

    BeginDrawing();
        ClearBackground(BLACK);
        DrawTexturePro(
            app.canvas.texture,
            (Rectangle){ 0, 0, 320, -180 },
            (Rectangle){ 0, 0, GetScreenWidth(), GetScreenHeight() },
            (Vector2){ 0, 0 },
            0,
            WHITE
        );
    EndDrawing();
}

/* -------------------------------------------------------------------------- */
/* Destroy                                                                    */
/* -------------------------------------------------------------------------- */

static void app_quit() {
    map_destroy(app.map);
    level_destroy(app.level);

    for (int i = 0; i < MAX_CARS; i++)
        if (app.car_active[i]) car_destroy(&app.cars[i]);

    UnloadRenderTexture(app.shadow_canvas);
    UnloadRenderTexture(app.canvas);

    if (app.transport) transport_destroy(app.transport);
}

/* -------------------------------------------------------------------------- */
/* Main loop                                                                  */
/* -------------------------------------------------------------------------- */

static void UpdateDrawFrame(void) {
    float dt = GetFrameTime();

    app_update(dt);
    app_draw();
}

int main(void) {
    if (!app_init()) {
        return 1;
    }

    #ifdef __EMSCRIPTEN__
        emscripten_set_main_loop(
            UpdateDrawFrame,
            0,
            1
        );
    #else
        while (!WindowShouldClose() && app.running) {
            UpdateDrawFrame();
        }
    #endif

    app_quit();
    CloseWindow();
    return 0;
}
