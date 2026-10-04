#include "menu_scene.h"
#include "raylib.h"
#include "scene.h"
#include "../app.h"

static void pause_update(Scene *self, float dt) {
    (void)self; (void)dt;
    if (IsKeyPressed(KEY_ESCAPE)) scene_pop();
}
static void pause_draw(Scene *self) {
    (void)self;
    BeginTextureMode(app.canvas);
        DrawRectangle(0, 0, 320, 180, (Color){0,0,0,120});
        DrawText("paused", 130, 80, 10, WHITE);
    EndTextureMode();
}

static void connecting_on_connect(Scene *self) {
    scene_switch(make_menu_scene(NULL));
}
static void connecting_draw(Scene *self) {
    (void)self;
    BeginTextureMode(app.canvas);
        ClearBackground(BLACK);
        DrawText("connecting", 125, 80, 10, WHITE);
    EndTextureMode();
}

static const SceneOps PAUSE_OPS = { .update = pause_update, .draw = pause_draw };
static const SceneOps CONNECTING_OPS = { .draw = connecting_draw, .on_connect = connecting_on_connect };

Scene make_pause_scene(void) { return (Scene){ .ops = &PAUSE_OPS, .blocks_draw = false, .blocks_update = true }; }
Scene make_connecting_scene(void) { return (Scene){ .ops = &CONNECTING_OPS, .blocks_draw = true, .blocks_update = true }; }
