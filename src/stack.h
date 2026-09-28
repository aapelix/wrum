#pragma once

#include <SDL3/SDL.h>

typedef struct Stack {
    SDL_Renderer *r;
    SDL_Texture *tex;
    SDL_FPoint center;

    int sw;
    int w;
    int h;
    int c;
} Stack;

bool stack_init(Stack *s, SDL_Renderer *r, const char *path, int sw);
void stack_destroy(Stack *s);
void stack_draw(const Stack *s, float x, float y, double a);
