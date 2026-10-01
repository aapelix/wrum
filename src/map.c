#include "map.h"
#include <stdio.h>
#include <stdlib.h>
#include <cJSON.h>
#include "SDL3/SDL_rect.h"
#include "stb_ds.h"

static const int tileWidth = 6;

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

Tile* load_map(const char *path) {
    char *text = read_file(path);
    if (!text)
        return NULL;

    Tile *tiles = NULL;

    cJSON *root = cJSON_Parse(text);
    free(text);
    if (!root)
        goto end;

    cJSON *tilesetsJson = cJSON_GetObjectItemCaseSensitive(root, "tilesets");
    if (!cJSON_IsArray(tilesetsJson))
        goto end;

    // todo: multiple tilesets
    cJSON *tilesetJson = cJSON_GetArrayItem(tilesetsJson, 0);
    cJSON *tilesetWidthJson = cJSON_GetObjectItemCaseSensitive(tilesetJson, "imagewidth");
    if (!cJSON_IsNumber(tilesetWidthJson))
        goto end;

    int tilesetWidth = tilesetWidthJson->valueint;
    int tilesPerRow = tilesetWidth / tileWidth;

    cJSON *layersJson = cJSON_GetObjectItemCaseSensitive(root, "layers");
    if (!cJSON_IsArray(layersJson))
        goto end;

    // todo: multiple layers
    cJSON *groundLayerJson = cJSON_GetArrayItem(layersJson, 0);
    cJSON *layerWidthJson = cJSON_GetObjectItemCaseSensitive(groundLayerJson, "width");
    if (!cJSON_IsNumber(layerWidthJson))
        goto end;

    int layerWidth = layerWidthJson->valueint;
    int layerWidthPixels = layerWidth * tileWidth;

    cJSON *tilesJson = cJSON_GetObjectItemCaseSensitive(groundLayerJson, "data");
    if (!cJSON_IsArray(tilesJson))
        goto end;

    float x = 0;
    float y = 0;
    cJSON *tileJson = NULL;
    cJSON_ArrayForEach(tileJson, tilesJson) {
        int index = tileJson->valueint;
        int col = index % tilesPerRow;
        int row = index / tilesPerRow;

        SDL_FRect src = {
            .x = col * tileWidth, .y = row * tileWidth,
            .w = tileWidth, .h = tileWidth,
        };

        if (x >= layerWidthPixels) {
            x = 0;
            y += tileWidth;
        }
        Tile tile = { .x = x, .y = y, .src = src };
        arrput(tiles, tile);

        x += tileWidth;
    }
    goto end;
end:
    cJSON_Delete(root);
    return tiles;
}
