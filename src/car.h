#pragma once

#include "camera.h"
#include "stack.h"
#include "sim.h"

typedef struct Car {
    Stack tires[4];
    Stack body;

    float x, y;
    double a, ta;

    CarState sim;
} Car;

bool car_init(Car *c, float x, float y, double a);
void car_destroy(Car *c);
void car_draw(const Car *c, const GameCamera *cam);
void car_draw_shadow(const Car *c, const GameCamera *cam);
