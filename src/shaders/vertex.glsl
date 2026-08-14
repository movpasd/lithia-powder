#version 460 core
#pragma shader_stage(vertex)

layout(std140, set = 1, binding = 0) uniform Camera {
    mat4 projection;
};

layout(location = 0) in vec3 va_position;
layout(location = 1) in vec3 va_color;

layout(location = 0) out vec3 so_color;

void main() {
    gl_Position = projection * vec4(va_position, 1.0);
    so_color = va_color;
}
