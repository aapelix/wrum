#pragma once

typedef struct { float x, y; } Vec2;
typedef struct { float x, y, hw, hh, a; } OBB;

typedef struct { OBB box; Vec2 vel; float inv_mass; } CollisionBody;

bool obb_overlap(const OBB *a, const OBB *b, Vec2 *normal, float *depth);
void resolve_collision(CollisionBody *a, CollisionBody *b, float restitution);
OBB obb_from_segment(Vec2 p0, Vec2 p1, float thickness);
