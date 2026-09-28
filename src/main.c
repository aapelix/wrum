#include "stack.h"
#define SDL_MAIN_USE_CALLBACKS
#include <SDL3/SDL.h>
#include <SDL3/SDL_main.h>

typedef struct App {
    SDL_Window *window;
    SDL_Renderer *renderer;
    SDL_Texture *canvas;

    Stack stack;
    float rotation;

    Uint64 last_time;
    bool running;
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

    if (!stack_init(&app->stack, app->renderer, "assets/body.png", 10))
        return false;

    app->rotation = 0.0f;
    app->running = true;
    app->last_time = SDL_GetTicksNS();
    return true;
}

static void app_update(App *app, float dt) {
    app->rotation += 90.0f * dt;
}
static void app_draw(App *app) {
    SDL_SetRenderTarget(app->renderer, app->canvas);
    SDL_RenderClear(app->renderer);

    // render

    for (int i = 0; i < 9; i++)
        stack_draw(&app->stack, 100.0f + i * 10.0f, 100.0f, app->rotation);

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
    stack_destroy(&app->stack);
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
