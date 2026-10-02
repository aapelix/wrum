#include "bytes.h"
#include "net/proto.h"
#include "raylib.h"
#include "sim.h"
#include "net/transport.h"
#include "net/transport_enet.h"
#include "net/transport_ws_server.h"
#include <stdint.h>
#include <stdio.h>
#include <string.h>

#define SERVER_PORT      7777
#define SERVER_WS_PORT   8888
#define SNAPSHOT_EVERY   2
#define QUEUE_CAP        8

typedef struct Client {
    Transport *owner;
    Peer *peer;
    uint8_t id;
    bool used;
    bool active;
    char name[16];

    Input queue[QUEUE_CAP];
    int qhead, qlen;

    Input last_input;
    bool has_seq;
    uint32_t last_seq_seen;
    uint32_t last_seq_applied;

    CarState car;
} Client;

typedef struct Server {
    Transport *transports[2]; // [0] enet, [1] ws
    int n_transports;
    Client clients[MAX_CARS];
    uint32_t tick;
} Server;

static Client *alloc_client(Server *s) {
    for (int i = 0; i < MAX_CARS; i++)
        if (!s->clients[i].active) return &s->clients[i];
    return NULL;
}

static void queue_push(Client *c, Input in) {
    if (c->qlen >= QUEUE_CAP) return;
    int idx = (c->qhead + c->qlen) % QUEUE_CAP;
    c->queue[idx] = in;
    c->qlen++;
}

static Input queue_pop(Client *c) {
    Input in = c->queue[c->qhead];
    c->qhead = (c->qhead + 1) % QUEUE_CAP;
    c->qlen--;
    return in;
}

static void on_connect(Server *s, Transport *owner, Peer *peer) {
    Client *c = alloc_client(s);
    if (!c) { transport_disconnect(owner, peer); return; }

    memset(c, 0, sizeof *c);
    c->used = true;
    c->owner = owner;
    c->peer = peer;
    c->id = (uint8_t)(c - s->clients);
    transport_peer_set_data(owner, peer, c);
}
static void on_disconnect(Server *s, Transport *owner, Peer *peer) {
    (void)s;
    Client *c = transport_peer_get_data(owner, peer);
    if (!c) return;
    printf("client %d disconnected\n", c->id);
    c->used = false; c->active = false;
}

static void handle_hello(Server *s, Client *c, Reader *body) {
    MsgHello hello;
    if (!r_hello(body, &hello)) { transport_disconnect(c->owner, c->peer); return; }

    if (hello.version != PROTOCOL_VERSION) {
        MsgReject rej = { .reason = REJECT_VERSION_MISMATCH };
        uint8_t buf[16];
        Writer w = w_init(buf, sizeof buf);
        w_reject(&w, &rej);
        if (w.ok) transport_send(c->owner, c->peer, CH_RELIABLE, buf, w.len);
        transport_disconnect_after_send(c->owner, c->peer);
        return;
    }

    memcpy(c->name, hello.name, sizeof c->name);
    c->active = true;
    c->car = (CarState){ .x = 100.0f + c->id * 40.0f, .y = 100.0f, .a = 0 };
    printf("client %d joined as \"%s\"\n", c->id, c->name);

    MsgWelcome w = { .your_id = c->id, .tick_rate = TICK_RATE, .spawn = c->car };
    uint8_t buf[64];
    Writer wr = w_init(buf, sizeof buf);
    w_welcome(&wr, &w);
    if (wr.ok) transport_send(c->owner, c->peer, CH_RELIABLE, buf, wr.len);
}

static void on_receive(Server *s, Transport *owner, Peer *peer, const uint8_t *data, size_t len) {
    Client *c = transport_peer_get_data(owner, peer);
    if (!c || !c->used) return;

    Reader r = r_init(data, len);
    MsgView m;
    while (msg_next(&r, &m)) {
        if (m.type == MSG_HELLO && !c->active) {
            handle_hello(s, c, &m.body);
        } else if (m.type == MSG_INPUT && c->active) {
            MsgInput in;
            if (!r_msg_input(&m.body, &in)) continue;
            for (int i = 0; i < in.count; i++) {
                Input *inp = &in.inputs[i];
                if (c->has_seq && inp->seq <= c->last_seq_seen) continue;
                queue_push(c, *inp);
                c->last_seq_seen = inp->seq;
                c->has_seq = true;
            }
        }
    }
}

static void server_poll_network(Server *s) {
    for (int i = 0; i < s->n_transports; i++) {
        Transport *t = s->transports[i];
        NetEvent ev;
        while (transport_poll(t, &ev)) {
            switch (ev.type) {
            case NET_CONNECT:    on_connect(s, t, ev.peer); break;
            case NET_DISCONNECT: on_disconnect(s, t, ev.peer); break;
            case NET_RECEIVE:    on_receive(s, t, ev.peer, ev.data, ev.len); break;
            }
        }
    }
}

void resolve_car_pair(CarState *a, CarState *b) {
    CollisionBody ba = { car_obb(a), car_velocity(a), 1.0f };
    CollisionBody bb = { car_obb(b), car_velocity(b), 1.0f };

    resolve_collision(&ba, &bb, CAR_RESTITUTION);

    a->x = ba.box.x; a->y = ba.box.y;
    b->x = bb.box.x; b->y = bb.box.y;
    car_apply_velocity(a, ba.vel);
    car_apply_velocity(b, bb.vel);
}
static void server_sim_tick(Server *s) {
    for (int i = 0; i < MAX_CARS; i++) {
        Client *c = &s->clients[i];
        if (!c->active) continue;
        if (c->qlen > 0) {
            c->last_input = queue_pop(c);
            c->last_seq_applied = c->last_input.seq;
        }
        car_step(&c->car, c->last_input);
    }
    for (int i = 0; i < MAX_CARS; i++) {
        if (!s->clients[i].active) continue;
        for (int j = i + 1; j < MAX_CARS; j++) {
            if (!s->clients[j].active) continue;
            resolve_car_pair(&s->clients[i].car, &s->clients[j].car);
        }
    }
    s->tick++;
}

static void server_send_snapshots(Server *s) {
    for (int i = 0; i < MAX_CARS; i++) {
        Client *c = &s->clients[i];
        if (!c->active) continue;

        MsgSnapshot snap = { .server_tick = s->tick, .ack_seq = c->last_seq_applied, .n = 0 };
        for (int j = 0; j < MAX_CARS; j++) {
            if (!s->clients[j].active) continue;
            snap.ents[snap.n++] = (Entity){
                .id = s->clients[j].id, .type = ENT_CAR, .car = s->clients[j].car
            };
        }

        uint8_t buf[MAX_PACKET];
        Writer w = w_init(buf, sizeof buf);
        w_snapshot(&w, &snap);
        if (w.ok) transport_send(c->owner, c->peer, CH_UNRELIABLE, buf, w.len);
    }
}

int main(void) {
    Server s = {0};

    s.transports[s.n_transports++] = transport_enet_host(SERVER_PORT, MAX_CARS);

    Transport *ws = transport_ws_server_host(SERVER_WS_PORT);
    if (ws)
        s.transports[s.n_transports++] = ws;
    else
        fprintf(stderr, "warning: WS listener failed, browser clients can't connect\n");

    printf("server listening on port %d\n", SERVER_PORT);
    printf("ws server listening on port %d\n", SERVER_WS_PORT);

    double last = GetTime();
    float acc = 0.0f;

    for (;;) {
        double now = GetTime();
        float dt = now - last;
        last = now;

        acc += dt;
        if (acc > 0.25f) acc = 0.25f;

        server_poll_network(&s);

        while (acc >= TICK_DT) {
            acc -= TICK_DT;
            server_sim_tick(&s);

            if (s.tick % SNAPSHOT_EVERY == 0)
                server_send_snapshots(&s);
        }

        WaitTime(TICK_DT / 4);
    }
}
