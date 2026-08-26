#version 460 core
#pragma shader_stage(vertex)

layout(std140, set = 1, binding = 0) uniform U_Transforms {
    mat4 u_view;
    mat4 u_persp;
    mat4 u_translation;
    mat4 u_rotation;
};

layout(location = 0) in vec4 va_position;
layout(location = 1) in vec4 va_color;
layout(location = 2) in vec4 va_normal;

layout(location = 0) out vec4 so_color;
layout(location = 1) out float so_lighting;

layout(location = 2) out vec3 so_worldPos;
layout(location = 3) out vec3 so_worldNormal;

const vec3 lightDir = normalize(vec3(-3.0, 0.0, 1.0));

void main() {
    vec4 worldPos = u_translation * u_rotation * va_position;
    vec4 worldNormal = u_rotation * va_normal;

    vec4 viewPos = u_view * worldPos;
    gl_Position = u_persp * viewPos;
    so_color = va_color;

    float lightDot = dot(normalize(worldNormal.xyz), lightDir);
    so_lighting = (1.0 + lightDot) / 2.0;

    so_worldPos = worldPos.xyz;
    so_worldNormal = worldNormal.xyz;
}
