#pragma once
#include "net/transport.h"
#include "raylib.h"

typedef struct NetContext {
    Transport *transport;
    Peer *server;
    uint32_t next_seq;
} NetContext;

typedef struct App {
    RenderTexture2D canvas;
    NetContext net;
    bool running;
} App;

extern App app;
