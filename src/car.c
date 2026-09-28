#include "car.h"
#include "SDL3/SDL_rect.h"
#include "SDL3/SDL_stdinc.h"
#include "stack.h"
#include <math.h>
#include <stdio.h>

static const SDL_FPoint tire_positions[4] = {
    { -3.5f, -4.0f },
    {  3.5f, -4.0f },
    { -3.5f,  4.0f },
    {  3.5f,  4.0f },
};

bool car_init(Car *c, SDL_Renderer *r, float x, float y, double a) {
    SDL_zerop(c);
    c->x = x;
    c->y = y;
    c->a = a;

    if (!stack_init(&c->tires[0], r, "assets/tire.png", 1))
        goto fail;

    if (!stack_init(&c->tires[1], r, "assets/tire.png", 1))
        goto fail;

    if (!stack_init(&c->tires[2], r, "assets/tire.png", 1))
        goto fail;

    if (!stack_init(&c->tires[3], r, "assets/tire.png", 1))
        goto fail;

    if (!stack_init(&c->body, r, "assets/body.png", 10))
        goto fail;

    return true;

fail:
    car_destroy(c);
    return false;
}

void car_destroy(Car *c) {
    for (int i = 0; i < 4; i++)
        stack_destroy(&c->tires[i]);

    stack_destroy(&c->body);
}

void car_draw(Car *c) {
    for (int i = 0; i < 4; i++) {
        SDL_FPoint base = tire_positions[i];

        float rad = c->a * (float)M_PI / 180.0f;
        float s = sinf(rad);
        float co = cosf(rad);
        float x = base.x * co - base.y * s;
        float y = base.x * s + base.y * co;

        stack_draw(&c->tires[i], c->x + x, c->y + y, c->a);
    }

    stack_draw(&c->body, c->x, c->y, c->a);
}
