#pragma once
#include <stdint.h>
#include <stdbool.h>
#include "../bytes.h"

typedef struct Scene Scene;

typedef struct {
    void (*enter)(Scene *self);
    void (*exit)(Scene *self);
    void (*update)(Scene *self, float dt);
    void (*draw)(Scene *self);

    void (*on_message)(Scene *self, uint8_t type, Reader *body);
    void (*on_connect)(Scene *self);
    void (*on_disconnect)(Scene *self);
} SceneOps;

struct Scene {
    const SceneOps *ops;
    void *data;
    bool blocks_draw;
    bool blocks_update;
};

void scene_manager_init(void);
void scene_manager_destroy(void);

void scene_switch(Scene scene);
void scene_push(Scene scene);
void scene_pop(void);

void scene_manager_update(float dt);
void scene_manager_draw(void);

void scene_manager_on_message(uint8_t type, Reader *body);
void scene_manager_on_connect(void);
void scene_manager_on_disconnect(void);
