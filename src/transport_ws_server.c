#include "transport_ws_server.h"
#include "mongoose.h"
#include <stdlib.h>
#include <string.h>
#include "proto.h"

#define EVENT_QUEUE_CAP 64

typedef struct { NetEventType type; struct mg_connection *peer; uint8_t data[MAX_PACKET]; size_t len; } QueuedEvent;

typedef struct {
    Transport base;
    struct mg_mgr mgr;
    QueuedEvent queue[EVENT_QUEUE_CAP];
    int qhead, qlen;
} WsServerTransport;

static WsServerTransport *g_instance = NULL;

static void queue_push(NetEventType type, struct mg_connection *c, const void *data, size_t len) {
    WsServerTransport *t = g_instance;
    if (!t || t->qlen == EVENT_QUEUE_CAP) return;
    int idx = (t->qhead + t->qlen) % EVENT_QUEUE_CAP;
    QueuedEvent *e = &t->queue[idx];
    e->type = type; e->peer = c; e->len = 0;
    if (data && len) { e->len = len > MAX_PACKET ? MAX_PACKET : len; memcpy(e->data, data, e->len); }
    t->qlen++;
}

static void mg_event_fn(struct mg_connection *c, int ev, void *ev_data) {
    if (ev == MG_EV_HTTP_MSG) {
        struct mg_http_message *hm = (struct mg_http_message *)ev_data;
        if (mg_match(hm->uri, mg_str("/ws"), NULL)) mg_ws_upgrade(c, hm, NULL);
        else mg_http_reply(c, 404, "", "not found");
    } else if (ev == MG_EV_WS_OPEN) {
        queue_push(NET_CONNECT, c, NULL, 0);
    } else if (ev == MG_EV_WS_MSG) {
        struct mg_ws_message *wm = (struct mg_ws_message *)ev_data;
        if ((wm->flags & 0x0F) == WEBSOCKET_OP_BINARY)
            queue_push(NET_RECEIVE, c, wm->data.buf, wm->data.len);
    } else if (ev == MG_EV_CLOSE) {
        // fires even for connections that never opened a WS (fn_data still
        // NULL then); on_disconnect's existing `if (!c) return;` covers that
        queue_push(NET_DISCONNECT, c, NULL, 0);
    }
}

static bool wsserver_t_poll(Transport *tb, NetEvent *out) {
    WsServerTransport *t = (WsServerTransport *)tb;
    if (t->qlen == 0) mg_mgr_poll(&t->mgr, 0);
    if (t->qlen == 0) return false;
    QueuedEvent *e = &t->queue[t->qhead];
    t->qhead = (t->qhead + 1) % EVENT_QUEUE_CAP; t->qlen--;
    out->type = e->type; out->peer = (Peer *)e->peer;
    out->data = e->len ? e->data : NULL; out->len = e->len;
    return true;
}
static void wsserver_t_send(Transport *tb, Peer *peer, Channel ch, const uint8_t *data, size_t len) {
    (void)tb; (void)ch;
    mg_ws_send((struct mg_connection *)peer, data, len, WEBSOCKET_OP_BINARY);
}
static void wsserver_t_disconnect(Transport *tb, Peer *peer) { (void)tb; ((struct mg_connection *)peer)->is_closing = 1; }
static void wsserver_t_disconnect_after_send(Transport *tb, Peer *peer) { wsserver_t_disconnect(tb, peer); }
static void wsserver_t_destroy(Transport *tb) {
    WsServerTransport *t = (WsServerTransport *)tb;
    mg_mgr_free(&t->mgr); g_instance = NULL; free(t);
}
static void  wsserver_t_peer_set_data(Peer *peer, void *data) { ((struct mg_connection *)peer)->fn_data = data; }
static void *wsserver_t_peer_get_data(Peer *peer) { return ((struct mg_connection *)peer)->fn_data; }

static const TransportOps WS_SERVER_OPS = {
    .poll = wsserver_t_poll, .send = wsserver_t_send,
    .disconnect = wsserver_t_disconnect, .disconnect_after_send = wsserver_t_disconnect_after_send,
    .destroy = wsserver_t_destroy, .peer_set_data = wsserver_t_peer_set_data, .peer_get_data = wsserver_t_peer_get_data,
};

Transport *transport_ws_server_host(uint16_t port) {
    if (g_instance) return NULL;
    WsServerTransport *t = calloc(1, sizeof *t);
    if (!t) return NULL;
    t->base.ops = &WS_SERVER_OPS;
    mg_mgr_init(&t->mgr);
    char url[64];
    snprintf(url, sizeof url, "http://0.0.0.0:%u", (unsigned)port);
    if (!mg_http_listen(&t->mgr, url, mg_event_fn, NULL)) { mg_mgr_free(&t->mgr); free(t); return NULL; }
    g_instance = t;
    return &t->base;
}
