#include "../app.h"
#include "../camera.h"
#include "../car.h"
#include "../interp.h"
#include "../level.h"
#include "../map.h"
#include "../net/proto.h"
#include "raylib.h"
#include "scene.h"
#include "../sim.h"
#include <stdlib.h>
#include "game_scene.h"
#include "menu_scene.h"
#include "general.h"

#define PENDING_CAP 64

#define RECON_SNAP_DIST 100.0f
#define RECON_DECAY     12.0f

/* -------------------------------------------------------------------------- */
/* Helpers                                                                    */
/* -------------------------------------------------------------------------- */

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

static Input sample_input(void) {
    Input in = {0};
    in.steer =
        (int8_t)(IsKeyDown(KEY_D) - IsKeyDown(KEY_A));
    in.throttle =
        (int8_t)(IsKeyDown(KEY_W) - IsKeyDown(KEY_S));
    return in;
}


/* -------------------------------------------------------------------------- */
/* App                                                                        */
/* -------------------------------------------------------------------------- */

typedef struct {
    RenderTexture2D shadow_canvas;
    GameCamera camera;
    Map *map;
    Level *level;

    Car cars[MAX_CARS];
    bool car_active[MAX_CARS];

    uint8_t my_id;
    char lobby_code[LOBBY_CODE_LEN];
    uint32_t last_server_tick;

    InputRing pending;
    CarState prev_sim;
    float err_x, err_y, err_a;

    SnapshotBuffer snapbuf;
    float render_tick;
    float accumulator;

    CarState pending_spawn;
} GameSceneData;

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
/* Sim                                                                        */
/* -------------------------------------------------------------------------- */

#define GHOST_RESTITUTION CAR_RESTITUTION
static void client_predict_car_collisions(GameSceneData *g, CarState *me_sim) {
    for (int id = 0; id < MAX_CARS; id++) {
        if (id == g->my_id || !g->car_active[id]) continue;
        Car *other = &g->cars[id];
        OBB ghost = { other->x, other->y, CAR_HALF_W, CAR_HALF_H, other->a * PI / 180.0f };
        car_resolve_vs_obstacle(me_sim, ghost, GHOST_RESTITUTION);
    }
}

static void client_sim_tick(GameSceneData *g) {
    Input in = sample_input();
    in.seq = app.net.next_seq++;

    Car *me = &g->cars[g->my_id];
    g->prev_sim = me->sim;
    car_step(&me->sim, in);
    level_resolve_car(g->level, &me->sim);
    client_predict_car_collisions(g, &me->sim);

    ring_push(&g->pending, in);

    MsgInput msg = { .last_server_tick = g->last_server_tick };
    build_input_msg(&g->pending, &msg);

    uint8_t buf[MAX_PACKET];
    Writer w = w_init(buf, sizeof buf);
    w_msg_input(&w, &msg);
    if (w.ok) transport_send(app.net.transport, app.net.server, CH_UNRELIABLE, buf, w.len);
}

/* -------------------------------------------------------------------------- */
/* Net                                                                        */
/* -------------------------------------------------------------------------- */

static void ensure_car(GameSceneData *g, uint8_t id, CarState spawn) {
    if (id >= MAX_CARS || g->car_active[id]) return;
    float deg = spawn.a * 180.0f / PI;
    if (!car_init(&g->cars[id], spawn.x, spawn.y, deg)) {
        TraceLog(LOG_ERROR, "failed to init car for id %d", id);
        return;
    }
    g->cars[id].sim = spawn;
    g->car_active[id] = true;
}

static void send_empty_msg(uint8_t type) {
    uint8_t buf[8];
    Writer w = w_init(buf, sizeof buf);
    size_t at = msg_begin(&w, type);
    msg_end(&w, at);
    if (w.ok) transport_send(app.net.transport, app.net.server, CH_RELIABLE, buf, w.len);
}
static void client_quickplay(void)    { send_empty_msg(MSG_QUICKPLAY); }
static void client_create_lobby(void) { send_empty_msg(MSG_CREATE_LOBBY); }
static void client_join_lobby(const char *code) {
    MsgJoinLobby msg = {0};
    TextCopy(msg.code, code);
    uint8_t buf[64];
    Writer w = w_init(buf, sizeof buf);
    w_join_lobby(&w, &msg);
    if (w.ok) transport_send(app.net.transport, app.net.server, CH_RELIABLE, buf, w.len);
}

static void client_apply_snapshot(GameSceneData *g, const MsgSnapshot *snap) {
    g->last_server_tick = snap->server_tick;

    for (int i = 0; i < snap->n; i++) {
        const Entity *e = &snap->ents[i];
        if (e->id >= MAX_CARS) continue;
        ensure_car(g, e->id, e->car);

        if (e->id == g->my_id) {
            ring_discard_upto(&g->pending, snap->ack_seq);

            Car *me = &g->cars[g->my_id];
            CarState before = me->sim;

            CarState s = e->car;
            for (int j = 0; j < g->pending.len; j++) {
                int idx = (g->pending.head + j) % PENDING_CAP;
                car_step(&s, g->pending.items[idx]);
                level_resolve_car(g->level, &s);
                client_predict_car_collisions(g, &s);
            }
            me->sim = s;

            float dx = before.x - s.x;
            float dy = before.y - s.y;
            float da = angle_diff(s.a, before.a);
            if (dx*dx + dy*dy > RECON_SNAP_DIST * RECON_SNAP_DIST) { dx = 0; dy = 0; }
            if (fabsf(da) > 1.57f) da = 0;
            g->err_x += dx;
            g->err_y += dy;
            g->err_a += da;
        }
    }

    snapbuf_push(&g->snapbuf, snap);
}

/* -------------------------------------------------------------------------- */
/* Rendering state                                                            */
/* -------------------------------------------------------------------------- */

static void client_update_render_tick(GameSceneData *g, float dt) {
    if (g->snapbuf.count == 0) return;
    SnapshotEntry *newest = snapbuf_at(&g->snapbuf, g->snapbuf.count - 1);

    g->render_tick += dt / TICK_DT;

    float target = (float)newest->tick - INTERP_DELAY_TICKS;
    float diff = target - g->render_tick;
    if (fabsf(diff) > INTERP_SNAP_TICKS) g->render_tick = target;
    else                                  g->render_tick += diff * INTERP_SMOOTH;
}
static void client_interpolate_others(GameSceneData *g) {
    if (g->snapbuf.count == 0) return;

    SnapshotEntry *a, *c; float t;
    find_bracket(&g->snapbuf, g->render_tick, &a, &c, &t);

    for (int id = 0; id < MAX_CARS; id++) {
        if (id == g->my_id || !g->car_active[id]) continue;

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

        Car *car = &g->cars[id];
        car->x = r.x; car->y = r.y;
        car->a = r.a * 180.0f / PI;
        car->ta = r.tire * 180.0f / PI;
    }
}
static void client_update_own_render(GameSceneData *g, float dt) {
    Car *me = &g->cars[g->my_id];

    float alpha = Clamp(g->accumulator / TICK_DT, 0.0f, 1.0f);
    float ix = flerp(g->prev_sim.x, me->sim.x, alpha);
    float iy = flerp(g->prev_sim.y, me->sim.y, alpha);
    float ia = lerp_angle(g->prev_sim.a, me->sim.a, alpha);
    float it = flerp(g->prev_sim.tire, me->sim.tire, alpha);

    float decay = expf(-RECON_DECAY * dt);
    g->err_x *= decay;
    g->err_y *= decay;
    g->err_a *= decay;

    me->x  = ix + g->err_x;
    me->y  = iy + g->err_y;
    me->a  = (ia + g->err_a) * 180.0f / PI;
    me->ta = it * 180.0f / PI;
}

/* -------------------------------------------------------------------------- */
/* Scene ops                                                                  */
/* -------------------------------------------------------------------------- */

static void game_enter(Scene *self) {
    GameSceneData *g = self->data;
    g->map = map_load("assets/maps/untitled.json");
    g->level = level_load("assets/maps/untitled.json");
    g->shadow_canvas = LoadRenderTexture(320, 180);
    ensure_car(g, g->my_id, g->pending_spawn);
}
static void game_exit(Scene *self) {
    GameSceneData *g = self->data;
    map_destroy(g->map);
    level_destroy(g->level);
    UnloadRenderTexture(g->shadow_canvas);
    for (int i = 0; i < MAX_CARS; i++) if (g->car_active[i]) car_destroy(&g->cars[i]);
    free(g);
}
static void game_update(Scene *self, float dt) {
    GameSceneData *g = self->data;
    g->accumulator += dt;
    if (g->accumulator > 0.25f) g->accumulator = 0.25f;
    while (g->accumulator >= TICK_DT) {
        g->accumulator -= TICK_DT;
        client_sim_tick(g);
    }
    client_update_own_render(g, dt);
    client_update_render_tick(g, dt);
    client_interpolate_others(g);

    Car *me = &g->cars[g->my_id];
    camera_update(&g->camera, (Vector2){me->x, me->y}, me->a, dt);

    if (IsKeyPressed(KEY_ESCAPE)) scene_push(make_pause_scene());
}
static void game_draw(Scene *self) {
    GameSceneData *g = self->data;
    BeginTextureMode(g->shadow_canvas);
        ClearBackground(BLANK);
        map_draw_shadows(g->map, &g->camera);

        for (int i = 0; i < MAX_CARS; i++) {
            if (g->car_active[i])
                car_draw_shadow(
                    &g->cars[i],
                    &g->camera
                );
        }
    EndTextureMode();

    BeginTextureMode(app.canvas);
        ClearBackground(BLACK);

        map_draw_layers(g->map, &g->camera);

        DrawTextureRec(
            g->shadow_canvas.texture,
            (Rectangle){ 0, 0, 320, -180 },
            (Vector2){ 0, 0 },
            (Color){ 255, 255, 255, 180 }
        );

        draw_n = 0;

        for (int i = 0; i < MAX_CARS; i++) {
            if (!g->car_active[i]) continue;
            Vector2 p = camera_apply_pos(&g->camera, (Vector2){ g->cars[i].x, g->cars[i].y });
            draw_push(p.y, DR_CAR, i);
        }

        for (int i = 0; i < map_prop_count(g->map); i++) {
            Vector2 w = map_prop_pos(g->map, i);
            Vector2 p = camera_apply_pos(&g->camera, w);
            draw_push(p.y, DR_PROP, i);
        }

        qsort(drawlist, draw_n, sizeof(Drawable), draw_cmp);

        for (int i = 0; i < draw_n; i++) {
            Drawable *d = &drawlist[i];
            if (d->kind == DR_CAR) car_draw(&g->cars[d->index], &g->camera);
            else                   map_draw_prop(g->map, d->index, &g->camera);
        }

        if (g->lobby_code[0]) DrawText(g->lobby_code, 20, 10, 10, WHITE);
    EndTextureMode();
}

static void game_on_message(Scene *self, uint8_t type, Reader *body) {
    GameSceneData *g = self->data;
    if (type == MSG_SNAPSHOT) {
        MsgSnapshot snap;
        if (r_snapshot(body, &snap)) client_apply_snapshot(g, &snap);
    }
}
static void game_on_disconnect(Scene *self) {
    (void)self;
    scene_switch(make_menu_scene("disconnected from server"));
}

static const SceneOps GAME_SCENE_OPS = {
    .enter = game_enter, .exit = game_exit, .update = game_update, .draw = game_draw,
    .on_message = game_on_message, .on_disconnect = game_on_disconnect,
};

Scene make_game_scene(const MsgWelcome *welcome) {
    GameSceneData *g = calloc(1, sizeof *g);
    g->my_id = welcome->your_id;
    g->pending_spawn = welcome->spawn;
    TextCopy(g->lobby_code, welcome->lobby_code);
    return (Scene){ .ops = &GAME_SCENE_OPS, .data = g, .blocks_draw = true, .blocks_update = true };
}
