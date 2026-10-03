#pragma once

#include "camera.h"
#include "raylib.h"

#define MAX_LAYERS 2

typedef struct Tile {
    Vector2 pos;
    Rectangle src;
} Tile;
typedef struct Layer {
    Tile *tiles;
} Layer;

typedef struct Map {
    Layer layers[MAX_LAYERS];
    Texture2D tileset;
} Map;

Map* map_load(const char *path);

void map_draw(const Map *map, const GameCamera *cam);
void map_destroy(Map *map);
