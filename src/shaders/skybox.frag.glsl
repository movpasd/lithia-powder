#version 460 core
#pragma shader_stage(fragment)

layout(std140, set = 3, binding = 0) uniform ULamp {
    vec4 fromDirection;
} uLamp;

layout(location = 0) in vec4 sWorldNormal;
layout(location = 0) out vec4 fColor;

const vec4 SKY_COLOR = vec4(0.0, 191.0, 255.0, 255.0) / 255.0;
const vec4 AUREOLE_COLOR = vec4(1.0, 1.0, 1.0, 1.0);
const float AUREOLE_ANGLE_WIDTH = radians(90.0);

void main() {

    float skyAngleWithLamp = acos(dot(normalize(sWorldNormal), uLamp.fromDirection));
    float lampAureoleIntensity;
    if (skyAngleWithLamp >= AUREOLE_ANGLE_WIDTH) {
        lampAureoleIntensity = 0.0;
    } else {
        lampAureoleIntensity = clamp(1.0 - skyAngleWithLamp / AUREOLE_ANGLE_WIDTH, 0.0, 1.0);
    }

    fColor = mix(SKY_COLOR, AUREOLE_COLOR, pow(lampAureoleIntensity, 2.0));
}
