#include "collision.h"
#include <math.h>
#include <float.h>

static Vec2 obb_axis(const OBB *b, int which) {
    float c = cosf(b->a), s = sinf(b->a);
    return which == 0 ? (Vec2){c, s} : (Vec2){-s, c};
}
static float obb_project_radius(const OBB *b, Vec2 axis) {
    Vec2 ax0 = obb_axis(b, 0), ax1 = obb_axis(b, 1);
    return fabsf(ax0.x*axis.x + ax0.y*axis.y) * b->hw
         + fabsf(ax1.x*axis.x + ax1.y*axis.y) * b->hh;
}

bool obb_overlap(const OBB *a, const OBB *b, Vec2 *normal, float *depth) {
    Vec2 axes[4] = { obb_axis(a,0), obb_axis(a,1), obb_axis(b,0), obb_axis(b,1) };
    Vec2 d = { b->x - a->x, b->y - a->y };

    float best_depth = FLT_MAX;
    Vec2 best_axis = {1, 0};

    for (int i = 0; i < 4; i++) {
        Vec2 axis = axes[i];
        float center_dist = fabsf(d.x*axis.x + d.y*axis.y);
        float overlap = obb_project_radius(a, axis) + obb_project_radius(b, axis) - center_dist;
        if (overlap <= 0.0f) return false;
        if (overlap < best_depth) { best_depth = overlap; best_axis = axis; }
    }

    if (d.x*best_axis.x + d.y*best_axis.y < 0.0f) { best_axis.x = -best_axis.x; best_axis.y = -best_axis.y; }
    *normal = best_axis;
    *depth = best_depth;
    return true;
}

void resolve_collision(CollisionBody *a, CollisionBody *b, float restitution) {
    Vec2 normal; float depth;
    if (!obb_overlap(&a->box, &b->box, &normal, &depth)) return;

    float total_inv_mass = a->inv_mass + b->inv_mass;
    if (total_inv_mass <= 0.0f) return;

    const float slop = 0.5f, percent = 0.8f;
    float corr = fmaxf(depth - slop, 0.0f) * percent / total_inv_mass;
    a->box.x -= normal.x * corr * a->inv_mass;
    a->box.y -= normal.y * corr * a->inv_mass;
    b->box.x += normal.x * corr * b->inv_mass;
    b->box.y += normal.y * corr * b->inv_mass;

    Vec2 rel = { b->vel.x - a->vel.x, b->vel.y - a->vel.y };
    float vn = rel.x*normal.x + rel.y*normal.y;
    if (vn > 0.0f) return;

    float j = -(1.0f + restitution) * vn / total_inv_mass;
    Vec2 impulse = { normal.x * j, normal.y * j };
    a->vel.x -= impulse.x * a->inv_mass;
    a->vel.y -= impulse.y * a->inv_mass;
    b->vel.x += impulse.x * b->inv_mass;
    b->vel.y += impulse.y * b->inv_mass;
}

OBB obb_from_segment(Vec2 p0, Vec2 p1, float thickness) {
    float dx = p1.x - p0.x, dy = p1.y - p0.y;
    float len = sqrtf(dx*dx + dy*dy);
    float a = atan2f(-dx, dy);
    return (OBB){ (p0.x+p1.x)*0.5f, (p0.y+p1.y)*0.5f, thickness*0.5f, len*0.5f, a };
}
