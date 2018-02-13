#[macro_use]
extern crate glium;
extern crate rand;

const WIDTH: u16 = 32;
const HEIGHT: u16 = 12;


const VERTEX_SHADER: &'static str = r#"
#version 330 core

#define WIDTH 32
#define HEIGHT 12

in vec2 position;
in vec3 foreground;
in vec3 background;

out vec3 fs_foreground;
out vec3 fs_background;

void main() {
	fs_foreground = foreground;
	fs_background = background;

	gl_Position = vec4(
		(position.x + float(gl_InstanceID % WIDTH) / WIDTH) * 2.0 - 1.0,
		(position.y + float(gl_InstanceID / WIDTH) / HEIGHT) * 2.0 - 1.0,
		0.0,
		1.0
	);
}
"#;


const FRAGMENT_SHADER: &'static str = r#"
#version 330 core

in vec3 fs_foreground;
in vec3 fs_background;
out vec4 FragColor;

void main() {
	FragColor = vec4(fs_foreground, 1.0);
}
"#;




#[derive(Copy, Clone)]
struct Character {
	foreground: (f32, f32, f32),
	background: (f32, f32, f32),
}
implement_vertex!(Character, foreground, background);



#[derive(Copy, Clone)]
struct Vertex {
	position: (f32, f32),
}
implement_vertex!(Vertex, position);




fn main() {
	// Create window and OpenGL Context
	let mut events_loop = glium::glutin::EventsLoop::new();
	let window = glium::glutin::WindowBuilder::new()
		.with_dimensions(128, 96)
		.with_title("LEM 1802 - Low Energy Monitor - Nya Elektriska");
	let context = glium::glutin::ContextBuilder::new()
		.with_vsync(true);
	let display = glium::Display::new(window, context, &events_loop).unwrap();



	// Create GPU Buffers
	let font_texture = {
		let mut data: Vec<u8> = Vec::new();
		for i in 0..128 * 96 {
			for j in 0..128 {
				data.push(if j % 2 == 0 {0} else {1})
			}
		}

		let image = glium::texture::RawImage2d::from_vec_raw1d(&data);

		glium::texture::texture2d::Texture2d::new(&display, &data).unwrap()
	};


	let character_buffer = {
		let mut data: Vec<Character> = Vec::new();

		for _ in 0..WIDTH {
			for _ in 0..HEIGHT {
				data.push(Character {
					foreground: rand::random::<(f32, f32, f32)>(),
					background: rand::random::<(f32, f32, f32)>(),
				});
			}
		}

		glium::VertexBuffer::new(&display, &data).unwrap()
	};

	let character_shape_buffer = {
		let character_width = 1.0 / (WIDTH as f32);
		let character_height = 1.0 / (HEIGHT as f32);

		let data: [Vertex; 4] = [
			Vertex {position: (0.0, 0.0)},
			Vertex {position: (character_width, 0.0)},
			Vertex {position: (0.0, character_height)},
			Vertex {position: (character_width, character_height)},
		];

		glium::VertexBuffer::new(&display, &data).unwrap()
	};

	let indices = glium::index::NoIndices(glium::index::PrimitiveType::TriangleStrip);

	// Create shader program
	let program = glium::Program::from_source(&display, VERTEX_SHADER, FRAGMENT_SHADER, None).unwrap();


	// Render
	loop {
		use glium::Surface;

		let mut target = display.draw();
		target.draw(
			(&character_shape_buffer, character_buffer.per_instance().unwrap()),
			&indices,
			&program,
			&glium::uniforms::EmptyUniforms,
			&Default::default()
		).unwrap();
		target.finish().unwrap();

		events_loop.poll_events(|e| {
			match e {
				_ => ()
			}
		});

		std::thread::sleep(std::time::Duration::new(0, 13333300));
	}
}
