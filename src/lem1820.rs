use glium;
use dcpu;

pub struct Lem1820 {
	// Dcpu State
	font_ram: [u16; 256],
	pallet_ram: [u16; 16],
	video_ram: u16,
	border_color: u16,

	// Window State
	events_loop: glium::glutin::EventsLoop,
	display: glium::Display,

	// OpenGL State
	font_texture: glium::texture::texture2d::Texture2d,
	character_buffer: glium::VertexBuffer<Character>,
	character_shape_buffer: glium::VertexBuffer<Vertex>,
	character_shape_indicies: glium::index::NoIndices,
	shader_program: glium::Program,
	render_buffer: glium::texture::texture2d::Texture2d,
	post_process_buffer: glium::VertexBuffer<Vertex>,
	post_process_indicies: glium::index::NoIndices,
	post_process_shader: glium::Program,
}




impl Lem1820 {
	pub fn new() -> Lem1820 {
		// Create window and OpenGL Context
		let mut events_loop = glium::glutin::EventsLoop::new();
		let window = glium::glutin::WindowBuilder::new()
			.with_dimensions(640, 480)
			.with_title("LEM 1802 - Low Energy Monitor - Nya Elektriska");
		let context = glium::glutin::ContextBuilder::new()
			.with_vsync(true);
		let display = glium::Display::new(window, context, &events_loop).unwrap();


		let font_texture = create_font_texture(&display, &DEFAULT_FONT);

		let character_buffer = glium::VertexBuffer::empty_dynamic(&display, (WIDTH * HEIGHT) as usize).unwrap();

		let character_shape_buffer = glium::VertexBuffer::new(&display, &[
			Vertex {position: (0.0, 0.0)},
			Vertex {position: (1.0, 0.0)},
			Vertex {position: (0.0, 1.0)},
			Vertex {position: (1.0, 1.0)},
		]).unwrap();

		let character_shape_indicies = glium::index::NoIndices(glium::index::PrimitiveType::TriangleStrip);

		let shader_program = glium::Program::from_source(&display, VERTEX_SHADER, FRAGMENT_SHADER, None).unwrap();

		let render_buffer = glium::texture::texture2d::Texture2d::empty(&display, 640, 480).unwrap();

		let post_process_buffer = glium::VertexBuffer::new(&display, &[
			Vertex {position: (-1.0, -1.0)},
			Vertex {position: (1.0, -1.0)},
			Vertex {position: (-1.0, 1.0)},
			Vertex {position: (1.0, 1.0)},
		]).unwrap();

		let post_process_indicies = glium::index::NoIndices(glium::index::PrimitiveType::TriangleStrip);

		let post_process_shader = load_shader(&display).unwrap();

		Lem1820 {
			font_ram: DEFAULT_FONT,
			pallet_ram: DEFAULT_PALLET,
			video_ram: 0,
			border_color: 7,

			events_loop: events_loop,
			display: display,

			font_texture: font_texture,
			character_buffer: character_buffer,
			character_shape_buffer: character_shape_buffer,
			character_shape_indicies: character_shape_indicies,
			shader_program: shader_program,
			render_buffer: render_buffer,
			post_process_buffer: post_process_buffer,
			post_process_indicies: post_process_indicies,
			post_process_shader: post_process_shader,
		}
	}

	pub fn interrupt(&mut self, dcpu: &mut dcpu::Dcpu) {
		match dcpu.registers[dcpu::A] {
			0 => self.mem_map_screen(dcpu),
			1 => self.mem_map_font(dcpu),
			2 => self.mem_map_pallet(dcpu),
			3 => self.set_border_color(dcpu),
			4 => self.mem_dump_font(dcpu),
			5 => self.mem_dump_pallet(dcpu),
			_ => (),
		}
	}

	pub fn step(&mut self, dcpu: &mut dcpu::Dcpu) {
		use glium::Surface;

		if self.video_ram != 0 {
			let mut character_data: Vec<Character> = Vec::new();
			let border_character = Character {
				character: 0,
				foreground: to_float_color(&self.pallet_ram, self.border_color as usize),
				background: to_float_color(&self.pallet_ram, self.border_color as usize),
			};

			// Push top border
			for _ in 0..WIDTH {
				character_data.push(border_character);
			}

			for y in 0..HEIGHT - 2 {
				// Push left border
				character_data.push(border_character);

				for x in 0..WIDTH - 2 {
					let character_index = (self.video_ram + x + (y * WIDTH)) as usize;
					let character_value = dcpu.memory[character_index];
					let character = (character_value & 0b0000_0000_0111_1111) as u8;
					let foreground = (character_value & 0b1111_0000_0000_0000) >> 12;
					let background = (character_value & 0b0000_1111_0000_0000) >> 8;

					character_data.push(Character {
						character: character,
						foreground: to_float_color(&self.pallet_ram, foreground as usize),
						background: to_float_color(&self.pallet_ram, background as usize),
					});
				}

				// Push right border
				character_data.push(border_character);
			}

			// Push bottom border
			for _ in 0..WIDTH {
				character_data.push(border_character);
			}

			self.character_buffer.write(&character_data);

			let mut surface = self.render_buffer.as_surface();
			surface.clear_color(0.0, 0.0, 0.0, 1.0);
			surface.draw(
				(&self.character_shape_buffer, self.character_buffer.per_instance().unwrap()),
				&self.character_shape_indicies,
				&self.shader_program,
				&uniform! {
					font_texture: glium::uniforms::Sampler::new(&self.font_texture)
						.minify_filter(glium::uniforms::MinifySamplerFilter::Nearest)
						.magnify_filter(glium::uniforms::MagnifySamplerFilter::Nearest)
				},
				&Default::default()
			).unwrap();

			let mut target = self.display.draw();
			target.draw(
				&self.post_process_buffer,
				&self.post_process_indicies,
				&self.post_process_shader,
				&uniform! {
					render_texture: glium::uniforms::Sampler::new(&self.render_buffer)
						.wrap_function(glium::uniforms::SamplerWrapFunction::Clamp),
				},
				&Default::default(),
			).unwrap();
			target.finish().unwrap();
		}

		let mut reload_shader = false;
		self.events_loop.poll_events(|e| {
			match e {
				glium::glutin::Event::WindowEvent {event, ..} =>
					match event {
						glium::glutin::WindowEvent::KeyboardInput {..} => {
							reload_shader = true;
						},

						_ => (),
					},

				_ => ()
			}
		});

		if reload_shader {
			if let Some(new_shader) = load_shader(&self.display) {
				self.post_process_shader = new_shader;
			}
		}
	}

	fn mem_map_screen(&mut self, dcpu: &mut dcpu::Dcpu) {
		self.video_ram = dcpu.registers[dcpu::B];
	}

	fn mem_map_font(&mut self, dcpu: &mut dcpu::Dcpu) {
		let ram_begin = dcpu.registers[dcpu::B] as usize;
		let ram_end = ram_begin + 384;
		self.font_texture = create_font_texture(&self.display, &dcpu.memory[ram_begin..ram_end]);
	}

	fn mem_map_pallet(&mut self, dcpu: &mut dcpu::Dcpu) {
		let ram_begin = dcpu.registers[dcpu::B] as usize;
		let ram_end = ram_begin + 16;
		self.pallet_ram.clone_from_slice(&dcpu.memory[ram_begin..ram_end]);
	}

	fn set_border_color(&mut self, dcpu: &mut dcpu::Dcpu) {
		self.border_color = dcpu.registers[dcpu::B] & 0xf;
	}

	fn mem_dump_font(&mut self, dcpu: &mut dcpu::Dcpu) {
		let ram_begin = dcpu.registers[dcpu::B] as usize;
		let ram_end = ram_begin + 384;
		dcpu.memory[ram_begin..ram_end].clone_from_slice(&self.pallet_ram);
	}

	fn mem_dump_pallet(&mut self, dcpu: &mut dcpu::Dcpu) {
		let ram_begin = dcpu.registers[dcpu::B] as usize;
		let ram_end = ram_begin + 16;
		dcpu.memory[ram_begin..ram_end].clone_from_slice(&self.pallet_ram);
	}
}



// Get the color in the pallet indicated by the given DCPU word as an OpenGL color
fn to_float_color(pallet: &[u16; 16], index: usize) -> (f32, f32, f32) {
	let red   = (pallet[index] & 0b0000_1111_0000_0000) >> 8;
	let green = (pallet[index] & 0b0000_0000_1111_0000) >> 4;
	let blue  = (pallet[index] & 0b0000_0000_0000_1111) >> 0;
	(
		(red as f32) / 16.0,
		(green as f32) / 16.0,
		(blue as f32) / 16.0,
	)
}


// Load the shader from disk
fn load_shader(display: &glium::Display) -> Option<glium::Program> {
	use std::io::prelude::*;
	use std::fs::File;

	let mut vertex_shader_file = File::open("vertex.glsl").unwrap();
	let mut vertex_shader_source = String::new();
	vertex_shader_file.read_to_string(&mut vertex_shader_source).unwrap();

	let mut fragment_shader_file = File::open("fragment.glsl").unwrap();
	let mut fragment_shader_source = String::new();
	fragment_shader_file.read_to_string(&mut fragment_shader_source).unwrap();

	match glium::Program::from_source(display, vertex_shader_source.as_str(), fragment_shader_source.as_str(), None) {
		Ok(program) =>
			Some(program),

		Err(err) => {
			println!("{:?}", err);
			None
		},
	}
}




// Upload the given font data to the GPU
fn create_font_texture(display: &glium::Display, font: &[u16]) -> glium::texture::texture2d::Texture2d {
	// Create a one-dimensional array representing a two dimensional texture
	// which will store 255 for marked pixels and 0 for unmarked pixels.
	// The characters are laid out side by side, 8 pixels high by 512 pixels
	// wide which is enough for the 128 character font.
	let mut data: [u8; 8 * 512] = [0; 8 * 512];

	// Loop over each character in the font, setting the pixels
	// in the data image to match.
	for i in 0..128 {
		let character_index = (i * 2) as usize;
		let character = ((font[character_index] as u32) << 16) + (font[character_index + 1] as u32);

		for j in 0..32 {
			let marked = character & (0x8000_0000u32 >> j) > 0;
			let x = (j / 8) + (i * 4);
			let y = j % 8;
			if marked {
				data[(y * 512) + x] = 255;
			}
		}
	}

	// Convert the data array into a vector containing 3 values
	// for each pixel - RGB, but we only use the red channel.
	let mut pixel_data: Vec<u8> = Vec::new();
	for point in data.iter() {
		pixel_data.push(*point);
		pixel_data.push(0);
		pixel_data.push(0);
	}

	glium::texture::texture2d::Texture2d::new(
		display,
		glium::texture::RawImage2d::from_raw_rgb_reversed(&pixel_data, (512, 8))
	).unwrap()
}


const WIDTH: u16 = 34;
const HEIGHT: u16 = 14;


const VERTEX_SHADER: &'static str = r#"
#version 330 core

#define WIDTH 34
#define HEIGHT 14

in vec2 position;
in uint character;
in vec3 foreground;
in vec3 background;

out vec3 fs_foreground;
out vec3 fs_background;
out vec2 fs_position;
flat out uint fs_character;

void main() {
	fs_foreground = foreground;
	fs_background = background;
	fs_position = position;
	fs_character = character;

	gl_Position = vec4(
		( (float(position.x) / WIDTH) + (float(gl_InstanceID % WIDTH) / WIDTH) ) * 2.0 - 1.0,
		( (float(position.y) / HEIGHT) + (float(gl_InstanceID / WIDTH) / HEIGHT) ) * -2.0 + 1.0,
		0.0,
		1.0
	);
}
"#;


const FRAGMENT_SHADER: &'static str = r#"
#version 330 core

in vec3 fs_foreground;
in vec3 fs_background;
in vec2 fs_position;
flat in uint fs_character;
out vec4 FragColor;

uniform sampler2D font_texture;

void main() {
	vec2 texture_position = vec2(
		(fs_position.x / 128.0) + float(fs_character) / 128.0,
		fs_position.y
	);

	if (texture(font_texture, texture_position).r > 0) {
		FragColor = vec4(fs_foreground, 1.0);
	} else {
		FragColor = vec4(fs_background, 1.0);
	}
}
"#;


const DEFAULT_PALLET: [u16; 16] = [
	0x000, 0x00a, 0x0a0, 0x0aa, 0xa00, 0xa0a, 0xa50, 0xaaa,
	0x555, 0x55f, 0x5f5, 0x5ff, 0xf55, 0xf5f, 0xff5, 0xfff,
];


const DEFAULT_FONT: [u16; 256] = [
	0x000f, 0x0808, 0x080f, 0x0808, 0x08f8, 0x0808, 0x00ff, 0x0808,
	0x0808, 0x0808, 0x08ff, 0x0808, 0x00ff, 0x1414, 0xff00, 0xff08,
	0x1f10, 0x1714, 0xfc04, 0xf414, 0x1710, 0x1714, 0xf404, 0xf414,
	0xff00, 0xf714, 0x1414, 0x1414, 0xf700, 0xf714, 0x1417, 0x1414,
	0x0f08, 0x0f08, 0x14f4, 0x1414, 0xf808, 0xf808, 0x0f08, 0x0f08,
	0x001f, 0x1414, 0x00fc, 0x1414, 0xf808, 0xf808, 0xff08, 0xff08,
	0x14ff, 0x1414, 0x080f, 0x0000, 0x00f8, 0x0808, 0xffff, 0xffff,
	0xf0f0, 0xf0f0, 0xffff, 0x0000, 0x0000, 0xffff, 0x0f0f, 0x0f0f,
	0x0000, 0x0000, 0x005f, 0x0000, 0x0300, 0x0300, 0x3e14, 0x3e00,
	0x266b, 0x3200, 0x611c, 0x4300, 0x3629, 0x7650, 0x0002, 0x0100,
	0x1c22, 0x4100, 0x4122, 0x1c00, 0x2a1c, 0x2a00, 0x083e, 0x0800,
	0x4020, 0x0000, 0x0808, 0x0800, 0x0040, 0x0000, 0x601c, 0x0300,
	0x3e41, 0x3e00, 0x427f, 0x4000, 0x6259, 0x4600, 0x2249, 0x3600,
	0x0f08, 0x7f00, 0x2745, 0x3900, 0x3e49, 0x3200, 0x6119, 0x0700,
	0x3649, 0x3600, 0x2649, 0x3e00, 0x0024, 0x0000, 0x4024, 0x0000,
	0x0814, 0x2241, 0x1414, 0x1400, 0x4122, 0x1408, 0x0259, 0x0600,
	0x3e59, 0x5e00, 0x7e09, 0x7e00, 0x7f49, 0x3600, 0x3e41, 0x2200,
	0x7f41, 0x3e00, 0x7f49, 0x4100, 0x7f09, 0x0100, 0x3e49, 0x3a00,
	0x7f08, 0x7f00, 0x417f, 0x4100, 0x2040, 0x3f00, 0x7f0c, 0x7300,
	0x7f40, 0x4000, 0x7f06, 0x7f00, 0x7f01, 0x7e00, 0x3e41, 0x3e00,
	0x7f09, 0x0600, 0x3e41, 0xbe00, 0x7f09, 0x7600, 0x2649, 0x3200,
	0x017f, 0x0100, 0x7f40, 0x7f00, 0x1f60, 0x1f00, 0x7f30, 0x7f00,
	0x7708, 0x7700, 0x0778, 0x0700, 0x7149, 0x4700, 0x007f, 0x4100,
	0x031c, 0x6000, 0x0041, 0x7f00, 0x0201, 0x0200, 0x8080, 0x8000,
	0x0001, 0x0200, 0x2454, 0x7800, 0x7f44, 0x3800, 0x3844, 0x2800,
	0x3844, 0x7f00, 0x3854, 0x5800, 0x087e, 0x0900, 0x4854, 0x3c00,
	0x7f04, 0x7800, 0x447d, 0x4000, 0x2040, 0x3d00, 0x7f10, 0x6c00,
	0x417f, 0x4000, 0x7c18, 0x7c00, 0x7c04, 0x7800, 0x3844, 0x3800,
	0x7c14, 0x0800, 0x0814, 0x7c00, 0x7c04, 0x0800, 0x4854, 0x2400,
	0x043e, 0x4400, 0x3c40, 0x7c00, 0x1c60, 0x1c00, 0x7c30, 0x7c00,
	0x6c10, 0x6c00, 0x4c50, 0x3c00, 0x6454, 0x4c00, 0x0836, 0x4100,
	0x0077, 0x0000, 0x4136, 0x0800, 0x0201, 0x0201, 0x704c, 0x7000,
];




#[derive(Copy, Clone)]
struct Character {
	character: u8,
	foreground: (f32, f32, f32),
	background: (f32, f32, f32),
}
implement_vertex!(Character, character, foreground, background);



#[derive(Copy, Clone)]
struct Vertex {
	position: (f32, f32),
}
implement_vertex!(Vertex, position);

