#version 100
#ifdef GL_FRAGMENT_PRECISION_HIGH
precision highp float;
#else
precision mediump float;
#endif

varying vec2 screenPos;

uniform float uTime;
uniform vec2  uOrigin;
uniform vec4  uInv;
uniform vec3  uClip;
uniform float uAlpha;

const vec3 C_FOAM  = vec3(0.93, 0.97, 1.00);
const vec3 C_LIGHT = vec3(0.35, 0.65, 0.90);
const vec3 C_MID   = vec3(0.20, 0.45, 0.78);
const vec3 C_DARK  = vec3(0.08, 0.20, 0.45);

vec2 toWorld(vec2 s) {
    vec2 d = s - uOrigin;
    return vec2(uInv.x * d.x + uInv.y * d.y,
                uInv.z * d.x + uInv.w * d.y);
}

float hash(vec2 p) {
    vec3 p3 = fract(vec3(p.xyx) * 0.1031);
    p3 += dot(p3, p3.yzx + 33.33);
    return fract((p3.x + p3.y) * p3.z);
}

float vnoise(vec2 p) {
    vec2 i = floor(p), f = fract(p);
    f = f * f * (3.0 - 2.0 * f);
    return mix(mix(hash(i),                  hash(i + vec2(1.0, 0.0)), f.x),
               mix(hash(i + vec2(0.0, 1.0)), hash(i + vec2(1.0, 1.0)), f.x), f.y);
}

float fbm(vec2 p) {
    return 0.55 * vnoise(p) + 0.30 * vnoise(p * 2.0) + 0.15 * vnoise(p * 4.0);
}

float foam(vec2 w) {
    vec2 warp = vec2(vnoise(w * 0.05 + uTime * 0.15),
                     vnoise(w * 0.05 + 7.3 - uTime * 0.12));
    float n = fbm(w * 0.04 + warp * 1.5);
    return step(0.83, fract(n * 2.0 - uTime * 0.25));
}

bool foamRing(vec2 s) {
    float d = length(s - uClip.xy);
    float r = uClip.z - 4.0 + sin(uTime * 6.0);
    float dd = abs(d - r);
    if (dd < 1.0) return true;
    vec2 w = toWorld(s);
    return dd < 2.5 && (foam(w) > 0.5 || vnoise(w * 0.4 + uTime) > 0.55);
}

void main() {
    vec2 sp = floor(screenPos) + 0.5;
    vec2 w  = toWorld(sp);

    float b = fbm(w * 0.012 + vec2(uTime * 0.02, uTime * 0.01));
    vec3 col = b > 0.5 ? C_LIGHT : C_MID;

    float f  = foam(w);
    float fs = foam(toWorld(sp - vec2(0.0, 2.0)));

    if (f > 0.5)       col = C_FOAM;
    else if (fs > 0.5) col = C_DARK;

    float a = uAlpha;
    if (uClip.z > 0.0) {
        float d = length(screenPos - uClip.xy);
        if (d > uClip.z) discard;
        a *= 1.0 - smoothstep(uClip.z - 5.0, uClip.z, d);

        vec2 s = floor(screenPos) + 0.5;
        if (foamRing(s)) {
            col = C_FOAM;
            a = 0.95;
        } else if (foamRing(s - vec2(0.0, 2.0))) {
            col = C_DARK;
            a = max(a, 0.85);
        }
    }
    gl_FragColor = vec4(col, a);
}
