#include "stack.h"
#include "raylib.h"
#include "raymath.h"

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

    s->shadow = LoadRenderTexture(sw, s->h);
    s->shadow = LoadRenderTexture(sw, s->h);
    if (s->shadow.texture.id == 0) {
        UnloadTexture(s->tex);
        s->tex = (Texture2D){0};
        return false;
    }

    BeginTextureMode(s->shadow);
        ClearBackground(BLANK);

        for (int i = 0; i < s->c; i++) {
            Rectangle src = {
                (float)(i * sw),
                0,
                sw,
                s->h
            };

            DrawTextureRec(s->tex, src, Vector2Zero(), BLACK);
        }
    EndTextureMode();

    return true;
}
void stack_destroy(Stack *s) {
    UnloadTexture(s->tex);
    UnloadRenderTexture(s->shadow);
    s->tex = (Texture2D){0};
    s->shadow = (RenderTexture2D){0};
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
void stack_draw_shadow(const Stack *s, float x, float y, double a) {
    DrawTexturePro(s->shadow.texture, (Rectangle){0, 0, s->sw, s->h}, (Rectangle){x, y, s->sw, s->h}, s->center, a, WHITE);
}
