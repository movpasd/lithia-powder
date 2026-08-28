#version 460 core
#pragma shader_stage(fragment)

#define TAU 6.28318530717958647692

layout(std140, set = 3, binding = 0) uniform UCamera {
    vec4 worldPosition;
    mat4 view;
    mat4 viewPerspective;
} uCamera;

layout(location = 0) in vec4 sColor;
layout(location = 1) in float sLampIllumination;
layout(location = 2) in vec4 sWorldPosition;
layout(location = 3) in vec4 sWorldNormal;

layout(location = 0) out vec4 fColor;

const vec3 lightDir = normalize(vec3(-3.0, 0.0, 1.0));


const float AMBIENT_ILLUMINATION = 0.33;
const float FULLY_LIT_MULTIPLIER = 1.4;

const float GLARE_ANGLE = radians(15.0);
const float GLARE_MULTIPLIER = 0.75;

void main() {

    vec4 uLamp_fromDirection = vec4(lightDir, 0.0);

    // ---

    float finalIllumination = max(AMBIENT_ILLUMINATION, sLampIllumination);
    float lightingMultiplier = FULLY_LIT_MULTIPLIER * pow(finalIllumination, 0.85);

    vec4 lampReflectionDir =
        2.0 * dot(uLamp_fromDirection, sWorldNormal) * sWorldNormal
        - uLamp_fromDirection;
    vec4 fragToCameraDir = normalize(uCamera.worldPosition - sWorldPosition);
    float angleFromReflection = acos(dot(lampReflectionDir, fragToCameraDir));
    float glare = max(0.0, 1.0 - angleFromReflection / GLARE_ANGLE);

    fColor = sColor * lightingMultiplier + GLARE_MULTIPLIER * glare;
}
