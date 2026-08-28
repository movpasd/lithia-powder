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
    // vec4 uCamera_worldPosition = ;
    mat4 uCamera_view = u_view;
    mat4 uCamera_viewPerspective = u_persp * u_view;
    vec4 uLamp_fromDirection = vec4(lightDir, 0.0);
    mat4 uPose_transform = u_translation * u_rotation;

    vec4 vModelPosition = va_position;
    vec4 vModelNormal = va_normal;
    vec4 vColor = va_color;
    vec4 sColor;
    float sLampIllumination;
    vec4 sWorldPosition;
    vec4 sWorldNormal;

    // ---

    sColor = vColor;

    vec4 worldPosition = uPose_transform * vModelPosition;
    vec4 worldNormal = uPose_transform * vModelNormal;
    vec4 clipPosition = uCamera_viewPerspective * worldPosition;
    gl_Position = clipPosition;
    sWorldPosition = worldPosition;
    sWorldNormal = worldNormal;

    // nb: worldNormal and uLamp.dir assumed to be normalized and w=0
    sLampIllumination = (1.0 + dot(worldNormal, uLamp_fromDirection)) / 2.0;

    // ---

    so_color = sColor;
    so_lighting = sLampIllumination;
    so_worldPos = sWorldPosition.xyz;
    so_worldNormal = sWorldNormal.xyz;
}
