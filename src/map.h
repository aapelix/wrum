#pragma once

#include "SDL3/SDL_rect.h"
#include "SDL3/SDL_render.h"
typedef struct Tile {
    float x, y;
    SDL_FRect src;
} Tile;

// this prob should return Map*
Tile* load_map(const char *path);

typedef struct Map {
    Tile *tiles;
    SDL_Renderer *r;
    SDL_Texture *tileset;
} Map;

void map_draw(const Map *map);
void map_destroy(Map *map);
