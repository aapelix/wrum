#pragma once

#include "camera.h"
#include "raylib.h"

typedef struct Tile {
    Vector2 pos;
    Rectangle src;
} Tile;
typedef struct Map {
    Tile *tiles;
    Texture2D tileset;
} Map;

Map* map_load(const char *path);

void map_draw(const Map *map, const GameCamera *cam);
void map_destroy(Map *map);
