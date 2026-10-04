#include "game_scene.h"
#include "menu_scene.h"
#include "raylib.h"
#include "scene.h"
#include <stdlib.h>
#include "../net/proto.h"
#include "../net/transport.h"
#include "../app.h"

typedef struct {
    char status[64];
    char join_code_buf[LOBBY_CODE_LEN];
} MenuSceneData;

static void send_empty_msg(uint8_t type) {
    uint8_t buf[8];
    Writer w = w_init(buf, sizeof buf);
    size_t at = msg_begin(&w, type);
    msg_end(&w, at);
    if (w.ok) transport_send(app.net.transport, app.net.server, CH_RELIABLE, buf, w.len);
}
static void send_join(const char *code) {
    MsgJoinLobby msg = {0};
    TextCopy(msg.code, code);
    uint8_t buf[64];
    Writer w = w_init(buf, sizeof buf);
    w_join_lobby(&w, &msg);
    if (w.ok) transport_send(app.net.transport, app.net.server, CH_RELIABLE, buf, w.len);
}

static void menu_exit(Scene *self) { free(self->data); }
static void menu_update(Scene *self, float dt) {
    (void)dt;
    MenuSceneData *m = self->data;
    if (!app.net.server) return;

    Rectangle quick = {20*4, 50*4, 120*4, 16*4}, create = {20*4, 68*4, 120*4, 16*4}, join = {20*4, 86*4, 120*4, 16*4};
    Vector2 mouse = GetMousePosition();
    if (CheckCollisionPointRec(mouse, quick) && IsMouseButtonPressed(MOUSE_BUTTON_LEFT)) send_empty_msg(MSG_QUICKPLAY);
    if (CheckCollisionPointRec(mouse, create) && IsMouseButtonPressed(MOUSE_BUTTON_LEFT)) send_empty_msg(MSG_CREATE_LOBBY);
    if (CheckCollisionPointRec(mouse, join) && IsMouseButtonPressed(MOUSE_BUTTON_LEFT) && m->join_code_buf[0]) send_join(m->join_code_buf);

    int key = GetCharPressed();
    while (key > 0) {
        size_t len = strlen(m->join_code_buf);
        if (key >= 32 && key <= 125 && len < sizeof(m->join_code_buf) - 1) m->join_code_buf[len] = (char)key;
        key = GetCharPressed();
    }
    if (IsKeyPressed(KEY_BACKSPACE)) {
        size_t len = strlen(m->join_code_buf);
        if (len > 0) m->join_code_buf[len - 1] = 0;
    }
}
static void menu_draw(Scene *self) {
    BeginTextureMode(app.canvas);
        ClearBackground(BLACK);
        MenuSceneData *m = self->data;
        DrawText("wrum", 20, 10, 20, WHITE);
        if (m->status[0]) DrawText(m->status, 20, 30, 10, RED);
        DrawRectangleLines(20, 50, 120, 16, WHITE); DrawText("Quickplay", 28, 52, 8, WHITE);
        DrawRectangleLines(20, 68, 120, 16, WHITE); DrawText("Create private", 28, 70, 8, WHITE);
        DrawRectangleLines(20, 86, 120, 16, WHITE); DrawText("Join", 28, 88, 8, WHITE);
        DrawText(m->join_code_buf, 150, 88, 8, WHITE);
    EndTextureMode();
}

static void menu_on_message(Scene *self, uint8_t type, Reader *body) {
    MenuSceneData *m = self->data;
    if (type == MSG_WELCOME) {
        MsgWelcome w;
        if (r_welcome(body, &w)) scene_switch(make_game_scene(&w));
    } else if (type == MSG_REJECT) {
        MsgReject rej;
        if (!r_reject(body, &rej)) return;
        switch (rej.reason) {
            case REJECT_LOBBY_NOT_FOUND:  TextCopy(m->status, "lobby not found"); break;
            case REJECT_LOBBY_FULL:       TextCopy(m->status, "lobby is full"); break;
            case REJECT_SERVER_FULL:      TextCopy(m->status, "server is full"); break;
            case REJECT_VERSION_MISMATCH: TextCopy(m->status, "client out of date"); break;
            default: TextCopy(m->status, "connection rejected"); break;
        }
    }
}

static const SceneOps MENU_SCENE_OPS = {
    .exit = menu_exit, .update = menu_update, .draw = menu_draw, .on_message = menu_on_message,
};

Scene make_menu_scene(const char *status_message) {
    MenuSceneData *m = calloc(1, sizeof *m);
    if (status_message) TextCopy(m->status, status_message);
    return (Scene){ .ops = &MENU_SCENE_OPS, .data = m, .blocks_draw = true, .blocks_update = true };
}
