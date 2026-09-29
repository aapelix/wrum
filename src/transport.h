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

typedef struct {
    bool (*poll)(Transport *t, NetEvent *out);
    void (*send)(Transport *t, Peer *peer, Channel ch, const uint8_t *data, size_t len);
    void (*disconnect)(Transport *t, Peer *peer);
    void (*disconnect_after_send)(Transport *t, Peer *peer);
    void (*destroy)(Transport *t);
    void  (*peer_set_data)(Peer *peer, void *data);
    void *(*peer_get_data)(Peer *peer);
} TransportOps;

struct Transport { const TransportOps *ops; };

static inline bool transport_poll(Transport *t, NetEvent *out) { return t->ops->poll(t, out); }
static inline void transport_send(Transport *t, Peer *peer, Channel ch, const uint8_t *d, size_t n) { t->ops->send(t, peer, ch, d, n); }
static inline void transport_disconnect(Transport *t, Peer *peer) { t->ops->disconnect(t, peer); }
static inline void transport_disconnect_after_send(Transport *t, Peer *peer) { t->ops->disconnect_after_send(t, peer); }
static inline void transport_destroy(Transport *t) { t->ops->destroy(t); }
static inline void  transport_peer_set_data(Transport *t, Peer *peer, void *data) { t->ops->peer_set_data(peer, data); }
static inline void *transport_peer_get_data(Transport *t, Peer *peer) { return t->ops->peer_get_data(peer); }
