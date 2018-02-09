extern crate dcpu16_emulator as dcpu;

use std::io::prelude::*;
use std::net::{TcpListener, TcpStream, Ipv4Addr};
use dcpu::Dcpu;

const NOTHING: u16 = 0x0000;
const NO_TELEPHONE_SERVICE: u16 = 0x0001;
const LINE_BUSY: u16 = 0x0002;
const NO_MODEM: u16 = 0x0003;
const CONNECTION_MADE: u16 = 0x0004;
const RINGING: u16 = 0x0005;
const CONNECTION_LOST: u16 = 0x0006;
const DATA_IN_BUFFER: u16 = 0x0007;


enum ModemState {
	Idle,
	Ringing(TcpStream),
	Dialing(TcpStream),
	Connected(TcpStream),
	Writing(TcpStream, u16, u16),
}

struct Modem {
	incoming_server: TcpListener,
	state: ModemState,
	buffer: Vec<u16>,
	interrupt_address: Option<u16>,
	last_interrupt: u16,
}

impl Modem {
	fn new() -> Modem {
		let incoming_server = TcpListener::bind("127.0.0.1:6483").unwrap();
		incoming_server.set_nonblocking(true);

		Modem {
			incoming_server: incoming_server,
			state: ModemState::Idle,
			buffer: vec![],
			interrupt_address: None,
			last_interrupt: NOTHING,
		}
	}

	fn print_state(&self) {
		match self.state {
			ModemState::Idle => println!("Idle"),
			ModemState::Ringing(_) => println!("Ringing"),
			ModemState::Dialing(_) => println!("Dialing"),
			ModemState::Connected(_) => println!("Connected"),
			ModemState::Writing(_, _, _) => println!("Writing"),
		}
	}

	fn interrupt(&mut self, dcpu: &mut Dcpu) {
		match dcpu.registers[dcpu::A] {
			0 => self.set_interrupt(dcpu),
			1 => self.get_status(dcpu),
			2 => self.answer(dcpu),
			3 => self.dial(dcpu),
			4 => self.hang_up(dcpu),
			5 => self.send(dcpu),
			// 6 => self.receive(dcpu),
			_ => (),
		}
	}

	fn get_status(&mut self, dcpu: &mut Dcpu) {
		dcpu.registers[dcpu::A] =
			match self.state {
				ModemState::Idle => 0,
				ModemState::Ringing(_) => 1,
				ModemState::Dialing(_) => 2,
				ModemState::Connected(_) => 3,
				ModemState::Writing(_, _, _) => 4,
			};

		dcpu.registers[dcpu::B] = self.last_interrupt;
		dcpu.registers[dcpu::C] = self.buffer.len() as u16;
	}

	fn set_interrupt(&mut self, dcpu: &mut Dcpu) {
		if dcpu.registers[dcpu::B] == 0 {
			self.interrupt_address = None;
		} else {
			self.interrupt_address = Some(dcpu.registers[dcpu::B]);
		}
	}

	fn refuse_incoming(tcp_listener: &mut TcpListener) {
		if let Ok((mut socket, addr)) = tcp_listener.accept() {
			socket.write(&[0xbb]);
		}
	}

	fn answer(&mut self, dcpu: &mut Dcpu) {
		match std::mem::replace(&mut self.state, ModemState::Idle) {
			ModemState::Ringing(mut socket) => {
				socket.write(&[0xaa]);
				self.state = ModemState::Connected(socket);
			},

			otherwise =>
				self.state = otherwise,
		}
	}

	fn hang_up(&mut self, _dcpu: &mut Dcpu) {
		self.state = ModemState::Idle;
	}

	fn step(&mut self, dcpu: &mut Dcpu) {
		match std::mem::replace(&mut self.state, ModemState::Idle) {
			ModemState::Idle =>
				match self.incoming_server.accept() {
					Ok((socket, _addr)) => {
						if let Some(address) = self.interrupt_address {
							self.last_interrupt = RINGING;
							dcpu.interrupt_queue.push(address);
						}

						self.state = ModemState::Ringing(socket);
					},

					Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock =>
						(),

					Err(e) =>
						println!("No Client: {:?}", e),
				},


			ModemState::Ringing(mut socket) => {
				Modem::refuse_incoming(&mut self.incoming_server);

				let mut buffer: [u8; 500] = [0; 500];
				while let Ok(_bytes) = socket.read(&mut buffer) {
					// Do nothing
				}

				self.state = ModemState::Ringing(socket)
			}


			ModemState::Dialing(mut socket) => {
				Modem::refuse_incoming(&mut self.incoming_server);

				let mut buffer: [u8; 1] = [0; 1];
				match socket.read(&mut buffer) {
					Ok(bytes_read) =>
						if bytes_read == 0 {
							self.state = ModemState::Idle;

							if let Some(interrupt) = self.interrupt_address {
								self.last_interrupt = CONNECTION_LOST;
								dcpu.interrupt_queue.push(interrupt);
							}
						} else {
							if buffer[0] == 0xaa {
								self.state = ModemState::Connected(socket);

								if let Some(interrupt) = self.interrupt_address {
									self.last_interrupt = CONNECTION_MADE;
									dcpu.interrupt_queue.push(interrupt);
								}
							} else if buffer[0] == 0xbb {
								self.state = ModemState::Idle;

								if let Some(interrupt) = self.interrupt_address {
									self.last_interrupt = LINE_BUSY;
									dcpu.interrupt_queue.push(interrupt);
								}
							} else {
								self.state = ModemState::Dialing(socket);
							}
						},

					Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock =>
						self.state = ModemState::Dialing(socket),

					Err(e) => {
						println!("Error during read during dialing {:?}", e);
						self.state = ModemState::Idle;
					}
				}
			},


			ModemState::Connected(mut socket) => {
				Modem::refuse_incoming(&mut self.incoming_server);

				let mut buffer: [u8; 1000] = [0; 1000];
				match socket.read(&mut buffer) {
					Ok(bytes_read) => {
						if bytes_read == 0 {
							self.state = ModemState::Idle;

							if let Some(address) = self.interrupt_address {
								self.last_interrupt = CONNECTION_LOST;
								dcpu.interrupt_queue.push(address);
							}
						} else {
							self.state = ModemState::Connected(socket);

							if self.buffer.len() == 0 {
								if let Some(interrupt) = self.interrupt_address {
									self.last_interrupt = DATA_IN_BUFFER;
									dcpu.interrupt_queue.push(interrupt);
								}
							}

							self.buffer.append(
								&mut buffer[..bytes_read]
									.chunks(2)
									.map(|chunk|
										if chunk.len() == 1 {
											chunk[0] as u16
										} else {
											((chunk[0] as u16) << 8) + (chunk[1] as u16)
										})
									.collect::<Vec<_>>()
							);
						}
					},

					Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock =>
						self.state = ModemState::Connected(socket),

					Err(e) =>
						println!("Unable to read: {:?}", e),
				}
			},


			ModemState::Writing(mut socket, current_location, end_location) => {
				Modem::refuse_incoming(&mut self.incoming_server);

				let mut packet = Vec::new();
				for i in current_location..current_location + 5 {
					if i < end_location {
						let word = dcpu.memory[i as usize];
						packet.push((word << 8) as u8);
						packet.push(word as u8);
					}
				}

				self.state =
					match socket.write(&packet) {
						Ok(_bytes_written) =>
							if current_location + 5 > end_location {
								ModemState::Connected(socket)
							} else {
								ModemState::Writing(socket, current_location + 5, end_location)
							},

						Err(e) => {
							println!("Error writing: {:?}", e);
							ModemState::Idle
						}
					}
			},
		}
	}

	fn dial(&mut self, dcpu: &mut Dcpu) {
		self.state = ModemState::Idle;

		let first_half = dcpu.registers[dcpu::B];
		let second_half = dcpu.registers[dcpu::C];

		let a = (first_half >> 8) as u8;
		let b = first_half as u8;
		let c = (second_half >> 8) as u8;
		let d = second_half as u8;

		match TcpStream::connect((Ipv4Addr::new(a, b, c, d), 6482)) {
			Err(ref e) if e.kind() == std::io::ErrorKind::ConnectionRefused =>
				if let Some(address) = self.interrupt_address {
					self.last_interrupt = NO_MODEM;
					dcpu.interrupt_queue.push(address);
				},

			Err(_) =>
				if let Some(address) = self.interrupt_address {
					self.last_interrupt = NO_TELEPHONE_SERVICE;
					dcpu.interrupt_queue.push(address);
				},

			Ok(socket) => {
				socket.set_nonblocking(true);
				self.state = ModemState::Dialing(socket);
			},
		}
	}

	fn send(&mut self, dcpu: &mut Dcpu) {
		if let ModemState::Connected(ref mut socket) = self.state {
			let mut buffer: Vec<u8> = Vec::new();

			let offset = dcpu.registers[dcpu::B];
			let size = dcpu.registers[dcpu::C];
			for i in 0..size {
				buffer.push((dcpu.memory[(offset + i) as usize] >> 8) as u8);
				buffer.push((dcpu.memory[(offset + i) as usize]) as u8);
			}

			socket.write(buffer.as_slice());
		}
	}
}


fn main() {
	let mut dcpu = dcpu::Dcpu::new();
	let mut modem = Modem::new();

	dcpu.registers[dcpu::A] = 0xf00d;
	modem.set_interrupt(&mut dcpu);

	dcpu.registers[dcpu::B] = 0x7f00;
	dcpu.registers[dcpu::C] = 0x0001;
	modem.dial(&mut dcpu);

	dcpu.registers[dcpu::B] = 0x0100;
	dcpu.registers[dcpu::C] = 0x0010;
	modem.send(&mut dcpu);

	loop {
		std::thread::sleep(std::time::Duration::new(2, 0));

		modem.step(&mut dcpu);
		modem.print_state();
		modem.get_status(&mut dcpu);

		println!(">> {}, {} , {}", dcpu.registers[dcpu::A], dcpu.registers[dcpu::B], dcpu.registers[dcpu::C]);
		println!("   {} {:?}", modem.last_interrupt, dcpu.interrupt_queue);

		if dcpu.registers[dcpu::C] == 5 {
			dcpu.memory[0x0200] = 0xf00d;
			dcpu.registers[dcpu::B] = 0x0200;
			dcpu.registers[dcpu::C] = 0x0001;
			modem.send(&mut dcpu);
		}

		if dcpu.registers[dcpu::C] > 10 {
			modem.hang_up(&mut dcpu);
		}
	}

}