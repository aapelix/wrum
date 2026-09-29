#pragma once

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
