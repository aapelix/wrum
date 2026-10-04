#include "app.h"
#include "bytes.h"
#include "net/proto.h"
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include "raylib.h"
#include "scene/general.h"
#include "scene/scene.h"

#ifdef __EMSCRIPTEN__
    #include "net/transport_ws.h"
    #include <emscripten/emscripten.h>
#else
    #include "net/transport_enet.h"
#endif

#define STB_DS_IMPLEMENTATION
#include "stb_ds.h"

#define SERVER_PORT 7777

/* -------------------------------------------------------------------------- */
/* App                                                                        */
/* -------------------------------------------------------------------------- */

App app;

#ifdef __EMSCRIPTEN__

EM_JS(void, get_ws_url, (char *out, int max_len), {
    const protocol = location.protocol === "https:" ? "wss" : "ws";
    const url = protocol + "://" + (location.host == "localhost:8000" ? "localhost:8888" : location.host) + "/ws";
    stringToUTF8(url, out, max_len);
});

#endif

static void client_poll_network(void) {
    NetEvent ev;
    while (transport_poll(app.net.transport, &ev)) {
        switch (ev.type) {
        case NET_CONNECT: {
            app.net.server = ev.peer;
            MsgHello hello = { .version = PROTOCOL_VERSION };
            TextCopy(hello.name, "test");
            uint8_t buf[64];
            Writer w = w_init(buf, sizeof buf);
            w_hello(&w, &hello);
            if (w.ok) transport_send(app.net.transport, app.net.server, CH_RELIABLE, buf, w.len);
            scene_manager_on_connect();
            break;
        }
        case NET_DISCONNECT:
            app.net.server = NULL;
            scene_manager_on_disconnect();
            break;
        case NET_RECEIVE: {
            Reader r = r_init(ev.data, ev.len);
            MsgView m;
            while (msg_next(&r, &m)) scene_manager_on_message(m.type, &m.body);
            break;
        }
        }
    }
}
static void UpdateDrawFrame(void) {
    float dt = GetFrameTime();
    client_poll_network();
    scene_manager_update(dt);

    scene_manager_draw();

    BeginDrawing();
        ClearBackground(BLACK);
        DrawTexturePro(app.canvas.texture, (Rectangle){0, 0, 320, -180},
                        (Rectangle){0, 0, GetScreenWidth(), GetScreenHeight()},
                        (Vector2){0, 0}, 0, WHITE);
    EndDrawing();
}

int main(void) {
    InitWindow(1280, 720, "wrum");
    SetExitKey(KEY_NULL);

    app.canvas = LoadRenderTexture(320, 180);
    SetTextureFilter(app.canvas.texture, TEXTURE_FILTER_POINT);

    #ifdef __EMSCRIPTEN__
        char ws_url[256];
        get_ws_url(ws_url, sizeof ws_url);
        TraceLog(LOG_INFO, "connecting to %s", ws_url);
        app.net.transport = transport_ws_connect(ws_url);
    #else

        const char *host = getenv("HOST");
        const char *port = getenv("PORT");

        if (!host) {
            fprintf(stderr, "HOST is not set\n");
            return 1;
        }
        if (!port) {
            fprintf(stderr, "PORT is not set\n");
            return 1;
        }

        uint16_t port_number = (uint16_t)strtoul(port, NULL, 10);
        app.net.transport = transport_enet_connect(host, port_number);
    #endif
    if (!app.net.transport) {
        TraceLog(LOG_ERROR, "could not start connecting");
        UnloadRenderTexture(app.canvas);
        CloseWindow();
        return 1;
    }

    scene_manager_init();
    scene_switch(make_connecting_scene());
    app.running = true;

    #ifdef __EMSCRIPTEN__
        emscripten_set_main_loop(UpdateDrawFrame, 0, 1);
    #else
        while (!WindowShouldClose() && app.running) UpdateDrawFrame();
    #endif

    scene_manager_destroy();

    if (app.net.transport)
        transport_destroy(app.net.transport);

    UnloadRenderTexture(app.canvas);
    CloseWindow();
    return 0;
}
