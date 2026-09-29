#include "camera.h"
#define SDL_MAIN_USE_CALLBACKS
#include <SDL3/SDL.h>
#include <SDL3/SDL_main.h>
#include "SDL3/SDL_render.h"

#define STB_DS_IMPLEMENTATION
#include "stb_ds.h"

#include "car.h"
#include "sim.h"

typedef struct App {
    SDL_Window *window;
    SDL_Renderer *renderer;
    SDL_Texture *canvas;

    Camera camera;
    Car *cars;

    Uint64 last_time;
    bool running;

    float accumulator;
    uint32_t tick;
} App;

static bool app_init(App *app) {
    if (!SDL_Init(SDL_INIT_VIDEO))
        return false;

    app->window = SDL_CreateWindow("wrum", 1280, 720, 0);
    if (!app->window)
        return false;

    app->renderer = SDL_CreateRenderer(app->window, NULL);
    if (!app->renderer)
        return false;

    app->canvas = SDL_CreateTexture(app->renderer, SDL_PIXELFORMAT_RGBA8888,
                                    SDL_TEXTUREACCESS_TARGET, 320, 180);
    if (!app->canvas)
        return false;

    SDL_SetTextureScaleMode(app->canvas, SDL_SCALEMODE_NEAREST);

    app->camera = (Camera){0, 0, 0};

    app->cars = NULL;
    Car car;
    if (!car_init(&car, app->renderer, 100, 100, 90))
        return false;
    car.sim = (CarState){ .x = car.x, .y = car.y, .a = car.a * SDL_PI_F / 180.0f };

    Car car2;
    if (!car_init(&car2, app->renderer, 120, 100, 0))
        return false;
    car2.sim = (CarState){ .x = car2.x, .y = car2.y, .a = car2.a * SDL_PI_F / 180.0f };

    arrput(app->cars, car);
    arrput(app->cars, car2);

    app->running = true;
    app->last_time = SDL_GetTicksNS();
    return true;
}

static Input sample_input(void) {
    const bool *k = SDL_GetKeyboardState(NULL);
    Input in = {0};
    in.steer    = (int8_t)(k[SDL_SCANCODE_D] - k[SDL_SCANCODE_A]);
    in.throttle = (int8_t)(k[SDL_SCANCODE_W] - k[SDL_SCANCODE_S]);
    return in;
}

static void sim_tick(App *app) {
    Input in = sample_input();
    in.seq = app->tick++;

    Car *p = &app->cars[0];
    car_step(&p->sim, in);

    p->x = p->sim.x;
    p->y = p->sim.y;
    p->a = p->sim.a * 180.0f / SDL_PI_F;
    p->ta = p->sim.tire * 180.0f / SDL_PI_F;
}

static void app_update(App *app, float dt) {
    app->accumulator += dt;
    if (app->accumulator > 0.25f) app->accumulator = 0.25f;

    while (app->accumulator >= TICK_DT) {
        app->accumulator -= TICK_DT;
        sim_tick(app);
    }

    Car *player = &app->cars[0];
    camera_update(&app->camera, (SDL_FPoint){player->x, player->y}, player->a, dt);
}
static void app_draw(App *app) {
    SDL_SetRenderTarget(app->renderer, app->canvas);
    SDL_SetRenderDrawColor(app->renderer, 20, 255, 20, 255);
    SDL_RenderClear(app->renderer);

    for (size_t i = 0; i < arrlenu(app->cars); i++) {
         car_draw(&app->cars[i], &app->camera);
    }

    SDL_SetRenderTarget(app->renderer, NULL);
    SDL_RenderTexture(app->renderer, app->canvas, NULL, NULL);
    SDL_RenderPresent(app->renderer);
}
static void app_event(App *app, const SDL_Event *event) {
    if (event->type == SDL_EVENT_QUIT) {
        app->running = false;
    }
}

static void app_quit(App *app) {
    for (size_t i = 0; i < arrlenu(app->cars); i++) {
        car_destroy(&app->cars[i]);
    }
    arrfree(app->cars);
    SDL_DestroyTexture(app->canvas);
    SDL_DestroyRenderer(app->renderer);
    SDL_DestroyWindow(app->window);
}

SDL_AppResult SDL_AppInit(void **appstate, int argc, char **argv) {
    App *app = SDL_calloc(1, sizeof(App));
    if (!app)
        return SDL_APP_FAILURE;

    if (!app_init(app)) {
        app_quit(app);
        SDL_free(app);
        return SDL_APP_FAILURE;
    }

    *appstate = app;
    return SDL_APP_CONTINUE;
}
SDL_AppResult SDL_AppIterate(void *appstate) {
    App *app = appstate;

    Uint64 now = SDL_GetTicksNS();
    float dt = (float)(now - app->last_time) / 1000000000.0f;
    app->last_time = now;

    app_update(app, dt);
    app_draw(app);

    return app->running ? SDL_APP_CONTINUE : SDL_APP_SUCCESS;
}
SDL_AppResult SDL_AppEvent(void *appstate, SDL_Event *event) {
    App *app = appstate;
    app_event(app, event);
    return app->running ? SDL_APP_CONTINUE : SDL_APP_SUCCESS;
}
void SDL_AppQuit(void *appstate, SDL_AppResult result) {
    App *app = appstate;
    if (app) {
        app_quit(app);
        SDL_free(app);
    }
}
