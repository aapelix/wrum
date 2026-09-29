#pragma once

#include <stddef.h>
#include <stdint.h>
typedef struct Transport Transport;
typedef struct Peer Peer;

typedef enum { CH_RELIABLE = 0, CH_UNRELIABLE = 1, CH_COUNT } Channel;
typedef enum { NET_CONNECT, NET_DISCONNECT, NET_RECEIVE } NetEventType;

typedef struct {
    NetEventType type;
    Peer *peer;
    const uint8_t *data;
    size_t len;
} NetEvent;

Transport *transport_host(uint16_t port, int max_peers);
Transport *transport_connect(const char *host, uint16_t port);
void       transport_destroy(Transport *t);

bool transport_poll(Transport *t, NetEvent *out);

void transport_send(Transport *t, Peer *peer, Channel ch, const uint8_t *data, size_t len);
void transport_disconnect(Transport *t, Peer *peer);

void  transport_peer_set_data(Peer *peer, void *data);
void *transport_peer_get_data(Peer *peer);
