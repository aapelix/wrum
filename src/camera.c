#include "camera.h"
#include "SDL3/SDL_rect.h"
#include <math.h>

void camera_update(Camera *camera, SDL_FPoint target, double target_a, float dt) {
    float t = 1.0f - expf(-10.0f * dt);

    camera->x += (target.x - camera->x) * t;
    camera->y += (target.y - camera->y) * t;

    double a_diff = fmod(target_a - camera->a + 180.0, 360.0) - 180.0;
    camera->a += a_diff * t;
}

SDL_FPoint camera_apply_pos(const Camera *camera, SDL_FPoint p) {
    float tx = p.x - camera->x;
    float ty = p.y - camera->y;

    float rad = -camera->a * (float)M_PI / 180.0f;
    float s = sinf(rad);
    float c = cosf(rad);

    float rx = tx * c - ty * s;
    float ry = tx * s + ty * c;

    return (SDL_FPoint){rx + 160.0f, ry + 90.0f};
}
double camera_apply_a(const Camera *camera, double a) {
    return a-camera->a;
}
