use std;
use dcpu;
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

// Byte sent over the network when the user answers an incoming call
const ANSWER: u8 = 0xaa;
// Byte sent over the network to an incoming caller when a connection already exists
const BUSY: u8 = 0xbb;

enum ModemState {
	Idle,
	Ringing(TcpStream),
	Dialing(TcpStream),
	Connected(TcpStream),
	Writing(TcpStream, u16, u16),
}

pub struct Modem {
	incoming_server: TcpListener,
	state: ModemState,
	buffer: Vec<u16>,
	interrupt_address: Option<u16>,
	last_interrupt: u16,
}

impl Modem {
	/// Create a new Modem
	pub fn new() -> Modem {
		let incoming_server = TcpListener::bind("0.0.0.0:6483").unwrap();
		incoming_server.set_nonblocking(true).unwrap();

		Modem {
			incoming_server: incoming_server,
			state: ModemState::Idle,
			buffer: vec![],
			interrupt_address: None,
			last_interrupt: NOTHING,
		}
	}

	/// Print the state of the modem
	pub fn print_state(&self) {
		match self.state {
			ModemState::Idle => println!("Idle"),
			ModemState::Ringing(_) => println!("Ringing"),
			ModemState::Dialing(_) => println!("Dialing"),
			ModemState::Connected(_) => println!("Connected"),
			ModemState::Writing(_, _, _) => println!("Writing"),
		}
	}

	/// Interrupt the modem
	pub fn interrupt(&mut self, dcpu: &mut Dcpu) {
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

	/// Get the status of the modem
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

	/// Set the value to interrupt the Dcpu with when something happens
	fn set_interrupt(&mut self, dcpu: &mut Dcpu) {
		if dcpu.registers[dcpu::B] == 0 {
			self.interrupt_address = None;
		} else {
			self.interrupt_address = Some(dcpu.registers[dcpu::B]);
		}
	}

	/// Answer the ringing number
	fn answer(&mut self, dcpu: &mut Dcpu) {
		match std::mem::replace(&mut self.state, ModemState::Idle) {
			ModemState::Ringing(mut socket) => {
				socket.write(&[0xaa]).unwrap();
				self.state = ModemState::Connected(socket);
			},

			otherwise =>
				self.state = otherwise,
		}
	}

	/// Hang up the active connection
	fn hang_up(&mut self, _dcpu: &mut Dcpu) {
		self.state = ModemState::Idle;
	}


	// Dial to the given address
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
				self.interrupt_dcpu(dcpu, NO_MODEM),

			Err(_) =>
				if let Some(address) = self.interrupt_address {
					self.last_interrupt = NO_TELEPHONE_SERVICE;
					dcpu.interrupt_queue.push(address);
				},

			Ok(socket) => {
				socket.set_nonblocking(true).unwrap();
				self.state = ModemState::Dialing(socket);
			},
		}
	}


	// Interrupt the dcpu with the given message if interrupts are enabled
	fn interrupt_dcpu(&mut self, dcpu: &mut dcpu::Dcpu, interrupt_type: u16) {
		if let Some(address) = self.interrupt_address {
			self.last_interrupt = interrupt_type;
			dcpu.interrupt_queue.push(address);
		}
	}


	// Refuse incoming connections on the tcp listener
	fn refuse_incoming(tcp_listener: &mut TcpListener) {
		if let Ok((mut socket, addr)) = tcp_listener.accept() {
			socket.write(&[BUSY]).unwrap();
		}
	}


	/// Send data over the active connection
	fn send(&mut self, dcpu: &mut Dcpu) {
		if let ModemState::Connected(ref mut socket) = self.state {
			let mut buffer: Vec<u8> = Vec::new();

			let offset = dcpu.registers[dcpu::B];
			let size = dcpu.registers[dcpu::C];
			for i in 0..size {
				buffer.push((dcpu.memory[(offset + i) as usize] >> 8) as u8);
				buffer.push((dcpu.memory[(offset + i) as usize]) as u8);
			}

			socket.write(buffer.as_slice()).unwrap();
		}
	}


	// Step th emodem forward one step
	pub fn step(&mut self, dcpu: &mut Dcpu) {
		self.state = match std::mem::replace(&mut self.state, ModemState::Idle) {
			ModemState::Idle =>
				match self.incoming_server.accept() {
					Ok((socket, _addr)) => {
						self.interrupt_dcpu(dcpu, RINGING);
						ModemState::Ringing(socket)
					},

					Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock =>
						ModemState::Idle,

					Err(e) => {
						println!("No Client: {:?}", e);
						ModemState::Idle
					},
				},


			ModemState::Ringing(mut socket) => {
				Modem::refuse_incoming(&mut self.incoming_server);

				// Ignore any incoming bytes since the user hasn't answered yet
				let mut buffer: [u8; 500] = [0; 500];
				while let Ok(_bytes) = socket.read(&mut buffer) {
					// Doing nothing in this loop is not a bug. It's a feature.
				}

				ModemState::Ringing(socket)
			}


			ModemState::Dialing(mut socket) => {
				Modem::refuse_incoming(&mut self.incoming_server);

				let mut buffer: [u8; 1] = [0; 1];
				match socket.read(&mut buffer) {
					Ok(bytes_read) =>
						if bytes_read == 0 {
							self.interrupt_dcpu(dcpu, CONNECTION_LOST);
							ModemState::Idle
						} else {
							if buffer[0] == ANSWER {
								self.interrupt_dcpu(dcpu, CONNECTION_MADE);
								ModemState::Connected(socket)
							} else if buffer[0] == BUSY {
								self.interrupt_dcpu(dcpu, LINE_BUSY);
								ModemState::Idle
							} else {
								ModemState::Dialing(socket)
							}
						},

					Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock =>
						ModemState::Dialing(socket),

					Err(e) => {
						println!("Error during read during dialing {:?}", e);
						ModemState::Idle
					}
				}
			},


			ModemState::Connected(mut socket) => {
				Modem::refuse_incoming(&mut self.incoming_server);

				let mut buffer: [u8; 1000] = [0; 1000];
				match socket.read(&mut buffer) {
					Ok(bytes_read) => {
						if bytes_read == 0 {
							self.interrupt_dcpu(dcpu, CONNECTION_LOST);
							ModemState::Idle
						} else {
							if self.buffer.len() == 0 {
								self.interrupt_dcpu(dcpu, DATA_IN_BUFFER);
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

							ModemState::Connected(socket)
						}
					},

					Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock =>
						ModemState::Connected(socket),

					Err(e) => {
						println!("Unable to read: {:?}", e);
						ModemState::Idle
					}
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
}