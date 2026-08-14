#version 460 core
#pragma shader_stage(fragment)

layout(location = 0) in vec3 si_color;

layout(location = 0) out vec4 ca_color;

void main() {
    ca_color = vec4(pow(si_color, vec3(0.33, 0.33, 0.33)), 1.0);
}
