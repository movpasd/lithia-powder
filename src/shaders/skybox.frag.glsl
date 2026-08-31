#version 460 core
#pragma shader_stage(fragment)

layout(std140, set = 3, binding = 0) uniform ULamp {
    vec4 fromDirection;
} uLamp;

layout(location = 0) in vec4 sWorldNormal;
layout(location = 0) out vec4 fColor;

void main() {
    float lampFrac = (1.0 + dot(normalize(sWorldNormal), uLamp.fromDirection)) / 2.0;

    vec4 darkColor = vec4(1.0, 1.0, 1.0, 1.0);
    vec4 lightColor = vec4(0.67, 0.85, 0.90, 1.0);

    fColor = mix(darkColor, lightColor, lampFrac);
}
