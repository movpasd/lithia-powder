#version 460 core
#pragma shader_stage(fragment)

layout(location = 0) in vec4 si_color;
layout(location = 1) in float si_lighting;

layout(location = 0) out vec4 ca_color;

void main() {
    float lightingFactor = pow(si_lighting, 0.5);
    ca_color = si_color * lightingFactor;
}
