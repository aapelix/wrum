#pragma once

#include "SDL3/SDL_render.h"
#include "stack.h"

typedef struct Car {
    Stack tires[4];
    Stack body;

    float x, y;
    double a;
} Car;

bool car_init(Car *c, SDL_Renderer *r, float x, float y, double a);
void car_destroy(Car *c);
void car_draw(Car *c);
