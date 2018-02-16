extern crate dcpu16_emulator as dcpu;

#[macro_use]
extern crate glium;

mod modem;
mod lem1820;




fn main() {
	let mut dcpu = dcpu::Dcpu::new();
	let mut modem = modem::Modem::new();
	let mut monitor = lem1820::Lem1820::new();

	dcpu.registers[dcpu::A] = 3;
	dcpu.registers[dcpu::B] = 0x7F00;
	dcpu.registers[dcpu::C] = 0x0001;
	modem.interrupt(&mut dcpu);
	monitor.interrupt(&mut dcpu);

	// Render
	loop {
		modem.step(&mut dcpu);
		std::thread::sleep(std::time::Duration::new(0, 13333300));
	}
}
