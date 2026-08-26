#version 460 core
#pragma shader_stage(vertex)

layout(std140, set = 1, binding = 0) uniform U_Transforms {
    mat4 projection;
    mat4 translation;
    mat4 rotation;
};

layout(location = 0) in vec4 va_position;
layout(location = 1) in vec4 va_color;
layout(location = 2) in vec4 va_normal;

layout(location = 0) out vec4 so_color;
layout(location = 1) out float so_lighting;

void main() {
    vec4 worldPos = rotation * translation * va_position;
    vec4 worldNormal = rotation * translation * va_normal;

    gl_Position = projection * worldPos;
    so_color = va_color;

    vec3 lightDir = normalize(vec3(2.0, -3.0, 8.0));
    so_lighting = (1.0 + dot(normalize(worldNormal.xyz), lightDir)) / 2.0;
}
