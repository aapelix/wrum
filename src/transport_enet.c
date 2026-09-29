#include "transport.h"
#include <stdint.h>
#include <stdlib.h>
#define ENET_IMPLEMENTATION
#include <enet.h>

struct Transport {
    ENetHost *host;
    uint8_t recv_buf[1200];
};

static bool g_enet_ready = false;
static bool ensure_enet(void) {
    if (g_enet_ready) return true;
    if (enet_initialize() != 0) return false;
    atexit(enet_deinitialize);
    g_enet_ready = true;
    return true;
}

Transport *transport_host(uint16_t port, int max_peers) {
    if (!ensure_enet()) return NULL;
    ENetAddress addr = { .host = ENET_HOST_ANY, .port = port };
    ENetHost *host = enet_host_create(&addr, (size_t)max_peers, CH_COUNT, 0, 0);
    if (!host) return NULL;
    Transport *t = calloc(1, sizeof *t);
    t->host = host;
    return t;
}

Transport *transport_connect(const char *hostname, uint16_t port) {
    if (!ensure_enet()) return NULL;
    ENetHost *host = enet_host_create(NULL, 1, CH_COUNT, 0, 0);
    if (!host) return NULL;

    ENetAddress addr;
    if (enet_address_set_host(&addr, hostname) != 0) {
        enet_host_destroy(host);
        return NULL;
    }
    addr.port = port;

    if (!enet_host_connect(host, &addr, CH_COUNT, 0)) {
        enet_host_destroy(host);
        return NULL;
    }

    Transport *t = calloc(1, sizeof *t);
    t->host = host;
    return t;
}
bool transport_poll(Transport *t, NetEvent *out) {
    ENetEvent ev;
    if (enet_host_service(t->host, &ev, 0) <= 0) return false;   // 0 = non-blocking

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
        *out = (NetEvent){ .type = NET_RECEIVE, .peer = (Peer *)ev.peer,
                            .data = t->recv_buf, .len = n };
        return true;
    }
    default:
        return false;
    }
}
void transport_send(Transport *t, Peer *peer, Channel ch, const uint8_t *data, size_t len) {
    (void)t;
    ENetPacketFlag flags = (ch == CH_RELIABLE) ? ENET_PACKET_FLAG_RELIABLE
                                                : ENET_PACKET_FLAG_UNSEQUENCED;
    ENetPacket *pkt = enet_packet_create(data, len, flags);
    enet_peer_send((ENetPeer *)peer, ch, pkt);
}
void transport_disconnect(Transport *t, Peer *peer) {
    (void)t;
    enet_peer_disconnect((ENetPeer *)peer, 0);
}

void transport_destroy(Transport *t) {
    enet_host_destroy(t->host);
    free(t);
}

void  transport_peer_set_data(Peer *peer, void *data) { ((ENetPeer *)peer)->data = data; }
void *transport_peer_get_data(Peer *peer)             { return ((ENetPeer *)peer)->data; }
