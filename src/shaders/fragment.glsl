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

void main() {
    float fullyUnlitFactor = 0.45;
    float fullyUnlitThreshold = 0.33;
    float fullyLitFactor = 1.3;
    float fullyLitThreshold = 1.0;

    float scaledLightingFactor = pow(si_lighting, 0.85);
    float slope = (fullyLitFactor - fullyUnlitFactor) / (fullyLitThreshold - fullyUnlitThreshold);
    float lightingFactorUnclamped = fullyUnlitFactor + (scaledLightingFactor - fullyUnlitThreshold) * slope;
    float lightingFactor = clamp(lightingFactorUnclamped, fullyUnlitFactor, fullyLitFactor);

    vec3 lightReflectDir = 2.0 * dot(lightDir, si_worldNormal) * si_worldNormal - lightDir;
    lightReflectDir = normalize(lightReflectDir);
    float glareAngle = acos(dot(lightReflectDir, normalize(u_cameraWorldPos.xyz - si_worldPos)));
    float glareFactor;
    if (glareAngle < radians(15.0)) {
        glareFactor = pow(1.0 - glareAngle / radians(15.0), 3.0);
    } else {
        glareFactor = 0.0;
    }

    ca_color = si_color * lightingFactor + 0.75 * glareFactor;
}
