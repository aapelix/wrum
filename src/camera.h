#pragma once

#include "SDL3/SDL_rect.h"
typedef struct Camera {
    float x, y;
    double a;
} Camera;

void camera_update(Camera *camera, SDL_FPoint target, double target_a, float dt);

SDL_FPoint camera_apply_pos(const Camera *camera, SDL_FPoint p);
double camera_apply_a(const Camera *camera, double a);
