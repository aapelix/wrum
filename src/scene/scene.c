#include "scene.h"
#include <assert.h>

#define MAX_SCENE_STACK 8

typedef enum { TR_NONE, TR_PUSH, TR_POP, TR_SWITCH } TransitionKind;
typedef struct { TransitionKind kind; Scene scene; } PendingTransition;

static Scene g_stack[MAX_SCENE_STACK];
static int g_stack_len;
static PendingTransition g_pending;

static void destroy_scene(Scene *s) { if (s->ops->exit) s->ops->exit(s); }

void scene_manager_init(void) { g_stack_len = 0; g_pending.kind = TR_NONE; }
void scene_manager_destroy(void) { while (g_stack_len > 0) destroy_scene(&g_stack[--g_stack_len]); }

void scene_switch(Scene scene) { g_pending = (PendingTransition){ TR_SWITCH, scene }; }
void scene_push(Scene scene)   { g_pending = (PendingTransition){ TR_PUSH, scene }; }
void scene_pop(void)           { g_pending = (PendingTransition){ TR_POP, {0} }; }

static void apply_pending(void) {
    if (g_pending.kind == TR_NONE) return;
    PendingTransition p = g_pending;
    g_pending.kind = TR_NONE;

    if (p.kind == TR_SWITCH) {
        while (g_stack_len > 0) destroy_scene(&g_stack[--g_stack_len]);
    }
    if (p.kind == TR_SWITCH || p.kind == TR_PUSH) {
        assert(g_stack_len < MAX_SCENE_STACK);
        g_stack[g_stack_len++] = p.scene;
        Scene *top = &g_stack[g_stack_len - 1];
        if (top->ops->enter) top->ops->enter(top);
    } else if (p.kind == TR_POP) {
        if (g_stack_len > 0) destroy_scene(&g_stack[--g_stack_len]);
    }
}

void scene_manager_update(float dt) {
    for (int i = g_stack_len - 1; i >= 0; i--) {
        Scene *s = &g_stack[i];
        if (s->ops->update) s->ops->update(s, dt);
        if (s->blocks_update) break;
    }
    apply_pending();
}

void scene_manager_draw(void) {
    int start = g_stack_len - 1;
    while (start > 0 && !g_stack[start].blocks_draw) start--;
    for (int i = start; i < g_stack_len; i++)
        if (g_stack[i].ops->draw) g_stack[i].ops->draw(&g_stack[i]);
}

void scene_manager_on_message(uint8_t type, Reader *body) {
    if (g_stack_len == 0) return;
    Scene *top = &g_stack[g_stack_len - 1];
    if (top->ops->on_message) top->ops->on_message(top, type, body);
    apply_pending();
}
void scene_manager_on_connect(void) {
    if (g_stack_len == 0) return;
    Scene *top = &g_stack[g_stack_len - 1];
    if (top->ops->on_connect) top->ops->on_connect(top);
    apply_pending();
}
void scene_manager_on_disconnect(void) {
    if (g_stack_len == 0) return;
    Scene *top = &g_stack[g_stack_len - 1];
    if (top->ops->on_disconnect) top->ops->on_disconnect(top);
    apply_pending();
}
