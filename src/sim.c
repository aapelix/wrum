#include "sim.h"
#include <math.h>

#define PI 3.14159265358979323846f

static const float max_v = 200.0f;
static const float acc = 5.0f;
static const float drag = 5.0f;
static const float rs = 0.1f;
static const float max_a = 30.0f * PI / 180.0f;
static const float tire_rs = 0.05f;
static const float tire_ts = 0.1f;

static float approach(float v, float target, float step) {
    if (v < target) return fminf(v + step, target);
    return fmaxf(v - step, target);
}

void car_resolve_vs_obstacle(CarState *car, OBB obstacle, float restitution) {
    CollisionBody cb = { car_obb(car), car_velocity(car), 1.0f };
    CollisionBody ob = { obstacle, {0, 0}, 0.0f };
    resolve_collision(&cb, &ob, restitution);
    car->x = cb.box.x; car->y = cb.box.y;
    car_apply_velocity(car, cb.vel);
}

void car_step(CarState *c, Input in) {
    float steer = fminf(fmaxf(in.steer, -1.0f), 1.0f);
    float thr   = fminf(fmaxf(in.throttle, -1.0f), 1.0f);

    c->speed = approach(c->speed, thr * max_v, thr != 0.0f ? acc : drag);
    c->tire = approach(c->tire, steer * max_a, steer != 0.0f ? tire_ts : tire_rs);

    c->a += c->tire * (c->speed / max_v) * rs;

    c->x += sinf(c->a) * c->speed * TICK_DT;
    c->y -= cosf(c->a) * c->speed * TICK_DT;

    c->a = fmodf(c->a + PI, 2.0f * PI);
    if (c->a <= 0.0f)
        c->a += 2.0f * PI;
    c->a -= PI;
}
