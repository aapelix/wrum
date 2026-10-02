#pragma once

#include "raylib.h"
typedef struct Tile {
    float x, y;
    Rectangle src;
} Tile;

// this prob should return Map*
Tile* load_map(const char *path);

typedef struct Map {
    Tile *tiles;
    Texture2D *tileset;
} Map;

void map_draw(const Map *map);
void map_destroy(Map *map);
