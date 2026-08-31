#version 460 core
#pragma shader_stage(vertex)

layout(location = 0) in vec4 vWorldNormal;
layout(location = 0) out vec4 sWorldNormal;
void main() {
    sWorldNormal = vWorldNormal;
}
