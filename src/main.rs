#[macro_use]
extern crate glium;
#[macro_use]
extern crate serde_derive;
extern crate docopt;

mod dcpu;
mod modem;
mod lem1820;

const USAGE: &'static str = "
dcpu

Usage:
	dcpu start <image> [-l | -lem1820] [-e | --eklectic] [-k | --keyboard]
	dcpu assemble <file> [-o <outfile> | --output <outfile>]

Options:
	-l, --lem1820     Attach an LEM1820 Monitor
	-e, --eklectic    Attach a Tesla Eklectic Modem
	-k, --keyboard    Attach a generic keyboard
	-o, --output      Set the file to output the assembled image to
";

#[derive(Debug, Deserialize)]
struct Arguments {
	flag_lem1820: bool,
	flag_eklectic: bool,
	flag_keyboard: bool,
	cmd_start: bool,
	cmd_assemble: bool,
	arg_image: Option<String>,
	arg_file: Option<String>,
	arg_outfile: Option<String>,
}



use std::collections::VecDeque;

struct Keyboard {
	events_loop: glium::glutin::EventsLoop,
	keyboard_buffer: VecDeque<u16>,
	keyboard_interrupt: u16,
	last_refresh: std::time::Instant,
}


impl Keyboard {
	fn new(events_loop: glium::glutin::EventsLoop) -> Keyboard {
		Keyboard {
			events_loop: events_loop,
			keyboard_buffer: VecDeque::new(),
			keyboard_interrupt: 0,
			last_refresh: std::time::Instant::now(),
		}
	}


	fn step(&mut self, dcpu: &mut dcpu::Dcpu) {
		if let None = self.last_refresh.elapsed().checked_sub(std::time::Duration::new(0, 50_000_000)) {
			return;
		}


		let mut character = None;
		self.events_loop.poll_events(|e| {
			match e {
				glium::glutin::Event::WindowEvent {event, ..} =>
					match event {
						glium::glutin::WindowEvent::ReceivedCharacter(c) => {
							if c.is_ascii() {
								let converted: u8 = c as u8;

								if converted >= 0x20 && converted < 0x7f {
									character = Some(converted as u16);
								}
							}
						},

						glium::glutin::WindowEvent::KeyboardInput {input: glium::glutin::KeyboardInput {virtual_keycode, state: glium::glutin::ElementState::Pressed, ..}, ..} => {
							use glium::glutin::VirtualKeyCode as Vk;
							match virtual_keycode {
								Some(Vk::Back) => character = Some(0x10),
								Some(Vk::Return) => character = Some(0x11),
								Some(Vk::Insert) => character = Some(0x12),
								Some(Vk::Delete) => character = Some(0x13),
								Some(Vk::Up) => character = Some(0x80),
								Some(Vk::Down) => character = Some(0x81),
								Some(Vk::Left) => character = Some(0x82),
								Some(Vk::Right) => character = Some(0x83),
								Some(Vk::RShift) | Some(Vk::LShift) => character = Some(0x90),
								Some(Vk::RControl) | Some(Vk::LControl) => character = Some(0x91),
								_ => (),
							}
						}

						_ => (),
					},

				_ => ()
			}
		});

		if let Some(c) = character {
			self.keyboard_buffer.push_back(c);
		}
	}


	pub fn interrupt(&mut self, dcpu: &mut dcpu::Dcpu) {
		match dcpu.registers[dcpu::A] {
			0 => self.keyboard_buffer.clear(),
			1 => dcpu.registers[dcpu::C] = self.keyboard_buffer.pop_front().unwrap_or(0),
			2 => unimplemented!(),
			3 => self.keyboard_interrupt = dcpu.registers[dcpu::B],
			_ => (),
		}
	}
}




enum HardwareType {
	Lem1820(lem1820::Lem1820),
	Eklectic(modem::Modem),
	Keyboard(Keyboard),
}

struct System {
	dcpu: dcpu::Dcpu,
	hardware: Vec<HardwareType>,
}

impl System {
	fn step(&mut self) {
		self.dcpu.step();

		if let Some(h) = self.dcpu.hardware_interrupt {
			match h {
				dcpu::HardwareInstruction::GetCount(destination) =>
					unimplemented!(),

				dcpu::HardwareInstruction::GetInfo(hardware_id) =>
					unimplemented!(),

				dcpu::HardwareInstruction::Interrupt(hardware_id) => {
					let hardware = self.hardware.get_mut(hardware_id as usize);

					match hardware {
						Some(&mut HardwareType::Lem1820(ref mut lem)) => lem.interrupt(&mut self.dcpu),
						Some(&mut HardwareType::Keyboard(ref mut key)) => key.interrupt(&mut self.dcpu),
						Some(&mut HardwareType::Eklectic(ref mut ek)) => ek.interrupt(&mut self.dcpu),
						None => (),
					}
				}

			}

			self.dcpu.hardware_interrupt = None;
		}

		for hardware in &mut self.hardware {
			match hardware {
				&mut HardwareType::Lem1820(ref mut lem) => lem.step(&mut self.dcpu),
				&mut HardwareType::Keyboard(ref mut key) => key.step(&mut self.dcpu),
				&mut HardwareType::Eklectic(ref mut ek) => ek.step(&mut self.dcpu),
			}
		}
	}
}


fn main() {
	let arguments: Arguments = docopt::Docopt::new(USAGE)
		.and_then(|d| d.deserialize())
		.unwrap_or_else(|e| e.exit());


	let mut hardware = Vec::new();

	if arguments.flag_lem1820 {
		let (lem, events_loop) = lem1820::Lem1820::new();
		hardware.push(HardwareType::Lem1820(lem));
		hardware.push(HardwareType::Keyboard(Keyboard::new(events_loop)));
	}

	if arguments.flag_eklectic {
		hardware.push(HardwareType::Eklectic(modem::Modem::new()));
	}

	if arguments.cmd_start {
		let mut system = System {
			dcpu: dcpu::Dcpu::new(),
			hardware: hardware,
		};

		use std::io::Read;
		let mut image: Vec<u8> = Vec::new();
		std::fs::File::open(arguments.arg_image.clone().unwrap()).unwrap().read_to_end(&mut image).unwrap();

		for (index, byte) in image.iter().enumerate() {
			if index % 2 == 0 {
				system.dcpu.memory[(index / 2) as usize] = (*byte as u16) << 8;
			} else {
				system.dcpu.memory[(index / 2) as usize] |= *byte as u16;
			}
		}

		loop {
			system.step();
		}
	} else if arguments.cmd_assemble {
		unimplemented!();
	}

	println!("{:?}", arguments);
}
