#include "transport_ws_server.h"
#include "mongoose.h"
#include <stdlib.h>
#include <string.h>
#include "proto.h"

#define EVENT_QUEUE_CAP 64
#define MAX_WS_PEERS    MAX_CLIENTS

typedef struct {
    struct mg_connection *c;
    void *data;
} WsPeer;

typedef struct { NetEventType type; WsPeer *peer; uint8_t data[MAX_PACKET]; size_t len; } QueuedEvent;

typedef struct {
    Transport base;
    struct mg_mgr mgr;
    WsPeer peers[MAX_WS_PEERS];
    QueuedEvent queue[EVENT_QUEUE_CAP];
    int qhead, qlen;
} WsServerTransport;

static WsServerTransport *g_instance = NULL;

static WsPeer *alloc_peer(WsServerTransport *t) {
    for (int i = 0; i < MAX_WS_PEERS; i++)
        if (!t->peers[i].c) return &t->peers[i];
    return NULL;
}

static void queue_push(NetEventType type, WsPeer *peer, const void *data, size_t len) {
    WsServerTransport *t = g_instance;
    if (!t || t->qlen == EVENT_QUEUE_CAP) return;
    int idx = (t->qhead + t->qlen) % EVENT_QUEUE_CAP;
    QueuedEvent *e = &t->queue[idx];
    e->type = type; e->peer = peer; e->len = 0;
    if (data && len) { e->len = len > MAX_PACKET ? MAX_PACKET : len; memcpy(e->data, data, e->len); }
    t->qlen++;
}

static void mg_event_fn(struct mg_connection *c, int ev, void *ev_data) {
    if (ev == MG_EV_HTTP_MSG) {
        struct mg_http_message *hm = (struct mg_http_message *)ev_data;
        if (mg_match(hm->uri, mg_str("/ws"), NULL)) {
            mg_ws_upgrade(c, hm, NULL);
            return;
        }
        if (mg_match(hm->uri, mg_str("/"), NULL)) {
            mg_http_serve_file(c, hm, "web/wrum.html", NULL);
            return;
        }
        const char *root = getenv("WRUM_WEB_ROOT");
        if (!root)
            root = "build/web-release";
        struct mg_http_serve_opts opts = { .root_dir = root };
        mg_http_serve_dir(c, hm, &opts);
    } else if (ev == MG_EV_WS_OPEN) {
        WsServerTransport *t = g_instance;
        WsPeer *peer = t ? alloc_peer(t) : NULL;
        if (!peer) { c->is_closing = 1; return; }
        peer->c = c; peer->data = NULL;
        c->fn_data = peer;
        queue_push(NET_CONNECT, peer, NULL, 0);
    } else if (ev == MG_EV_WS_MSG) {
        struct mg_ws_message *wm = (struct mg_ws_message *)ev_data;
        WsPeer *peer = (WsPeer *)c->fn_data;
        if ((wm->flags & 0x0F) == WEBSOCKET_OP_BINARY && peer)
            queue_push(NET_RECEIVE, peer, wm->data.buf, wm->data.len);
    } else if (ev == MG_EV_CLOSE) {
        WsPeer *peer = (WsPeer *)c->fn_data;
        if (peer) {
            peer->c = NULL;
            queue_push(NET_DISCONNECT, peer, NULL, 0);
        }
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
    if (e->type == NET_DISCONNECT) {
        e->peer->data = NULL;
    }
    return true;
}
static void wsserver_t_send(Transport *tb, Peer *peer, Channel ch, const uint8_t *data, size_t len) {
    (void)tb; (void)ch;
    WsPeer *p = (WsPeer *)peer;
    if (p && p->c) mg_ws_send(p->c, data, len, WEBSOCKET_OP_BINARY);
}
static void wsserver_t_disconnect(Transport *tb, Peer *peer) {
    (void)tb;
    WsPeer *p = (WsPeer *)peer;
    if (p && p->c) p->c->is_closing = 1;
}
static void wsserver_t_disconnect_after_send(Transport *tb, Peer *peer) { wsserver_t_disconnect(tb, peer); }
static void wsserver_t_destroy(Transport *tb) {
    WsServerTransport *t = (WsServerTransport *)tb;
    mg_mgr_free(&t->mgr); g_instance = NULL; free(t);
}
static void  wsserver_t_peer_set_data(Peer *peer, void *data) { ((WsPeer *)peer)->data = data; }
static void *wsserver_t_peer_get_data(Peer *peer) { return ((WsPeer *)peer)->data; }

static const TransportOps WS_SERVER_OPS = {
    .poll = wsserver_t_poll, .send = wsserver_t_send,
    .disconnect = wsserver_t_disconnect, .disconnect_after_send = wsserver_t_disconnect_after_send,
    .destroy = wsserver_t_destroy, .peer_set_data = wsserver_t_peer_set_data, .peer_get_data = wsserver_t_peer_get_data,
};

Transport *transport_ws_server_host(const char *url) {
    if (g_instance) return NULL;
    WsServerTransport *t = calloc(1, sizeof *t);
    if (!t) return NULL;
    t->base.ops = &WS_SERVER_OPS;
    mg_mgr_init(&t->mgr);
    if (!mg_http_listen(&t->mgr, url, mg_event_fn, NULL)) { mg_mgr_free(&t->mgr); free(t); return NULL; }
    g_instance = t;
    return &t->base;
}
