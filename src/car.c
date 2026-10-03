#include "car.h"
#include "camera.h"
#include "stack.h"
#include <math.h>
#include <stdio.h>

static const Vector2 tire_positions[4] = {
    { -3.5f, -4.0f },
    {  3.5f, -4.0f },
    { -3.5f,  4.0f },
    {  3.5f,  4.0f },
};

static const Vector2 shadow_offset = {
    1.0f, 0.5f
};

bool car_init(Car *c, float x, float y, double a) {
    *c = (Car){0};

    c->x = x;
    c->y = y;
    c->a = a;

    for (int i = 0; i < 4; i++) {
        if (!stack_init(&c->tires[i], "assets/tire.png", 1))
            goto fail;
    }

    if (!stack_init(&c->body, "assets/body.png", 10))
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

void car_draw(const Car *c, const GameCamera *cam) {
    float rad = c->a * M_PI / 180.0f;
    float s = sinf(rad);
    float co = cosf(rad);

    double draw_a = camera_apply_a(cam, c->a);

    for (int i = 0; i < 4; i++) {
        Vector2 base = tire_positions[i];

        Vector2 world = {
            c->x + base.x * co - base.y * s,
            c->y + base.x * s + base.y * co
        };

        Vector2 screen = camera_apply_pos(cam, world);
        float a = draw_a;
        if (i < 2) {
            a += c->ta;
        }

        stack_draw(
            &c->tires[i],
            screen.x,
            screen.y,
            a
        );
    }

    Vector2 p = camera_apply_pos(cam, (Vector2){c->x, c->y});
    stack_draw(&c->body, p.x, p.y, draw_a);
}

void car_draw_shadow(const Car *c, const GameCamera *cam) {
    double draw_a = camera_apply_a(cam, c->a);

    float rad = c->a * M_PI / 180.0f;
    float s = sinf(rad);
    float co = cosf(rad);

    Vector2 offset = {
        shadow_offset.x * co - shadow_offset.y * s,
        shadow_offset.x * s + shadow_offset.y * co
    };

    Vector2 shadow_pos = {
        c->x + shadow_offset.x,
        c->y + shadow_offset.y
    };

    Vector2 p = camera_apply_pos(cam, shadow_pos);

    for (int i = 0; i < 4; i++) {
        Vector2 base = tire_positions[i];

        Vector2 world = {
            shadow_pos.x + base.x * co - base.y * s,
            shadow_pos.y + base.x * s + base.y * co
        };

        Vector2 screen = camera_apply_pos(cam, world);

        float a = draw_a;
        if (i < 2)
            a += c->ta;

        stack_draw_shadow(
            &c->tires[i],
            screen.x,
            screen.y,
            a
        );
    }

    stack_draw_shadow(&c->body, p.x, p.y, draw_a);
}
