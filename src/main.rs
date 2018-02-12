extern crate dcpu16_emulator as dcpu;

#[macro_use]
extern crate glium;

extern crate rand;

const WIDTH: u16 = 128;
const HEIGHT: u16 = 96;

const VERTEX_SHADER: &'static str = r#"
#version 330 core

#define WIDTH 130
#define HEIGHT 98

in vec3 color;
out vec3 fs_color;

void main() {
	fs_color = color;

	gl_Position = vec4(
		(float(gl_VertexID % WIDTH) / float(WIDTH) * 2.0) - 1.0,
		(float(gl_VertexID / WIDTH) / float(HEIGHT) * -2.0) + 1.0,
		0.0,
		1.0
	);
}
"#;


const GEOMETRY_SHADER: &'static str = r#"
#version 330 core

#define WIDTH 130.0
#define HEIGHT 98.0
#define PIXEL_WIDTH (1.0 / WIDTH * 2.0)
#define PIXEL_HEIGHT (1.0 / HEIGHT * 2.0)

layout (points) in;
layout (triangle_strip, max_vertices = 4) out;

in vec3 gs_color[];
out vec3 fs_color;

void main() {
	fs_color = gs_color[0];

	gl_Position = gl_in[0].gl_Position + vec4(0.0, 0.0, 0.0, 0.0);
	EmitVertex();
	gl_Position = gl_in[0].gl_Position + vec4(0.0, -PIXEL_HEIGHT, 0.0, 0.0);
	EmitVertex();
	gl_Position = gl_in[0].gl_Position + vec4(PIXEL_WIDTH, 0.0, 0.0, 0.0);
	EmitVertex();
	gl_Position = gl_in[0].gl_Position + vec4(PIXEL_WIDTH, -PIXEL_HEIGHT, 0.0, 0.0);
	EmitVertex();

	EndPrimitive();
}
"#;


const FRAGMENT_SHADER: &'static str = r#"
#version 330 core

in vec3 fs_color;
out vec4 FragColor;

void main() {
	FragColor = vec4(fs_color, 1.0);
}
"#;


#[derive(Copy, Clone)]
struct Pixel {
	color: (f32, f32, f32),
}
implement_vertex!(Pixel, color);

#[derive(Copy, Clone)]
struct Vertex {
	position: (f32, f32),
}
implement_vertex!(Vertex, position);

struct Lem1802 {
	// Lem State
	font: [u16; 256],
	border_color: u16,
	video_memory_location: u16,
	pallet: [u16; 16],

	// Keyboard State
	keyboard_buffer: [u16; 62],

	// OpenGL State
	events_loop: glium::glutin::EventsLoop,
	display: glium::Display,
	pixel_buffer: glium::VertexBuffer<Pixel>,
	pixel_shape_buffer: glium::VertexBuffer<Vertex>,
	indices: glium::index::NoIndices,
	program: glium::Program,
	previous_render_instant: time::Instant,
}


use glium::{glutin, Surface};
use std::time;
impl Lem1802 {
	// pub fn interrupt_keyboard(&mut self, dcpu: &mut dcpu::Dcpu) {
	// 	match dcpu.registers[dcpu::A] {
	// 		0 => self.keyboard_clear(dcpu),
	// 		// 1 => self.keyboard_get_next_key(dcpu),
	// 		// 2 => self.keyboard_is_pressed(dcpu),
	// 		// 3 => self.keyboard_set_interrupts(dcpu),
	// 		_ => (),
	// 	}
	// }


	pub fn interrupt_monitor(&mut self, dcpu: &mut dcpu::Dcpu) {
		match dcpu.registers[dcpu::A] {
			0 => self.map_memory(dcpu),
			1 => self.map_font(dcpu),
			2 => self.map_pallet(dcpu),
			3 => self.set_border_color(dcpu),
			4 => self.dump_font(dcpu),
			5 => self.dump_pallet(dcpu),
			_ => (),
		}
	}

	fn map_memory(&mut self, dcpu: &dcpu::Dcpu) {
		self.video_memory_location = dcpu.registers[dcpu::B];
	}

	fn map_font(&mut self, dcpu: &dcpu::Dcpu) {
		let location = dcpu.registers[dcpu::B] as usize;
		self.font.clone_from_slice(&dcpu.memory[location..location + 256]);
	}

	fn map_pallet(&mut self, dcpu: &dcpu::Dcpu) {
		let location = dcpu.registers[dcpu::B] as usize;
		self.pallet.clone_from_slice(&dcpu.memory[location..location + 16]);
	}

	fn set_border_color(&mut self, dcpu: &dcpu::Dcpu) {
		let border_color = dcpu.registers[dcpu::B];
		self.border_color = border_color & 0b0000_0000_0000_1111;
	}

	fn dump_font(&self, dcpu: &mut dcpu::Dcpu) {
		let location = dcpu.registers[dcpu::B] as usize;
		dcpu.memory[location..location + 256].clone_from_slice(&self.font);
	}

	fn dump_pallet(&self, dcpu: &mut dcpu::Dcpu) {
		let location = dcpu.registers[dcpu::B] as usize;
		dcpu.memory[location..location + 16].clone_from_slice(&self.pallet);
	}






	fn get_color(&self, index: usize) -> (f32, f32, f32) {
		let color = self.pallet[index];
		(
			((color & 0b0000_1111_0000_0000) >> 8) as f32 / 16.0,
			((color & 0b0000_0000_1111_0000) >> 4) as f32 / 16.0,
			((color & 0b0000_0000_0000_1111) >> 0) as f32 / 16.0,
		)
	}

	// Create a new Lem1802 with the default font, color pallet, and background color
	// Initialize and show the OpenGL window.
	pub fn new() -> Lem1802 {
		let events_loop = glium::glutin::EventsLoop::new();
		let window = glium::glutin::WindowBuilder::new()
			.with_dimensions(768, 576)
			.with_title("LEM 1802 - Low Energy Monitor - Nya Elektriska");
		let context = glium::glutin::ContextBuilder::new()
			.with_vsync(true);
		let display = glium::Display::new(window, context, &events_loop).unwrap();

		let pixel_buffer = {
			let data: [Pixel; ((WIDTH + 2) * (HEIGHT + 2)) as usize] = [Pixel {color: (0.0, 0.0, 0.0)}; ((WIDTH + 2) * (HEIGHT + 2)) as usize];
			glium::VertexBuffer::new(&display, &data).unwrap()
		};

		let pixel_shape_buffer = {
			let pixel_width = (1.0 / (WIDTH as f32) * 2.0);
			let pixel_height = (1.0 / (HEIGHT as f32) * 2.0);
			let data: [Vertex; 4] = [
				Vertex {position: (0.0, 0.0)},
				Vertex {position: (0.0, -pixel_height)},
				Vertex {position: (pixel_width, 0.0)},
				Vertex {position: (pixel_width, -pixel_height)},
			];
			glium::VertexBuffer::new(&display, &data).unwrap()
		};

		let indices = glium::index::NoIndices(glium::index::PrimitiveType::TriangleStrip);
		let program = glium::Program::from_source(&display, VERTEX_SHADER, FRAGMENT_SHADER, None).unwrap();

		Lem1802 {
			events_loop: events_loop,
			display: display,
			font: [
				0x0000, 0x0000, 0x653E, 0x3E65, 0x3E5B, 0x5B3E, 0x1E7C, 0x1E00,
				0x1C7F, 0x1C00, 0x4C73, 0x4C00, 0x5C7F, 0x5C00, 0x183C, 0x1800,
				0xE7C3, 0xE7FF, 0x1824, 0x1800, 0xE7DB, 0xE7FF, 0xE7DB, 0xE7FF,
				0x2C72, 0x2C00, 0x607F, 0x0507, 0x607F, 0x617F, 0x2A1F, 0x7C2A,
				0x7F3E, 0x1C08, 0x081C, 0x3E7F, 0x227F, 0x7F22, 0x5F00, 0x5F00,
				0x0609, 0x7F7F, 0x9AA5, 0xA559, 0x6060, 0x6060, 0xA2FF, 0xFFA2,
				0x027F, 0x7F02, 0x207F, 0x7F20, 0x1818, 0x3C18, 0x183C, 0x1818,
				0x3020, 0x2020, 0x081C, 0x1C08, 0x707E, 0x7E70, 0x0E7E, 0x7E0E,
				0x0000, 0x0000, 0x005F, 0x0000, 0x0700, 0x0700, 0x3E14, 0x3E00,
				0x266B, 0x3200, 0x611C, 0x4300, 0x6659, 0xE690, 0x0005, 0x0300,
				0x1C22, 0x4100, 0x4122, 0x1C00, 0x2A1C, 0x2A00, 0x083E, 0x0800,
				0x00A0, 0x6000, 0x0808, 0x0800, 0x0060, 0x0000, 0x601C, 0x0300,
				0x3E4D, 0x3E00, 0x427F, 0x4000, 0x6259, 0x4600, 0x2249, 0x3600,
				0x0E08, 0x7F00, 0x2745, 0x3900, 0x3E49, 0x3200, 0x6119, 0x0700,
				0x3649, 0x3600, 0x2649, 0x3E00, 0x0066, 0x0000, 0x8066, 0x0000,
				0x0814, 0x2241, 0x1414, 0x1400, 0x4122, 0x1408, 0x0259, 0x0600,
				0x3E59, 0x5E00, 0x7E09, 0x7E00, 0x7F49, 0x3600, 0x3E41, 0x2200,
				0x7F41, 0x3E00, 0x7F49, 0x4100, 0x7F09, 0x0100, 0x3E49, 0x3A00,
				0x7F08, 0x7F00, 0x417F, 0x4100, 0x2040, 0x3F00, 0x7F0C, 0x7300,
				0x7F40, 0x4000, 0x7F0E, 0x7F00, 0x7E1C, 0x7F00, 0x7F41, 0x7F00,
				0x7F09, 0x0600, 0x3E41, 0xBE00, 0x7F09, 0x7600, 0x2649, 0x3200,
				0x017F, 0x0100, 0x7F40, 0x7F00, 0x1F60, 0x1F00, 0x7F30, 0x7F00,
				0x771C, 0x7700, 0x0778, 0x0700, 0x615D, 0x4300, 0x007F, 0x4100,
				0x0618, 0x6000, 0x0041, 0x7F00, 0x0C06, 0x0C00, 0x8080, 0x8080,
				0x0003, 0x0500, 0x2454, 0x7800, 0x7F44, 0x3800, 0x3844, 0x2800,
				0x3844, 0x7F00, 0x3854, 0x5800, 0x087E, 0x0900, 0x98A4, 0x7C00,
				0x7F04, 0x7800, 0x047D, 0x0000, 0x4080, 0x7D00, 0x7F10, 0x6C00,
				0x417F, 0x4000, 0x7C18, 0x7C00, 0x7C04, 0x7800, 0x3844, 0x3800,
				0xFC24, 0x1800, 0x1824, 0xFC80, 0x7C04, 0x0800, 0x4854, 0x2400,
				0x043E, 0x4400, 0x3C40, 0x7C00, 0x1C60, 0x1C00, 0x7C30, 0x7C00,
				0x6C10, 0x6C00, 0x9CA0, 0x7C00, 0x6454, 0x4C00, 0x0836, 0x4100,
				0x0077, 0x0000, 0x4136, 0x0800, 0x0201, 0x0201, 0x704C, 0x7000,
			],
			keyboard_buffer: [0; 62],
			previous_render_instant: time::Instant::now(),
			pixel_buffer: pixel_buffer,
			pixel_shape_buffer: pixel_shape_buffer,
			indices: indices,
			program: program,
			border_color: 7,
			video_memory_location: 0x0000,
			pallet: [
				0x0000, 0x000a, 0x00a0, 0x00aa,
				0x0a00, 0x0a0a, 0x0a50, 0x0aaa,
				0x0555, 0x055f, 0x05f5, 0x05ff,
				0x0f55, 0x0f5f, 0x0ff5, 0x0fff,
			]
		}
	}




	pub fn step(&mut self, dcpu: &dcpu::Dcpu) {
		// Only update the window at most 30 times a second
		let elapsed = self.previous_render_instant.elapsed();
		if let None = time::Duration::new(0, 33333300).checked_sub(elapsed) {
			self.render(dcpu);
			self.previous_render_instant = time::Instant::now();
		}

		// self.events_loop.poll_events(|event| {
		// 	if let glium::glutin::Event::WindowEvent {event: window_event, ..} = event {
		// 		match window_event {
		// 			glium::glutin::WindowEvent::KeyboardInput {input: input} => {
		// 				use glium::glutin::VirtualKeyCode::*;
		// 				match input.virtual_keycode {
		// 					Some(Backspace) => Some(0x10),
		// 					Some(Return) => Some(0x11),
		// 					Some(Insert) => Some(0x12),
		// 					Some(Delete) => Some(0x13),
		// 					Some(Up) => Some(0x80),
		// 					Some(Down) => Some(0x81),
		// 					Some(Left) => Some(0x82),
		// 					Some(Right) => Some(0x83),
		// 					Some(Shift) => Some(0x90),
		// 					Some(Control) => Some(0x91),
		// 					_ => None,
		// 				}
		// 			},

		// 			glium::glutin::WindowEvent::CharacterInput(character) => {
		// 				Some(character as u8)
		// 			}
		// 		}
		// 	}
		// })
	}


	fn render(&mut self, dcpu: &dcpu::Dcpu) {
		// Render black if the monitor is off
		if self.video_memory_location == 0 {
			let mut target = self.display.draw();
			target.clear_color(0.0, 0.0, 0.0, 1.0);
			target.finish().unwrap();
		} else {
			// Vector to hold every pixel in this frame
			let mut frame = Vec::with_capacity((WIDTH * HEIGHT) as usize);

			// Cache the border pixel color
			let border_pixel = Pixel {
				color: self.get_color(self.border_color as usize),
			};

			// Push top border
			for _ in 0..WIDTH + 2 {
				frame.push(border_pixel)
			}

			// Loop over every pixel in the screen, computing it's color
			// and pushing it onto the frame's vector
			for y in 0..HEIGHT {
				// Push left border
				frame.push(border_pixel);

				for x in 0..WIDTH {
					// Compute the x and y character coordinates this pixel is inside of
					// (The screen is 32x12 characters, where each character is 4x8 pixels)
					let character_x = x / 4;
					let character_y = y / 8;

					// Compute the offset from video memory start where this character is located
					let memory_offset = (character_y * 32) + character_x; // The screen is 32 characters wide

					// Get the character that this pixel is inside of
					let character = dcpu.memory[memory_offset as usize + self.video_memory_location as usize];

					// Compute where in font memory this character's bitmap is stored
					let character_index = (character & 0b0000_0000_0111_1111) * 2; // Each character is 2 words long

					// Get the character's bitmap from font memory
					let character_shape = ((self.font[character_index as usize] as u32) << 16) | (self.font[character_index as usize + 1]) as u32;

					// Isolate just the pixel being drawn from the character's bitmap
					let isolated_pixel = character_shape & (0b00000001000000000000000000000000 >> (8 * (x % 4))) << (y % 8);

					// Get the forground and background colors that the character should be drawn with
					let forground_color = (character & 0b1111_0000_0000_0000) >> 12;
					let background_color = (character & 0b0000_1111_0000_0000) >> 8;

					// Push the pixel onto the frame memory
					frame.push(Pixel {
						color: if isolated_pixel == 0 {
							self.get_color(background_color as usize)
						} else {
							self.get_color(forground_color as usize)
						}
					});
				}

				// Push right border
				frame.push(border_pixel);
			}

			// Push bottom border
			for _ in 0..WIDTH + 2 {
				frame.push(border_pixel);
			}

			// Write the frame to GPU memory
			self.pixel_buffer.write(&frame);

			// Render
			let mut target = self.display.draw();
			target.draw(
				(&self.pixel_shape_buffer, self.pixel_buffer.per_instance().unwrap()),
				&self.indices,
				&self.program,
				&glium::uniforms::EmptyUniforms,
				&Default::default()
			).unwrap();
			target.finish().unwrap();
		}
	}
}




fn main() {
	let mut monitor = Lem1802::new();
	let mut dcpu = dcpu::Dcpu::new();

	dcpu.registers[dcpu::A] = 0;
	dcpu.registers[dcpu::B] = 32;
	monitor.interrupt_monitor(&mut dcpu);

	dcpu.registers[dcpu::A] = 3;
	dcpu.registers[dcpu::B] = 1;
	monitor.interrupt_monitor(&mut dcpu);

	for i in 0..256 {
		if i % 2 == 0 {
			dcpu.memory[i + 32] = 0b0011_0000_0000_0000 as u16 + (i / 2) as u16;
		}
	}

	loop {
		monitor.step(&dcpu);
	}
}