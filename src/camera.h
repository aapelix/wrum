#pragma once

#include "raylib.h"
typedef struct GameCamera {
    float x, y;
    double a;
} GameCamera;

void camera_update(GameCamera *camera, Vector2 target, double target_a, float dt);

Vector2 camera_apply_pos(const GameCamera *camera, Vector2 p);
double camera_apply_a(const GameCamera *camera, double a);
