#version 460 core
#pragma shader_stage(fragment)

#define TAU 6.28318530717958647692

layout(std140, set = 3, binding = 0) uniform U_Camera {
    vec4 u_cameraWorldPos;
};

layout(location = 0) in vec4 si_color;
layout(location = 1) in float si_lighting;

layout(location = 2) in vec3 si_worldPos;
layout(location = 3) in vec3 si_worldNormal;

layout(location = 0) out vec4 ca_color;

const vec3 lightDir = normalize(vec3(-3.0, 0.0, 1.0));


const float AMBIENT_ILLUMINATION = 0.33;
const float FULLY_LIT_MULTIPLIER = 1.4;

const float GLARE_ANGLE = radians(15.0);
const float GLARE_MULTIPLIER = 0.75;

void main() {

    vec4 uCamera_worldPosition = u_cameraWorldPos;
    // mat4 uCamera_view = ;
    // mat4 uCamera_viewPerspective = ;
    vec4 uLamp_fromDirection = vec4(lightDir, 0.0);

    vec4 sColor = si_color;
    float sLampIllumination = si_lighting;
    vec4 sWorldPosition = vec4(si_worldPos, 1.0);
    vec4 sWorldNormal = vec4(si_worldNormal, 0.0);

    vec4 fColor;

    // ---

    float finalIllumination = max(AMBIENT_ILLUMINATION, sLampIllumination);
    float lightingMultiplier = FULLY_LIT_MULTIPLIER * pow(finalIllumination, 0.85);

    vec4 lampReflectionDir =
        2.0 * dot(uLamp_fromDirection, sWorldNormal) * sWorldNormal
        - uLamp_fromDirection;
    vec4 fragToCameraDir = normalize(uCamera_worldPosition - sWorldPosition);
    float angleFromReflection = acos(dot(lampReflectionDir, fragToCameraDir));
    float glare = max(0.0, 1.0 - angleFromReflection / GLARE_ANGLE);

    fColor = sColor * lightingMultiplier + GLARE_MULTIPLIER * glare;

    // ---

    ca_color = fColor;
}
