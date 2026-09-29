#include "SDL3/SDL_init.h"
#include "SDL3/SDL_timer.h"
#include "proto.h"
#include "sim.h"
#include "transport.h"
#include <stdint.h>
#include <stdio.h>

#define SERVER_PORT      7777
#define SNAPSHOT_EVERY   2
#define QUEUE_CAP        8

typedef struct Client {
    Peer *peer;
    uint8_t id;
    bool active;

    Input queue[QUEUE_CAP];
    int qhead, qlen;

    Input last_input;
    bool has_seq;
    uint32_t last_seq_seen;
    uint32_t last_seq_applied;

    CarState car;
} Client;

typedef struct Server {
    Transport *t;
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

static void on_connect(Server *s, Peer *peer) {
    Client *c = alloc_client(s);
    if (!c) { transport_disconnect(s->t, peer); return; }

    memset(c, 0, sizeof *c);
    c->active = true;
    c->peer = peer;
    c->id = (uint8_t)(c - s->clients);
    c->car = (CarState){ .x = 100.0f + c->id * 40.0f, .y = 100.0f, .a = 0 };
    transport_peer_set_data(peer, c);

    printf("client %d connected\n", c->id);

    MsgWelcome w = { .your_id = c->id, .tick_rate = TICK_RATE, .spawn = c->car };
    uint8_t buf[64];
    Writer wr = w_init(buf, sizeof buf);
    w_welcome(&wr, &w);
    if (wr.ok) transport_send(s->t, peer, CH_RELIABLE, buf, wr.len);
}

static void on_disconnect(Server *s, Peer *peer) {
    (void)s;
    Client *c = transport_peer_get_data(peer);
    if (!c) return;
    printf("client %d disconnected\n", c->id);
    c->active = false;
}

static void on_receive(Server *s, Peer *peer, const uint8_t *data, size_t len) {
    (void)s;
    Client *c = transport_peer_get_data(peer);
    if (!c || !c->active) return;

    Reader r = r_init(data, len);
    MsgView m;
    while (msg_next(&r, &m)) {
        if (m.type != MSG_INPUT) continue;
        MsgInput in;
        if (!r_msg_input(&m.body, &in)) continue;

        for (int i = 0; i < in.count; i++) {
            Input *inp = &in.inputs[i];
            if (c->has_seq && inp->seq <= c->last_seq_seen) continue;  // stale/duplicate
            queue_push(c, *inp);
            c->last_seq_seen = inp->seq;
            c->has_seq = true;
        }
    }
}

static void server_poll_network(Server *s) {
    NetEvent ev;
    while (transport_poll(s->t, &ev)) {
        switch (ev.type) {
        case NET_CONNECT:    on_connect(s, ev.peer); break;
        case NET_DISCONNECT: on_disconnect(s, ev.peer); break;
        case NET_RECEIVE:    on_receive(s, ev.peer, ev.data, ev.len); break;
        }
    }
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
    s->tick++;
}

static void server_send_snapshots(Server *s) {
    for (int i = 0; i < MAX_CARS; i++) {
        Client *self = &s->clients[i];
        if (!self->active) continue;

        MsgSnapshot snap = { .server_tick = s->tick, .ack_seq = self->last_seq_applied, .n = 0 };
        for (int j = 0; j < MAX_CARS; j++) {
            if (!s->clients[j].active) continue;
            snap.ents[snap.n++] = (Entity){
                .id = s->clients[j].id, .type = ENT_CAR, .car = s->clients[j].car
            };
        }

        uint8_t buf[MAX_PACKET];
        Writer w = w_init(buf, sizeof buf);
        w_snapshot(&w, &snap);
        if (w.ok) transport_send(s->t, self->peer, CH_UNRELIABLE, buf, w.len);
    }
}

int main(void) {
    SDL_Init(0);

    Server s = {0};
    s.t = transport_host(SERVER_PORT, MAX_CARS);
    if (!s.t) { fprintf(stderr, "failed to host on port %d\n", SERVER_PORT); return 1; }
    printf("server listening on port %d\n", SERVER_PORT);

    Uint64 last = SDL_GetTicksNS();
    float acc = 0.0f;

    for (;;) {
        Uint64 now = SDL_GetTicksNS();
        float dt = (float)(now - last) / 1000000000.0f;
        last = now;
        acc += dt;
        if (acc > 0.25f) acc = 0.25f;

        server_poll_network(&s);

        while (acc >= TICK_DT) {
            acc -= TICK_DT;
            server_sim_tick(&s);
            if (s.tick % SNAPSHOT_EVERY == 0) server_send_snapshots(&s);
        }

        SDL_DelayNS((Uint64)(TICK_DT * 1000000000.0 / 4));
    }
}
