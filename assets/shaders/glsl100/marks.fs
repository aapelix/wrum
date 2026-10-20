#version 100
precision mediump float;

varying vec2 screenPos;

uniform vec2 uOrigin;
uniform vec4 uInv;
uniform sampler2D uMarks;
uniform vec2 uMarkOrigin;
uniform vec2 uMarkSize;

vec2 toWorld(vec2 s) {
    vec2 d = s - uOrigin;
    return vec2(uInv.x * d.x + uInv.y * d.y, uInv.z * d.x + uInv.w * d.y);
}
float hash(vec2 p) { return fract(sin(dot(p, vec2(127.1, 311.7))) * 43758.5453); }

void main() {
    vec2 w  = toWorld(floor(screenPos) + 0.5);
    vec2 uv = (w - uMarkOrigin) / uMarkSize;
    if (uv.x < 0.0 || uv.y < 0.0 || uv.x > 1.0 || uv.y > 1.0) discard;

    float v = texture2D(uMarks, vec2(uv.x, 1.0 - uv.y)).r;
    if (v < 0.03) discard;

    float n = hash(floor(w));
    if (n > v * 1.35) discard;

    bool core = v > 0.55;
    vec3 col = core ? vec3(0.05, 0.045, 0.05) : vec3(0.10, 0.09, 0.10);
    gl_FragColor = vec4(col, core ? 0.62 : 0.40);
}
