#version 330 core

in vec2 position;
out vec2 fs_position;

void main() {
	fs_position = position;
	gl_Position = vec4(position * 2.0 - 1.0, 0.0, 1.0);
}