#version 460 core
#pragma shader_stage(vertex)

layout(std140, set = 1, binding = 0) uniform Camera {
    mat4 projection;
};

layout(location = 0) in vec3 va_position;
layout(location = 1) in vec3 va_color;

layout(location = 0) out vec3 so_color;

void main() {
    mat4 fixedProj = mat4(
        vec4(-0.8780334, -0.46932858, -0.8021848, -0.8017837),
        vec4(1.31705, -0.31288573, -0.5347899, -0.5345225),
        vec4(0.0, 2.0337572, -0.26739496, -0.26726124),
        vec4(1.8869605e-7, 0.0, 3.6434796, 3.7416575)
    );

    vec4 clipPos = fixedProj * vec4(va_position, 1.0);

    gl_Position = clipPos / clipPos.w;

    so_color = va_color;
}
