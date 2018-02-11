extern crate dcpu16_emulator as dcpu;

#[macro_use]
extern crate glium;

extern crate rand;



const VERTEX_SHADER: &'static str = r#"
#version 330 core

#define WIDTH 128
#define HEIGHT 96

in vec2 position;
in vec3 color;

out vec3 frag_color;

void main() {
	frag_color = color;

	gl_Position = vec4(
		position.x + (float(gl_InstanceID % WIDTH) / float(WIDTH)),
		position.y + (float(gl_InstanceID / WIDTH) / float(HEIGHT)),
		0.0,
		1.0
	);
}
"#;


const FRAGMENT_SHADER: &'static str = r#"
#version 330 core

in vec3 frag_color;
out vec4 FragColor;

void main() {
	FragColor = vec4(frag_color, 1.0);
}
"#;

#[derive(Copy, Clone)]
struct Vertex {
	position: (f32, f32),
}
implement_vertex!(Vertex, position);

#[derive(Copy, Clone)]
struct Pixel {
	color: (f32, f32, f32),
}
implement_vertex!(Pixel, color);


fn main() {
	use glium::{glutin, Surface};
	use std::time;

	let mut events_loop = glium::glutin::EventsLoop::new();
	let window = glium::glutin::WindowBuilder::new()
		.with_dimensions(128, 96)
		.with_title("LEM 1802 - Low Energy Monitor - Nya Elektriska");
	let context = glium::glutin::ContextBuilder::new()
		.with_vsync(true);
	let display = glium::Display::new(window, context, &events_loop).unwrap();


	let shape_buffer =
		glium::VertexBuffer::new(&display, &vec![
			Vertex {position: (0.0, 1.0)},
			Vertex {position: (1.0, 1.0)},
			Vertex {position: (0.0, 0.0)},
			Vertex {position: (1.0, 0.0)},
		]).unwrap();

	let image_buffer = {
		let mut image_data = Vec::with_capacity(128 * 96);
		for i in 0..128 {
			for j in 0..96 {
				image_data.push(Pixel {
					color: rand::random::<(f32, f32, f32)>(),
				});
			}
		}

		glium::VertexBuffer::dynamic(&display, &image_data).unwrap()
	};


	let indices = glium::index::NoIndices(glium::index::PrimitiveType::TriangleStrip);
	let program = glium::Program::from_source(&display, VERTEX_SHADER, FRAGMENT_SHADER, None).unwrap();


	let frame_length = time::Duration::new(0, 33333300);
	let mut previous_instant = time::Instant::now();
	let mut closed = false;
	while !closed {
		let elapsed = previous_instant.elapsed();
		if let Some(difference) = frame_length.checked_sub(elapsed) {
			std::thread::sleep(difference);
		}
		previous_instant = time::Instant::now();

		events_loop.poll_events(|ev| {
			match ev {
				glutin::Event::WindowEvent {event, ..} => match event {
					glutin::WindowEvent::Closed => closed = true,
					_ => (),
				},
				_ => (),
			}
		});

		let mut target = display.draw();
		target.clear_color(0.0, 0.0, 1.0, 1.0);
		target.draw(
			(&shape_buffer, image_buffer.per_instance().unwrap()),
			&indices,
			&program,
			&glium::uniforms::EmptyUniforms,
			&Default::default()
		).unwrap();
		target.finish().unwrap();
	}
}