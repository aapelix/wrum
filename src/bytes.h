#pragma once
#include <stddef.h>
#include <stdint.h>
#include <string.h>

typedef struct {
    uint8_t *buf;
    size_t cap, len;
    bool ok;
} Writer;
typedef struct {
    const uint8_t *buf;
    size_t len, pos;
    bool ok;
} Reader;

static inline Writer w_init(uint8_t *buf, size_t cap) { return (Writer){buf, cap, 0, true}; }
static inline Reader r_init(const uint8_t *buf, size_t len) { return (Reader){buf, len, 0, true}; }

static inline void w_u8(Writer *w, uint8_t v) {
    if (!w->ok || w->len+1 > w->cap) { w->ok = false; return; }
    w->buf[w->len++] = v;
}
static inline void w_u16(Writer *w, uint16_t v) { w_u8(w, v & 0xFF); w_u8(w, v >> 8); }
static inline void w_u32(Writer *w, uint32_t v) { w_u16(w, v & 0xFFFF); w_u16(w, v >> 16); }
static inline void w_f32(Writer *w, float f) { uint32_t u; memcpy(&u, &f, 4); w_u32(w, u); }

static inline uint8_t r_u8(Reader *r) {
    if (!r->ok || r->pos + 1 > r->len) { r->ok = false; return 0; }
    return r->buf[r->pos++];
}
static inline uint16_t r_u16(Reader *r) { uint16_t lo = r_u8(r); uint16_t hi = r_u8(r); return lo | (hi << 8); }
static inline uint32_t r_u32(Reader *r) { uint32_t lo = r_u16(r); uint32_t hi = r_u16(r); return lo | (hi << 16); }
static inline float r_f32(Reader *r) { uint32_t u = r_u32(r); float f; memcpy(&f, &u, 4); return f; }

static inline size_t msg_begin(Writer *w, uint8_t type) {
    w_u8(w, type);
    size_t at = w->len;
    w_u16(w, 0);
    return at;
}
static inline void msg_end(Writer *w, size_t at) {
    if (!w->ok) return;
    uint16_t n = (uint16_t)(w->len - at - 2);
    w->buf[at] = n & 0xFF;
    w->buf[at + 1] = n >> 8;
}

typedef struct {
    uint8_t type;
    Reader body;
} MsgView;

static inline bool msg_next(Reader *r, MsgView *m) {
    if (!r->ok || r->pos >= r->len) return false;
    uint8_t type = r_u8(r);
    uint16_t n = r_u16(r);
    if (!r->ok || r->pos + n > r->len) { r->ok = false; return false; }
    m->type = type;
    m->body = (Reader){ r->buf + r->pos, n, 0, true };
    r->pos += n;
    return true;
}
