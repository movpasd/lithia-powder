#version 460 core
#pragma shader_stage(vertex)

layout(std140, set = 1, binding = 0) uniform UCamera {
    vec4 worldPosition;
    mat4 view;
    mat4 viewPerspective;
} uCamera;
layout(std140, set = 1, binding = 1) uniform ULamp {
    vec4 fromDirection;
} uLamp;
layout(std140, set = 1, binding = 2) uniform UPose {
    mat4 transform;
} uPose;

const uint MAX_MESHES = 1024;
layout(std140, set = 0, binding = 0) buffer BMeshData {
    mat4 poseTransforms[MAX_MESHES];
} bMeshData;

layout(location = 0) in vec4 vModelPosition;
layout(location = 1) in vec4 vModelNormal;
layout(location = 2) in vec4 vColor;
layout(location = 3) in uint vMeshId;

layout(location = 0) out vec4 sColor;
layout(location = 1) out float sLampIllumination;
layout(location = 2) out vec4 sWorldPosition;
layout(location = 3) out vec4 sWorldNormal;


void main() {
    sColor = vColor;

    vec4 worldPosition = uPose.transform * vModelPosition;
    vec4 worldNormal = uPose.transform * vModelNormal;
    vec4 clipPosition = uCamera.viewPerspective * worldPosition;
    gl_Position = clipPosition;
    sWorldPosition = worldPosition;
    sWorldNormal = worldNormal;

    // nb: worldNormal and uLamp.fromDirection assumed to be normalized and w=0
    sLampIllumination = (1.0 + dot(worldNormal, uLamp.fromDirection)) / 2.0;
}
