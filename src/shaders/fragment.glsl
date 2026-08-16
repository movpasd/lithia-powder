#version 460 core
#pragma shader_stage(fragment)

layout(location = 0) in vec4 si_color;

layout(location = 0) out vec4 ca_color;

void main() {
    ca_color = si_color;
}
