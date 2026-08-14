#version 460 core
#pragma shader_stage(vertex)

layout(location = 0) in vec3 va_position;
layout(location = 1) in vec3 va_color;

layout(location = 0) out vec3 so_color;

void main() {
    gl_Position = vec4(va_position, 1.0);
    so_color = va_color;
}
