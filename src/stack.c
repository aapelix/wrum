#include "stack.h"
#include <SDL3_image/SDL_image.h>

bool stack_init(Stack *s, SDL_Renderer *r, const char *path, int sw) {
    SDL_zerop(s);
    s->r = r;
    s->sw = sw;

    s->tex = IMG_LoadTexture(r, path);
    if (!s->tex)
        return false;

    SDL_SetTextureScaleMode(s->tex, SDL_SCALEMODE_NEAREST);

    float width, height;
    SDL_GetTextureSize(s->tex, &width, &height);

    s->w = (int)width;
    s->h = (int)height;
    s->c = s->w / sw;

    s->center.x = sw / 2.0f;
    s->center.y = s->h / 2.0f;
    return true;
}

void stack_destroy(Stack *s) {
    if (s->tex) {
        SDL_DestroyTexture(s->tex);
        s->tex = NULL;
    }
}

void stack_draw(const Stack *s, float x, float y, double a) {
    float fw = (float)s->sw;
    float fh = (float)s->h;

    for (int i = 0; i < s->c; i++) {
        SDL_FRect src = { (float)(i * s->sw), 0, fw, fh };
        SDL_FRect dst = {
            x - fw / 2.0f,
            y - fh / 2.0f - i,
            fw,
            fh
        };

        SDL_RenderTextureRotated(
            s->r, s->tex, &src, &dst, a,
            &s->center,
            SDL_FLIP_NONE
        );
    }
}
