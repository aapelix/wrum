#include "sim.h"
#include "SDL3/SDL_stdinc.h"

static const float max_v = 200.0f;
static const float acc = 5.0f;
static const float drag = 5.0f;
static const float rs = 0.1f;
static const float max_a = 30.0f * SDL_PI_F / 180.0f;
static const float tire_rs = 0.05f;
static const float tire_ts = 0.1f;

static float approach(float v, float target, float step) {
    if (v < target) return SDL_min(v + step, target);
    return SDL_max(v - step, target);
}

void car_step(CarState *c, Input in) {
    float steer = SDL_clamp(in.steer, -1, 1);
    float thr = SDL_clamp(in.throttle, -1, 1);

    c->speed = approach(c->speed, thr * max_v, thr != 0.0f ? acc : drag);
    c->tire = approach(c->tire, steer * max_a, steer != 0.0f ? tire_ts : tire_rs);

    c->a += c->tire * (c->speed / max_v) * rs;

    c->x += SDL_sinf(c->a) * c->speed * TICK_DT;
    c->y -= SDL_cosf(c->a) * c->speed * TICK_DT;

    c->a = SDL_fmodf(c->a + SDL_PI_F, 2.0f * SDL_PI_F);
    if (c->a <= 0.0f)
        c->a += 2.0f * SDL_PI_F;
    c->a -= SDL_PI_F;
}
