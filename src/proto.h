#pragma once

#include "bytes.h"
#include "sim.h"
#define PROTOCOL_VERSION   1
#define MAX_CARS           8
#define INPUT_REDUNDANCY   5
#define MAX_PACKET         1200

enum {
    // client -> server
    MSG_HELLO = 1,
    MSG_INPUT = 2,
    MSG_PING  = 3,
    // server -> client
    MSG_WELCOME  = 32,
    MSG_SNAPSHOT = 33,
    MSG_PONG     = 34,
    MSG_REJECT   = 35,
};

enum { ENT_CAR = 1 };
enum { BTN_HORN = 1 << 0, BTN_BOOST = 1 << 1, BTN_HANDBRAKE = 1 << 2 };
enum { REJECT_VERSION_MISMATCH = 1, REJECT_SERVER_FULL = 2 };

static void w_input(Writer *w, const Input *in) {
    w_u32(w, in->seq);
    w_u8(w, (uint8_t)in->steer);
    w_u8(w, (uint8_t)in->throttle);
    w_u8(w, in->buttons);
}
static Input r_input(Reader *r) {
    Input in;
    in.seq      = r_u32(r);
    in.steer    = (int8_t)r_u8(r);
    in.throttle = (int8_t)r_u8(r);
    in.buttons  = r_u8(r);
    return in;
}

static void w_car(Writer *w, const CarState *c) {
    w_f32(w, c->x); w_f32(w, c->y); w_f32(w, c->a);
    w_f32(w, c->speed); w_f32(w, c->tire);
}
static CarState r_car(Reader *r) {
    CarState c;
    c.x = r_f32(r); c.y = r_f32(r); c.a = r_f32(r);
    c.speed = r_f32(r); c.tire = r_f32(r);
    return c;
}

// ---------- client -> server ----------
typedef struct {
    uint8_t version;
    char    name[16];
} MsgHello;

typedef struct {
    uint32_t last_server_tick;           // newest snapshot the client has seen
    uint8_t  count;
    Input    inputs[INPUT_REDUNDANCY];   // oldest first
} MsgInput;

static void w_hello(Writer *w, const MsgHello *m) {
    size_t at = msg_begin(w, MSG_HELLO);
    w_u8(w, m->version);
    uint8_t n = (uint8_t)strnlen(m->name, sizeof m->name - 1);
    w_u8(w, n);
    for (int i = 0; i < n; i++) w_u8(w, (uint8_t)m->name[i]);
    msg_end(w, at);
}
static bool r_hello(Reader *r, MsgHello *m) {
    m->version = r_u8(r);
    uint8_t n = r_u8(r);
    if (n > sizeof m->name - 1) return false;
    for (int i = 0; i < n; i++) m->name[i] = (char)r_u8(r);
    m->name[n] = 0;
    return r->ok;
}

static void w_msg_input(Writer *w, const MsgInput *m) {
    size_t at = msg_begin(w, MSG_INPUT);
    w_u32(w, m->last_server_tick);
    w_u8(w, m->count);
    for (int i = 0; i < m->count; i++) w_input(w, &m->inputs[i]);
    msg_end(w, at);
}
static bool r_msg_input(Reader *r, MsgInput *m) {
    m->last_server_tick = r_u32(r);
    m->count = r_u8(r);
    if (m->count > INPUT_REDUNDANCY) return false;
    for (int i = 0; i < m->count; i++) m->inputs[i] = r_input(r);
    return r->ok;
}

// ---------- server -> client ----------
typedef struct {
    uint8_t  your_id;
    uint8_t  tick_rate;
    CarState spawn;
} MsgWelcome;

typedef struct { uint8_t id, type; CarState car; } Entity;

typedef struct {
    uint32_t server_tick;
    uint32_t ack_seq;          // last input of THIS client that was applied
    uint8_t  n;
    Entity   ents[MAX_CARS];
} MsgSnapshot;

typedef struct { uint8_t reason; } MsgReject;

static void w_welcome(Writer *w, const MsgWelcome *m) {
    size_t at = msg_begin(w, MSG_WELCOME);
    w_u8(w, m->your_id);
    w_u8(w, m->tick_rate);
    w_car(w, &m->spawn);
    msg_end(w, at);
}
static bool r_welcome(Reader *r, MsgWelcome *m) {
    m->your_id = r_u8(r);
    m->tick_rate = r_u8(r);
    m->spawn = r_car(r);
    return r->ok;
}

static void w_snapshot(Writer *w, const MsgSnapshot *m) {
    size_t at = msg_begin(w, MSG_SNAPSHOT);
    w_u32(w, m->server_tick);
    w_u32(w, m->ack_seq);
    w_u8(w, m->n);
    for (int i = 0; i < m->n; i++) {
        w_u8(w, m->ents[i].id);
        w_u8(w, m->ents[i].type);
        w_car(w, &m->ents[i].car);
    }
    msg_end(w, at);
}
static bool r_snapshot(Reader *r, MsgSnapshot *m) {
    m->server_tick = r_u32(r);
    m->ack_seq = r_u32(r);
    m->n = r_u8(r);
    if (m->n > MAX_CARS) return false;
    for (int i = 0; i < m->n; i++) {
        m->ents[i].id = r_u8(r);
        m->ents[i].type = r_u8(r);
        m->ents[i].car = r_car(r);
    }
    return r->ok;
}

static void w_reject(Writer *w, const MsgReject *m) {
    size_t at = msg_begin(w, MSG_REJECT);
    w_u8(w, m->reason);
    msg_end(w, at);
}
static bool r_reject(Reader *r, MsgReject *m) {
    m->reason = r_u8(r);
    return r->ok;
}
