#version 330
in vec3 vertexPosition;
uniform mat4 mvp;
out vec2 screenPos;
void main() {
    screenPos = vertexPosition.xy;
    gl_Position = mvp * vec4(vertexPosition, 1.0);
}
