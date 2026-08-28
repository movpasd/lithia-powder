#version 460 core
#pragma shader_stage(vertex)

layout(std140, set = 1, binding = 0) uniform U_Transforms {
    mat4 u_view;
    mat4 u_persp;
    mat4 u_translation;
    mat4 u_rotation;
};

layout(location = 0) in vec4 vModelPosition;
layout(location = 1) in vec4 vModelNormal;
layout(location = 2) in vec4 vColor;

layout(location = 0) out vec4 sColor;
layout(location = 1) out float sLampIllumination;
layout(location = 2) out vec4 sWorldPosition;
layout(location = 3) out vec4 sWorldNormal;

const vec3 lightDir = normalize(vec3(-3.0, 0.0, 1.0));

void main() {
    // vec4 uCamera_worldPosition = ;
    mat4 uCamera_view = u_view;
    mat4 uCamera_viewPerspective = u_persp * u_view;
    vec4 uLamp_fromDirection = vec4(lightDir, 0.0);
    mat4 uPose_transform = u_translation * u_rotation;

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
}
