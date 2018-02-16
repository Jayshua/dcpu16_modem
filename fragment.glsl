#version 330 core

in vec2 fs_position;
out vec4 FragColor;

uniform sampler2D render_texture;

vec2 Distort(vec2 point) {
	float theta = atan(point.y, point.x);
	float radius = length(point);
	radius = pow(radius, 1.13);
	point.x = radius * cos(theta);
	point.y = radius * sin(theta);
	return 0.5 * (point + 1.0);
}

vec4 scanline(vec4 color, vec2 position) {
	return color * sin(float(int(position.y * 480) % 8) / 4);
}

void main() {
	vec2 position = Distort(fs_position.xy * 2.0 - 1.0);

	vec4 pixel = texture(render_texture, position);
	vec4 left = texture(render_texture, vec2(position.x - 0.003, position.y));
	vec4 right = texture(render_texture, vec2(position.x + 0.002, position.y));

	FragColor = vec4(left.r, pixel.g, right.b, left.a);
}