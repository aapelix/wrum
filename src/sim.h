#pragma once

#include "collision.h"
#include <math.h>
#include <stdint.h>

#define TICK_RATE 60
#define TICK_DT (1.0f / TICK_RATE)

typedef struct Input {
    uint32_t seq;
    int8_t   steer;
    int8_t   throttle;
    uint8_t  buttons;
} Input;

typedef struct CarState {
    float x, y, a, speed, tire;
} CarState;

void car_step(CarState *c, Input in);

#define CAR_HALF_W 5.0f
#define CAR_HALF_H 8.0f

static inline OBB car_obb(const CarState *c) {
    return (OBB){ c->x, c->y, CAR_HALF_W, CAR_HALF_H, c->a };
}
static inline Vec2 car_velocity(const CarState *c) {
    return (Vec2){ sinf(c->a) * c->speed, -cosf(c->a) * c->speed };
}
static inline void car_apply_velocity(CarState *c, Vec2 v) {
    float hx = sinf(c->a), hy = -cosf(c->a);
    c->speed = v.x*hx + v.y*hy;
}
