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


struct System {
	dcpu: dcpu::Dcpu,
	lem1820: Option<lem1820::Lem1820>,
	eklectic: Option<modem::Modem>,
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

				dcpu::HardwareInstruction::Interrupt(hardware_id) =>
					match (hardware_id, &mut self.lem1820, &mut self.eklectic) {
						(0, &mut Some(ref mut lem), _) => lem.interrupt(&mut self.dcpu),
						(0, &mut None, &mut Some(ref mut ek)) => ek.interrupt(&mut self.dcpu),
						(1, &mut Some(_), &mut Some(ref mut ek)) => ek.interrupt(&mut self.dcpu),
						_ => (),
					},
			}

			self.dcpu.hardware_interrupt = None;
		}

		if let Some(ref mut lem) = self.lem1820 {
			lem.step(&mut self.dcpu);
		}

		if let Some(ref mut eklectic) = self.eklectic {
			eklectic.step(&mut self.dcpu);
		}
	}
}


fn main() {
	let arguments: Arguments = docopt::Docopt::new(USAGE)
		.and_then(|d| d.deserialize())
		.unwrap_or_else(|e| e.exit());

	if arguments.cmd_start {
		let mut system = System {
			dcpu: dcpu::Dcpu::new(),
			lem1820:
				if arguments.flag_lem1820 {
					Some(lem1820::Lem1820::new())
				} else {
					None
				},
			eklectic:
				if arguments.flag_eklectic {
					Some(modem::Modem::new())
				} else {
					None
				},
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
