#version 460 core
#pragma shader_stage(fragment)

layout(location = 0) in vec4 si_color;
layout(location = 1) in float si_lighting;

layout(location = 0) out vec4 ca_color;

void main() {
    float fullyUnlitFactor = 0.7;
    float fullyUnlitThreshold = 0.4;
    float fullyLitFactor = 1.5;
    float fullyLitThreshold = 1.0;

    float slope = (fullyLitFactor - fullyUnlitFactor) / (fullyLitThreshold - fullyUnlitThreshold);
    float lightingFactorUnclamped = fullyUnlitFactor + (si_lighting - fullyUnlitThreshold) * slope;
    float lightingFactor = clamp(lightingFactorUnclamped, fullyUnlitFactor, fullyLitFactor);

    ca_color = si_color * lightingFactor;
}
