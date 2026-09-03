#version 460 core
#pragma shader_stage(fragment)

layout(std140, set = 3, binding = 0) uniform UEyeball {
    vec4 worldPosition;
    mat4 view;
    mat4 viewPerspective;
} uEyeball;
layout(std140, set = 3, binding = 1) uniform ULamp {
    vec4 fromDirection;
} uLamp;

layout(location = 0) in vec4 sColor;
layout(location = 1) in float sLampIllumination;
layout(location = 2) in vec4 sWorldPosition;
layout(location = 3) in vec4 sWorldNormal;
layout(location = 4) in float sCornerOcclusion;

layout(location = 0) out vec4 fColor;


const float AMBIENT_ILLUMINATION = 0.5;
const float FULL_ILLUMINATION_THRESHOLD = 0.85;
const float FULLY_LIT_MULTIPLIER = 1.0;

const float HIGHLIGHT_ANGLE_WIDTH = radians(15.0);
const float HIGHLIGHT_MULTIPLIER = 0.4;

void main() {
    float finalIllumination = clamp(sLampIllumination / FULL_ILLUMINATION_THRESHOLD, AMBIENT_ILLUMINATION, 1.0);
    float lightingMultiplier = FULLY_LIT_MULTIPLIER * pow(finalIllumination, 0.67);

    vec4 lampReflectionDir =
        2.0 * dot(uLamp.fromDirection, sWorldNormal) * sWorldNormal
        - uLamp.fromDirection;
    vec4 fragToEyeballDir = normalize(uEyeball.worldPosition - sWorldPosition);
    float angleFromSpecularHighlight = acos(dot(lampReflectionDir, fragToEyeballDir));
    float highlightIntensity = max(0.0, 1.0 - angleFromSpecularHighlight / HIGHLIGHT_ANGLE_WIDTH);
    highlightIntensity = pow(highlightIntensity, 3.0);

    float cornerOcclusionMultiplier = pow(1.0 - sCornerOcclusion, 2.0);

    fColor = sColor * lightingMultiplier * cornerOcclusionMultiplier + HIGHLIGHT_MULTIPLIER * highlightIntensity;
}
