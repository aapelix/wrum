#include "bytes.h"
#include "level.h"
#include "net/proto.h"
#include "sim.h"
#include "net/transport.h"
#include "net/transport_enet.h"
#include "net/transport_ws_server.h"
#include <ctype.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <time.h>
#define STB_DS_IMPLEMENTATION
#include "stb_ds.h"

static double now_sec(void) {
    struct timespec ts;
    clock_gettime(CLOCK_MONOTONIC, &ts);
    return (double)ts.tv_sec + (double)ts.tv_nsec * 1e-9;
}
static void sleep_sec(double s) {
    struct timespec ts = { (time_t)s, (long)((s - (time_t)s) * 1e9) };
    nanosleep(&ts, NULL);
}

#define SERVER_PORT      7777
#define SERVER_WS_PORT   8888
#define SNAPSHOT_EVERY   2
#define QUEUE_CAP        8

#define MAX_LOBBIES 64
#define MAX_CLIENTS (MAX_LOBBIES * MAX_CARS)

static const char *CODE_ADJECTIVES[] = {
    "SOGGY","ANGRY","TURBO","RUSTY","SPICY","GREASY","WOBBLY","HAUNTED",
    "FERAL","CRUSTY","DIZZY","CAFFEINATED","ILLEGAL","SPARE","SCORCHED",
    "SUSPICIOUS","WRINKLED","MAJESTIC","UNLICENSED","FLAMMABLE",
};
static const char *CODE_NOUNS[] = {
    "HUBCAP","MUFFLER","BUMPER","SPOILER","JALOPY","BACKFIRE","POTHOLE",
    "DONUT","CLUTCH","GASKET","FENDER","CARBURETOR","HAIRPIN","GEARBOX",
    "WINDSHIELD","AXLE","TAILPIPE","HANDBRAKE","ODOMETER","WINGMIRROR",
};

typedef struct Lobby Lobby;

typedef struct Client {
    Transport *owner;
    Peer *peer;
    bool used;
    bool handshaked;
    char name[16];

    Lobby *lobby;
    uint8_t lobby_slot;

    Input queue[QUEUE_CAP];
    int qhead, qlen;
    Input last_input;
    bool has_seq;
    uint32_t last_seq_seen, last_seq_applied;

    CarState car;
} Client;

struct Lobby {
    bool used;
    bool is_private;
    char code[LOBBY_CODE_LEN];   // empty for public
    Client *clients[MAX_CARS];   // NULL = empty slot
    int player_count;
    uint32_t tick;
};

typedef struct Server {
    Transport *transports[2]; // [0] enet, [1] ws
    int n_transports;
    Client client_pool[MAX_CLIENTS];
    Lobby lobbies[MAX_LOBBIES];
    Level *level;
} Server;

static Client *alloc_client(Server *s) {
    for (int i = 0; i < MAX_CLIENTS; i++)
        if (!s->client_pool[i].used) return &s->client_pool[i];
    return NULL;
}
static Lobby *alloc_lobby(Server *s) {
    for (int i = 0; i < MAX_LOBBIES; i++)
        if (!s->lobbies[i].used) { s->lobbies[i] = (Lobby){0}; s->lobbies[i].used = true; return &s->lobbies[i]; }
    return NULL;
}

static bool code_in_use(Server *s, const char *code) {
    for (int i = 0; i < MAX_LOBBIES; i++)
        if (s->lobbies[i].used && s->lobbies[i].is_private &&
            strcmp(s->lobbies[i].code, code) == 0)
            return true;
    return false;
}
static void generate_lobby_code(Server *s, char *out, size_t out_size) {
    int n_adj = sizeof(CODE_ADJECTIVES) / sizeof(CODE_ADJECTIVES[0]);
    int n_noun = sizeof(CODE_NOUNS) / sizeof(CODE_NOUNS[0]);
    for (int attempt = 0; attempt < 20; attempt++) {
        snprintf(out, out_size, "%s-%s", CODE_ADJECTIVES[rand() % n_adj], CODE_NOUNS[rand() % n_noun]);
        if (!code_in_use(s, out)) return;
    }
    snprintf(out, out_size, "%s-%s-%02d", CODE_ADJECTIVES[rand() % n_adj], CODE_NOUNS[rand() % n_noun], rand() % 100);
}
static void normalize_code(char *s) {
    for (; *s; s++) *s = (char)toupper((unsigned char)*s);
}

static Lobby *find_or_create_public_lobby(Server *s) {
    Lobby *best = NULL;
    for (int i = 0; i < MAX_LOBBIES; i++) {
        Lobby *l = &s->lobbies[i];
        if (!l->used || l->is_private || l->player_count >= MAX_CARS) continue;
        if (!best || l->player_count > best->player_count) best = l;
    }
    return best ? best : alloc_lobby(s);
}
static Lobby *create_private_lobby(Server *s) {
    Lobby *l = alloc_lobby(s);
    if (!l) return NULL;
    l->is_private = true;
    generate_lobby_code(s, l->code, sizeof l->code);
    return l;
}
static Lobby *find_lobby_by_code(Server *s, const char *code) {
    for (int i = 0; i < MAX_LOBBIES; i++) {
        Lobby *l = &s->lobbies[i];
        if (l->used && l->is_private && strcmp(l->code, code) == 0) return l;
    }
    return NULL;
}

static void send_reject(Client *c, uint8_t reason) {
    MsgReject rej = { .reason = reason };
    uint8_t buf[16];
    Writer w = w_init(buf, sizeof buf);
    w_reject(&w, &rej);
    if (w.ok) transport_send(c->owner, c->peer, CH_RELIABLE, buf, w.len);
}
static bool join_client_to_lobby(Lobby *lobby, Client *c) {
    int slot = -1;
    for (int i = 0; i < MAX_CARS; i++) if (!lobby->clients[i]) { slot = i; break; }
    if (slot < 0) return false;

    lobby->clients[slot] = c;
    lobby->player_count++;
    c->lobby = lobby;
    c->lobby_slot = (uint8_t)slot;
    c->car = (CarState){ .x = 100.0f + slot * 40.0f, .y = 100.0f, .a = 0 };

    MsgWelcome w = { .your_id = (uint8_t)slot, .tick_rate = TICK_RATE, .spawn = c->car };
    if (lobby->is_private) strncpy(w.lobby_code, lobby->code, sizeof w.lobby_code - 1);
    uint8_t buf[96];
    Writer wr = w_init(buf, sizeof buf);
    w_welcome(&wr, &w);
    if (wr.ok) transport_send(c->owner, c->peer, CH_RELIABLE, buf, wr.len);

    printf("\"%s\" joined %s (slot %d)\n", c->name, lobby->is_private ? lobby->code : "a public lobby", slot);
    return true;
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

    *c = (Client){0};
    c->used = true;
    c->owner = owner;
    c->peer = peer;
    transport_peer_set_data(owner, peer, c);
}
static void on_disconnect(Server *s, Transport *owner, Peer *peer) {
    (void)s;
    Client *c = transport_peer_get_data(owner, peer);
    if (!c) return;

    if (c->lobby) {
        Lobby *l = c->lobby;
        l->clients[c->lobby_slot] = NULL;
        if (--l->player_count <= 0) {
            printf("lobby %s closed (empty)\n", l->is_private ? l->code : "(public)");
            *l = (Lobby){0};
        }
    }
    printf("\"%s\" disconnected\n", c->name);
    *c = (Client){0};
}

static void handle_hello(Server *s, Client *c, Reader *body) {
    (void)s;
    MsgHello hello;
    if (!r_hello(body, &hello)) { transport_disconnect(c->owner, c->peer); return; }

    if (hello.version != PROTOCOL_VERSION) {
        send_reject(c, REJECT_VERSION_MISMATCH);
        transport_disconnect_after_send(c->owner, c->peer);
        return;
    }

    memcpy(c->name, hello.name, sizeof c->name);
    c->handshaked = true;
}
static void handle_input(Client *c, Reader *body) {
    MsgInput in;
    if (!r_msg_input(body, &in)) return;
    for (int i = 0; i < in.count; i++) {
        Input *inp = &in.inputs[i];
        if (c->has_seq && inp->seq <= c->last_seq_seen) continue;
        queue_push(c, *inp);
        c->last_seq_seen = inp->seq;
        c->has_seq = true;
    }
}
static void handle_quickplay(Server *s, Client *c) {
    Lobby *l = find_or_create_public_lobby(s);
    if (!l || !join_client_to_lobby(l, c)) send_reject(c, REJECT_SERVER_FULL);
}
static void handle_create_lobby(Server *s, Client *c) {
    Lobby *l = create_private_lobby(s);
    if (!l || !join_client_to_lobby(l, c)) send_reject(c, REJECT_SERVER_FULL);
}
static void handle_join_lobby(Server *s, Client *c, Reader *body) {
    MsgJoinLobby msg;
    if (!r_join_lobby(body, &msg)) return;
    normalize_code(msg.code);
    Lobby *l = find_lobby_by_code(s, msg.code);
    if (!l) { send_reject(c, REJECT_LOBBY_NOT_FOUND); return; }
    if (!join_client_to_lobby(l, c)) send_reject(c, REJECT_LOBBY_FULL);
}

static void on_receive(Server *s, Transport *owner, Peer *peer, const uint8_t *data, size_t len) {
    Client *c = transport_peer_get_data(owner, peer);
    if (!c || !c->used) return;

    Reader r = r_init(data, len);
    MsgView m;
    while (msg_next(&r, &m)) {
        if (m.type == MSG_HELLO && !c->handshaked) handle_hello(s, c, &m.body);
        else if (!c->handshaked) continue;
        else if (c->lobby) { if (m.type == MSG_INPUT) handle_input(c, &m.body); }
        else if (m.type == MSG_QUICKPLAY)    handle_quickplay(s, c);
        else if (m.type == MSG_CREATE_LOBBY) handle_create_lobby(s, c);
        else if (m.type == MSG_JOIN_LOBBY)   handle_join_lobby(s, c, &m.body);
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
static void lobby_tick(Server *s, Lobby *l) {
    for (int i = 0; i < MAX_CARS; i++) {
        Client *c = l->clients[i];
        if (!c) continue;
        if (c->qlen > 0) { c->last_input = queue_pop(c); c->last_seq_applied = c->last_input.seq; }
        car_step(&c->car, c->last_input);
        level_resolve_car(s->level, &c->car);
    }
    for (int i = 0; i < MAX_CARS; i++) {
        if (!l->clients[i]) continue;
        for (int j = i + 1; j < MAX_CARS; j++)
            if (l->clients[j]) resolve_car_pair(&l->clients[i]->car, &l->clients[j]->car);
    }
    l->tick++;
}

static void lobby_send_snapshots(Lobby *l) {
    for (int i = 0; i < MAX_CARS; i++) {
        Client *self = l->clients[i];
        if (!self) continue;
        MsgSnapshot snap = { .server_tick = l->tick, .ack_seq = self->last_seq_applied, .n = 0 };
        for (int j = 0; j < MAX_CARS; j++)
            if (l->clients[j]) snap.ents[snap.n++] = (Entity){ .id = (uint8_t)j, .type = ENT_CAR, .car = l->clients[j]->car };
        uint8_t buf[MAX_PACKET];
        Writer w = w_init(buf, sizeof buf);
        w_snapshot(&w, &snap);
        if (w.ok) transport_send(self->owner, self->peer, CH_UNRELIABLE, buf, w.len);
    }
}

static void server_sim_tick(Server *s) {
    for (int i = 0; i < MAX_LOBBIES; i++) {
        Lobby *l = &s->lobbies[i];
        if (!l->used || l->player_count == 0) continue;
        lobby_tick(s, l);
        if (l->tick % SNAPSHOT_EVERY == 0) lobby_send_snapshots(l);
    }
}

int main(void) {
    Server s = {0};

    s.level = level_load("assets/maps/untitled.json");
    if (!s.level) { fprintf(stderr, "failed to load level\n"); return 1; }

    s.transports[s.n_transports++] = transport_enet_host(SERVER_PORT, MAX_CARS);

    Transport *ws = transport_ws_server_host(SERVER_WS_PORT);
    if (ws)
        s.transports[s.n_transports++] = ws;
    else
        fprintf(stderr, "warning: WS listener failed, browser clients can't connect\n");

    printf("server listening on port %d\n", SERVER_PORT);
    printf("ws server listening on port %d\n", SERVER_WS_PORT);

    double last = now_sec();
    float acc = 0.0f;

    for (;;) {
        double now = now_sec();
        float dt = now - last;
        last = now;

        acc += dt;
        if (acc > 0.25f) acc = 0.25f;

        server_poll_network(&s);

        while (acc >= TICK_DT) {
            acc -= TICK_DT;
            server_sim_tick(&s);
        }

        sleep_sec(TICK_DT / 4);
    }
}
