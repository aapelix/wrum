#include "map.h"
#include <stdio.h>
#include <stdlib.h>
#include <cJSON.h>
#include "camera.h"
#include "raylib.h"
#include "stb_ds.h"

static const int tileWidth = 6;
static const Vector2 center = { .x = tileWidth / 2.0f, .y = tileWidth / 2.0f };

char *read_file(const char *path) {
    FILE *file = fopen(path, "rb");
    if (!file)
        return NULL;

    fseek(file, 0, SEEK_END);
    long size = ftell(file);
    rewind(file);

    char *data = malloc(size + 1);
    if (!data) {
        fclose(file);
        return NULL;
    }

    fread(data, 1, size, file);
    data[size] = '\0';

    fclose(file);
    return data;
}

// free with map_destroy
Map* map_load(const char *path) {
    char *text = read_file(path);
    if (!text)
        return NULL;

    Map* map = calloc(1, sizeof(*map));
    if (!map) {
        free(text);
        return NULL;
    }

    cJSON *root = cJSON_Parse(text);
    free(text);
    if (!root)
        goto fail;

    cJSON *tilesetsJson = cJSON_GetObjectItemCaseSensitive(root, "tilesets");
    if (!cJSON_IsArray(tilesetsJson))
        goto fail;

    // todo: multiple tilesets
    cJSON *tilesetJson = cJSON_GetArrayItem(tilesetsJson, 0);
    cJSON *tilesetWidthJson = cJSON_GetObjectItemCaseSensitive(tilesetJson, "imagewidth");
    if (!cJSON_IsNumber(tilesetWidthJson))
        goto fail;

    int tilesetWidth = tilesetWidthJson->valueint;
    int tilesPerRow = tilesetWidth / tileWidth;

    cJSON *layersJson = cJSON_GetObjectItemCaseSensitive(root, "layers");
    if (!cJSON_IsArray(layersJson))
        goto fail;

    // todo: multiple layers
    cJSON *groundLayerJson = cJSON_GetArrayItem(layersJson, 0);
    cJSON *layerWidthJson = cJSON_GetObjectItemCaseSensitive(groundLayerJson, "width");
    if (!cJSON_IsNumber(layerWidthJson))
        goto fail;

    int layerWidth = layerWidthJson->valueint;
    int layerWidthPixels = layerWidth * tileWidth;

    cJSON *tilesJson = cJSON_GetObjectItemCaseSensitive(groundLayerJson, "data");
    if (!cJSON_IsArray(tilesJson))
        goto fail;

    float x = 0;
    float y = 0;
    cJSON *tileJson = NULL;
    cJSON_ArrayForEach(tileJson, tilesJson) {
        int index = tileJson->valueint-1;
        int col = index % tilesPerRow;
        int row = index / tilesPerRow;

        Rectangle src = {
            .x = col * tileWidth, .y = row * tileWidth,
            .width = tileWidth, .height = tileWidth,
        };

        if (x >= layerWidthPixels) {
            x = 0;
            y += tileWidth;
        }

        Vector2 pos = { .x = x, .y = y };
        Tile tile = { .pos = pos, .src = src };
        arrput(map->tiles, tile);

        x += tileWidth;
    }
    cJSON_Delete(root);

    map->tileset = LoadTexture("assets/tiles.png");
    return map;
fail:
    cJSON_Delete(root);
    map_destroy(map);
    return NULL;
}

void map_draw(const Map *map, const GameCamera *cam) {
    for (int i = 0; i < arrlenu(map->tiles); i++) {
        Tile t = map->tiles[i];
        float a = camera_apply_a(cam, 0);
        Vector2 world = camera_apply_pos(cam, t.pos);
        DrawTexturePro(
            map->tileset,  t.src,
            (Rectangle){.x = world.x, .y = world.y, .width = tileWidth, .height = tileWidth},
            center, a, WHITE
        );
    }
}

void map_destroy(Map *map) {
    if (!map)
        return;

    arrfree(map->tiles);
    UnloadTexture(map->tileset);
    free(map);
}
