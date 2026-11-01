#version 330
in vec2 screenPos;
out vec4 finalColor;

uniform float uTime;
uniform vec2  uOrigin;
uniform vec4  uInv;
uniform vec3  uClip;
uniform float uAlpha;
uniform sampler2D uMap;
uniform vec2  uMapSize;
uniform float uMask;

const vec3 C_FOAM  = vec3(0.93, 0.97, 1.00);
const vec3 C_LIGHT = vec3(0.35, 0.65, 0.90);
const vec3 C_MID   = vec3(0.20, 0.45, 0.78);
const vec3 C_DARK  = vec3(0.08, 0.20, 0.45);

vec2 toWorld(vec2 s) {
    vec2 d = s - uOrigin;
    return vec2(uInv.x * d.x + uInv.y * d.y,
                uInv.z * d.x + uInv.w * d.y);
}

float hash(vec2 p) { return fract(sin(dot(p, vec2(127.1, 311.7))) * 43758.5453); }

float vnoise(vec2 p) {
    vec2 i = floor(p), f = fract(p);
    f = f * f * (3.0 - 2.0 * f);
    return mix(mix(hash(i),               hash(i + vec2(1, 0)), f.x),
               mix(hash(i + vec2(0, 1)),  hash(i + vec2(1, 1)), f.x), f.y);
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
    vec2 w = toWorld(floor(screenPos) + 0.5);
    if (uMask > 0.5) {
        float land = texture(uMap, w / uMapSize).a;
        if (land > 0.5) discard;
    }

    float b = fbm(w * 0.012 + vec2(uTime * 0.02, uTime * 0.01));
    vec3 col = b > 0.5 ? C_LIGHT : C_MID;

    float f  = foam(w);
    float fs = foam(toWorld(floor(screenPos) + 0.5 - vec2(0.0, 2.0)));

    if (f > 0.5)        col = C_FOAM;
    else if (fs > 0.5)  col = C_DARK;

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
    finalColor = vec4(col, a);
}
