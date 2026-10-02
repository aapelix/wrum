#include "stack.h"
#include "raylib.h"

bool stack_init(Stack *s, const char *path, int sw) {
    *s = (Stack){0};

    s->sw = sw;

    s->tex = LoadTexture(path);
    if (s->tex.id == 0)
        return false;

    SetTextureFilter(s->tex, TEXTURE_FILTER_POINT);

    s->w = s->tex.width;
    s->h = s->tex.height;
    s->c = s->w / sw;

    s->center.x = sw / 2.0f;
    s->center.y = s->h / 2.0f;
    return true;
}
void stack_destroy(Stack *s) {
    UnloadTexture(s->tex);
    s->tex = (Texture2D){0};
}
void stack_draw(const Stack *s, float x, float y, double a) {
    float fw = (float)s->sw;
    float fh = (float)s->h;

    for (int i = 0; i < s->c; i++) {
        Rectangle src = {
            (float)(i * fw),
            0,
            fw,
            fh
        };

        Rectangle dst = {
            x,
            y - i,
            fw,
            fh
        };

        DrawTexturePro(
            s->tex,
            src,
            dst,
            s->center,
            a,
            WHITE
        );
    }
}
