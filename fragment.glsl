#version 330 core

in vec2 fs_position;
out vec4 FragColor;

uniform sampler2D render_texture;

vec2 Distort(vec2 point) {
	float theta = atan(point.y, point.x);
	float radius = length(point);
	radius = pow(radius, 1.08);
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
	vec4 left_pixel = texture(render_texture, vec2(position.x - 0.0008, position.y));
	vec4 right_pixel = texture(render_texture, vec2(position.x + 0.0008, position.y));

	float red = max(max(left_pixel.r, right_pixel.r), pixel.r);
	float blue = max(max(left_pixel.b, right_pixel.b), pixel.b);

	FragColor = vec4(red, pixel.g, blue, pixel.a);
}