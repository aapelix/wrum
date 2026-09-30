#pragma once

#include "SDL3/SDL_stdinc.h"
#include "net/proto.h"
#include <stdint.h>
#define SNAPSHOT_BUF_CAP    32
#define INTERP_DELAY_TICKS  6.0f   // ~100ms at TICK_RATE=60, SNAPSHOT_EVERY=2
#define INTERP_SMOOTH       0.1f
#define INTERP_SNAP_TICKS   20.0f  // beyond this, stop chasing smoothly and just jump

typedef struct {
    uint32_t tick;
    uint8_t  n;
    Entity   ents[MAX_CARS];
} SnapshotEntry;

typedef struct {
    SnapshotEntry items[SNAPSHOT_BUF_CAP];
    int head;
    int count;
} SnapshotBuffer;

static void snapbuf_push(SnapshotBuffer *b, const MsgSnapshot *snap) {
    if (b->count > 0) {
        SnapshotEntry *latest = &b->items[(b->head - 1 + SNAPSHOT_BUF_CAP) % SNAPSHOT_BUF_CAP];
        if (snap->server_tick <= latest->tick) return;
    }
    SnapshotEntry *e = &b->items[b->head];
    e->tick = snap->server_tick;
    e->n = snap->n;
    memcpy(e->ents, snap->ents, snap->n * sizeof(Entity));
    b->head = (b->head + 1) % SNAPSHOT_BUF_CAP;
    if (b->count < SNAPSHOT_BUF_CAP) b->count++;
}

static SnapshotEntry *snapbuf_at(SnapshotBuffer *b, int i) {
    int idx = (b->head - b->count + i + SNAPSHOT_BUF_CAP) % SNAPSHOT_BUF_CAP;
    return &b->items[idx];
}

static bool entry_find(const SnapshotEntry *e, uint8_t id, CarState *out) {
    for (int i = 0; i < e->n; i++)
        if (e->ents[i].id == id) { *out = e->ents[i].car; return true; }
    return false;
}

static float flerp(float a, float b, float t) { return a + (b - a) * t; }

static float lerp_angle(float a, float b, float t) {
    float d = b - a;
    while (d > SDL_PI_F)  d -= 2.0f * SDL_PI_F;
    while (d < -SDL_PI_F) d += 2.0f * SDL_PI_F;
    return a + d * t;
}

static void find_bracket(SnapshotBuffer *b, float render_tick,
                          SnapshotEntry **out_a, SnapshotEntry **out_c, float *out_t) {
    int n = b->count;
    if (n == 1) { *out_a = *out_c = snapbuf_at(b, 0); *out_t = 0.0f; return; }

    int hi = n - 1;
    for (int i = 0; i < n; i++)
        if ((float)snapbuf_at(b, i)->tick >= render_tick) { hi = i; break; }
    if (hi == 0) hi = 1;
    int lo = hi - 1;

    SnapshotEntry *a = snapbuf_at(b, lo), *c = snapbuf_at(b, hi);
    float t = (c->tick != a->tick)
        ? SDL_clamp((render_tick - (float)a->tick) / (float)(c->tick - a->tick), 0.0f, 1.0f)
        : 0.0f;
    *out_a = a; *out_c = c; *out_t = t;
}
