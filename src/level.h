#pragma once
#include "collision.h"
#include "sim.h"

typedef struct {
    OBB *walls;
} Level;

Level *level_load(const char *path);
void level_resolve_car(const Level* level, CarState *car);
void level_destroy(Level *level);
