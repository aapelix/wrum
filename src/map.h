#pragma once

#include "camera.h"
#include "raylib.h"
#include "stack.h"

#define MAX_LAYERS 2

typedef struct Tile {
    Vector2 pos;
    Rectangle src;
} Tile;
typedef struct Layer {
    Tile *tiles;
} Layer;

typedef struct Prop {
    Stack stack;
    Vector2 pos;
    double a;
} Prop;

typedef struct Map {
    Layer layers[MAX_LAYERS];
    Prop *props;
    Texture2D tileset;
} Map;

Map* map_load(const char *path);

void map_draw_layers(const Map *map, const GameCamera *cam);
int map_prop_count(const Map *map);
Vector2 map_prop_pos(const Map *map, int index);
void map_draw_prop(const Map *map, int index, const GameCamera *cam);
void map_draw_shadows(const Map *map, const GameCamera *cam);

void map_destroy(Map *map);
