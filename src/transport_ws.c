#include "transport_ws.h"
#include <emscripten/websocket.h>
#include <stdlib.h>
#include <string.h>
#include <stdio.h>
#include "proto.h"

#define EVENT_QUEUE_CAP 64

typedef struct { NetEventType type; uint8_t data[MAX_PACKET]; size_t len; } QueuedEvent;

typedef struct {
    Transport base;
    EMSCRIPTEN_WEBSOCKET_T ws;
    void *peer_data;
    QueuedEvent queue[EVENT_QUEUE_CAP];
    int qhead, qlen;
} WsTransport;

static void queue_push(WsTransport *t, NetEventType type, const void *data, size_t len) {
    if (t->qlen == EVENT_QUEUE_CAP) return;
    int idx = (t->qhead + t->qlen) % EVENT_QUEUE_CAP;
    QueuedEvent *e = &t->queue[idx];
    e->type = type; e->len = 0;
    if (data && len) { e->len = len > MAX_PACKET ? MAX_PACKET : len; memcpy(e->data, data, e->len); }
    t->qlen++;
}

static EM_BOOL on_open(int ty, const EmscriptenWebSocketOpenEvent *ev, void *u) { (void)ty;(void)ev; queue_push(u, NET_CONNECT, NULL, 0); return EM_TRUE; }
static EM_BOOL on_close(int ty, const EmscriptenWebSocketCloseEvent *ev, void *u) { (void)ty;(void)ev; queue_push(u, NET_DISCONNECT, NULL, 0); return EM_TRUE; }
static EM_BOOL on_error(int ty, const EmscriptenWebSocketErrorEvent *ev, void *u) { (void)ty;(void)ev; queue_push(u, NET_DISCONNECT, NULL, 0); return EM_TRUE; }
static EM_BOOL on_message(int ty, const EmscriptenWebSocketMessageEvent *ev, void *u) {
    (void)ty;
    if (!ev->isText) queue_push(u, NET_RECEIVE, ev->data, ev->numBytes);
    return EM_TRUE;
}

static bool ws_t_poll(Transport *tb, NetEvent *out) {
    WsTransport *t = (WsTransport *)tb;
    if (t->qlen == 0) return false;
    QueuedEvent *e = &t->queue[t->qhead];
    t->qhead = (t->qhead + 1) % EVENT_QUEUE_CAP; t->qlen--;
    out->type = e->type;
    out->peer = (Peer *)t;
    out->data = e->len ? e->data : NULL;
    out->len  = e->len;
    return true;
}
static void ws_t_send(Transport *tb, Peer *peer, Channel ch, const uint8_t *data, size_t len) {
    (void)peer; (void)ch;
    emscripten_websocket_send_binary(((WsTransport *)tb)->ws, (void *)data, len);
}
static void ws_t_disconnect(Transport *tb, Peer *peer) { (void)peer; emscripten_websocket_close(((WsTransport *)tb)->ws, 1000, "bye"); }
static void ws_t_disconnect_after_send(Transport *tb, Peer *peer) { ws_t_disconnect(tb, peer); }
static void ws_t_destroy(Transport *tb) {
    WsTransport *t = (WsTransport *)tb;
    if (t->ws) { emscripten_websocket_close(t->ws, 1000, "closing"); emscripten_websocket_delete(t->ws); }
    free(t);
}
static void  ws_t_peer_set_data(Peer *peer, void *data) { ((WsTransport *)peer)->peer_data = data; }
static void *ws_t_peer_get_data(Peer *peer) { return ((WsTransport *)peer)->peer_data; }

static const TransportOps WS_OPS = {
    .poll = ws_t_poll, .send = ws_t_send,
    .disconnect = ws_t_disconnect, .disconnect_after_send = ws_t_disconnect_after_send,
    .destroy = ws_t_destroy, .peer_set_data = ws_t_peer_set_data, .peer_get_data = ws_t_peer_get_data,
};

Transport *transport_ws_connect(const char *hostname, uint16_t port) {
    if (!emscripten_websocket_is_supported()) return NULL;

    WsTransport *t = calloc(1, sizeof *t);
    if (!t) return NULL;
    t->base.ops = &WS_OPS;

    static char url[256];
    snprintf(url, sizeof url, "ws://%s:%u/ws", hostname, (unsigned)port);

    EmscriptenWebSocketCreateAttributes attr;
    emscripten_websocket_init_create_attributes(&attr);
    attr.url = url;

    t->ws = emscripten_websocket_new(&attr);
    if (t->ws <= 0) { free(t); return NULL; }

    emscripten_websocket_set_onopen_callback(t->ws, t, on_open);
    emscripten_websocket_set_onclose_callback(t->ws, t, on_close);
    emscripten_websocket_set_onerror_callback(t->ws, t, on_error);
    emscripten_websocket_set_onmessage_callback(t->ws, t, on_message);
    return &t->base;
}
