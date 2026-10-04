#include "level.h"
#include "cJSON.h"
#include "stb_ds.h"
#include <stdio.h>
#include <stdlib.h>

#define WALL_RESTITUTION 0.4f
#define PI 3.14159265358979323846f

static char *read_file(const char *path) {
    FILE *file = fopen(path, "rb");
    if (!file) return NULL;
    fseek(file, 0, SEEK_END);
    long size = ftell(file);
    rewind(file);
    char *data = malloc(size + 1);
    if (!data) { fclose(file); return NULL; }
    fread(data, 1, size, file);
    data[size] = '\0';
    fclose(file);
    return data;
}

static OBB obb_from_tiled_rect(float x, float y, float w, float h, float rotation_deg) {
    float rad = rotation_deg * (float)PI / 180.0f;
    float cos_r = cosf(rad), sin_r = sinf(rad);
    float lx = w * 0.5f, ly = h * 0.5f;
    float wx = x + lx * cos_r - ly * sin_r;
    float wy = y + lx * sin_r + ly * cos_r;
    return (OBB){ wx, wy, w * 0.5f, h * 0.5f, rad };
}

Level *level_load(const char *path) {
    char *text = read_file(path);
    if (!text) return NULL;

    Level *level = calloc(1, sizeof *level);
    if (!level) { free(text); return NULL; }

    cJSON *root = cJSON_Parse(text);
    free(text);
    if (!root) goto fail;

    cJSON *layersJson = cJSON_GetObjectItemCaseSensitive(root, "layers");
    if (!cJSON_IsArray(layersJson)) goto fail;

    cJSON *layerJson = NULL;
    cJSON_ArrayForEach(layerJson, layersJson) {
        cJSON *typeJson = cJSON_GetObjectItemCaseSensitive(layerJson, "type");
        if (!cJSON_IsString(typeJson) || strcmp(typeJson->valuestring, "objectgroup") != 0)
            continue;

        cJSON *objectsJson = cJSON_GetObjectItemCaseSensitive(layerJson, "objects");
        if (!cJSON_IsArray(objectsJson)) goto fail;

        cJSON *objectJson = NULL;
        cJSON_ArrayForEach(objectJson, objectsJson) {
            cJSON *objType = cJSON_GetObjectItemCaseSensitive(objectJson, "type");
            if (!cJSON_IsString(objType) || strcmp(objType->valuestring, "wall") != 0)
                continue;

            cJSON *xJson = cJSON_GetObjectItemCaseSensitive(objectJson, "x");
            cJSON *yJson = cJSON_GetObjectItemCaseSensitive(objectJson, "y");
            cJSON *wJson = cJSON_GetObjectItemCaseSensitive(objectJson, "width");
            cJSON *hJson = cJSON_GetObjectItemCaseSensitive(objectJson, "height");
            if (!cJSON_IsNumber(xJson) || !cJSON_IsNumber(yJson) ||
                !cJSON_IsNumber(wJson) || !cJSON_IsNumber(hJson))
                goto fail;

            cJSON *rotJson = cJSON_GetObjectItemCaseSensitive(objectJson, "rotation");
            float rotation = cJSON_IsNumber(rotJson) ? (float)rotJson->valuedouble : 0.0f;

            OBB wall = obb_from_tiled_rect(
                (float)xJson->valuedouble, (float)yJson->valuedouble,
                (float)wJson->valuedouble, (float)hJson->valuedouble,
                rotation);
            arrput(level->walls, wall);
        }
    }

    cJSON_Delete(root);
    return level;
fail:
    cJSON_Delete(root);
    level_destroy(level);
    return NULL;
}

void level_resolve_car(const Level *level, CarState *car) {
    for (size_t i = 0; i < arrlenu(level->walls); i++)
        car_resolve_vs_obstacle(car, level->walls[i], WALL_RESTITUTION);
}

void level_destroy(Level *level) {
    if (!level) return;
    arrfree(level->walls);
    free(level);
}
