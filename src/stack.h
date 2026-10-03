#pragma once


#include "raylib.h"
typedef struct Stack {
    Texture2D tex;
    RenderTexture2D shadow;
    Vector2 center;

    int sw;
    int w;
    int h;
    int c;
} Stack;

bool stack_init(Stack *s, const char *path, int sw);
void stack_destroy(Stack *s);
void stack_draw(const Stack *s, float x, float y, double a);
void stack_draw_shadow(const Stack *s, float x, float y, double a);
