#include "transport_enet.h"
#include <stdint.h>
#include <stdlib.h>
#define ENET_IMPLEMENTATION
#include <enet.h>

typedef struct {
    Transport base;
    ENetHost *host;
    uint8_t recv_buf[1200];
} EnetTransport;

static bool g_enet_ready = false;
static bool ensure_enet(void) {
    if (g_enet_ready) return true;
    if (enet_initialize() != 0) return false;
    atexit(enet_deinitialize);
    g_enet_ready = true;
    return true;
}

static bool enet_t_poll(Transport *tb, NetEvent *out) {
    EnetTransport *t = (EnetTransport *)tb;
    ENetEvent ev;
    if (enet_host_service(t->host, &ev, 0) <= 0) return false;
    switch (ev.type) {
    case ENET_EVENT_TYPE_CONNECT:
        *out = (NetEvent){ .type = NET_CONNECT, .peer = (Peer *)ev.peer };
        return true;
    case ENET_EVENT_TYPE_DISCONNECT:
    case ENET_EVENT_TYPE_DISCONNECT_TIMEOUT:
        *out = (NetEvent){ .type = NET_DISCONNECT, .peer = (Peer *)ev.peer };
        return true;
    case ENET_EVENT_TYPE_RECEIVE: {
        size_t n = ev.packet->dataLength;
        if (n > sizeof t->recv_buf) n = sizeof t->recv_buf;
        memcpy(t->recv_buf, ev.packet->data, n);
        enet_packet_destroy(ev.packet);
        *out = (NetEvent){ .type = NET_RECEIVE, .peer = (Peer *)ev.peer, .data = t->recv_buf, .len = n };
        return true;
    }
    default: return false;
    }
}

static void enet_t_send(Transport *tb, Peer *peer, Channel ch, const uint8_t *data, size_t len) {
    (void)tb;
    ENetPacketFlag flags = (ch == CH_RELIABLE) ? ENET_PACKET_FLAG_RELIABLE : ENET_PACKET_FLAG_UNSEQUENCED;
    enet_peer_send((ENetPeer *)peer, ch, enet_packet_create(data, len, flags));
}
static void enet_t_disconnect(Transport *tb, Peer *peer) { (void)tb; enet_peer_disconnect((ENetPeer *)peer, 0); }
static void enet_t_disconnect_after_send(Transport *tb, Peer *peer) { (void)tb; enet_peer_disconnect_later((ENetPeer *)peer, 0); }
static void enet_t_destroy(Transport *tb) { EnetTransport *t = (EnetTransport *)tb; enet_host_destroy(t->host); free(t); }
static void  enet_t_peer_set_data(Peer *peer, void *data) { ((ENetPeer *)peer)->data = data; }
static void *enet_t_peer_get_data(Peer *peer) { return ((ENetPeer *)peer)->data; }

static const TransportOps ENET_OPS = {
    .poll = enet_t_poll, .send = enet_t_send,
    .disconnect = enet_t_disconnect, .disconnect_after_send = enet_t_disconnect_after_send,
    .destroy = enet_t_destroy,
    .peer_set_data = enet_t_peer_set_data, .peer_get_data = enet_t_peer_get_data,
};

Transport *transport_enet_host(uint16_t port, int max_peers) {
    if (!ensure_enet()) return NULL;
    ENetAddress addr = { .host = ENET_HOST_ANY, .port = port };
    ENetHost *host = enet_host_create(&addr, (size_t)max_peers, CH_COUNT, 0, 0);
    if (!host) return NULL;
    EnetTransport *t = calloc(1, sizeof *t);
    t->base.ops = &ENET_OPS;
    t->host = host;
    return &t->base;
}
Transport *transport_enet_connect(const char *hostname, uint16_t port) {
    if (!ensure_enet()) return NULL;
    ENetHost *host = enet_host_create(NULL, 1, CH_COUNT, 0, 0);
    if (!host) return NULL;
    ENetAddress addr;
    if (enet_address_set_host(&addr, hostname) != 0) { enet_host_destroy(host); return NULL; }
    addr.port = port;
    if (!enet_host_connect(host, &addr, CH_COUNT, 0)) { enet_host_destroy(host); return NULL; }
    EnetTransport *t = calloc(1, sizeof *t);
    t->base.ops = &ENET_OPS;
    t->host = host;
    return &t->base;
}
