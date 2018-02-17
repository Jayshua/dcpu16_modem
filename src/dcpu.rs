pub const A: usize = 0x0;
pub const B: usize = 0x1;
pub const C: usize = 0x2;
pub const X: usize = 0x3;
pub const Y: usize = 0x4;
pub const Z: usize = 0x5;
pub const I: usize = 0x6;
pub const J: usize = 0x7;


#[derive(Copy, Clone)]
pub enum HardwareInstruction {
   GetCount(u16),
   GetInfo(u16),
   Interrupt(u16),
}


pub struct Dcpu {
   pub registers: [u16; 8],
   pub stack_pointer: u16,
   pub program_counter: u16,
   pub excess: u16,
   pub memory: [u16; 0x10000],
   pub cycle_accumulator: u32,
   pub cycle_count: u32,
   pub interrupt_address: u16,
   pub interrupt_queue: Vec<u16>,
   pub interrupt_queueing: bool,
   pub hardware_interrupt: Option<HardwareInstruction>,
}

fn get_operand_cost(operand: u16) -> u32 {
   match operand {
      0x10...0x17 | 0x1a | 0x1e | 0x1f => 1,
      _ => 0
   }
}

fn get_operand_length(operand: u16) -> u16 {
   match operand {
      0x10...0x17 | 0x1a | 0x1e | 0x1f => 1,
      _ => 0
   }
}

fn get_opcode_length(opcode: u16) -> u16 {
   let (_, operand_b, operand_a) = get_opcode_parts(opcode);
   1 + get_operand_length(operand_b) + get_operand_length(operand_a)
}

fn get_instruction_cost(instruction: u16) -> u32 {
   match instruction {
      0x01 | 0x0a | 0x0b | 0x0c | 0x0d | 0x0e | 0x0f => 1,
      0x02 | 0x03 | 0x04 | 0x05 | 0x1e | 0x1f => 2,
      0x10 | 0x11 | 0x12 | 0x13 | 0x14 | 0x15 | 0x16 | 0x17 => 2,
      0x06 | 0x07 | 0x08 | 0x09 | 0x1a | 0x1b => 3,
      _ => 0
   }
}

fn get_special_instruction_cost(instruction: u16) -> u32 {
   match instruction {
      0x00 => 0,
      0x09 | 0x0a => 1,
      0x0c | 0x10 => 2,
      0x01 | 0x0b => 3,
      0x08 | 0x11 | 0x12 => 4,
      _ => 0 // panic!("Unknown special instruction {}", instruction),
   }
}

fn is_if_op_code(op_code: u16) -> bool {
   match op_code & 0x1f {
      0x10 | 0x11 | 0x12 | 0x13 | 0x14 | 0x15 | 0x16 | 0x17 => true,
      _ => false
   }
}


// Return the parts of the instruction as a tuple of the form (instruction, operand_b, operand_a)
fn get_opcode_parts(instruction: u16) -> (u16, u16, u16) {
   (
      (instruction & 0b0000_0000_0001_1111) >> 00,
      (instruction & 0b0000_0011_1110_0000) >> 05,
      (instruction & 0b1111_1100_0000_0000) >> 10
   )
}


impl Dcpu {
   pub fn new() -> Dcpu {
      Dcpu {
         registers: [0; 8],
         stack_pointer: 0,
         program_counter: 0,
         excess: 0,
         memory: [0; 0x10000],
         cycle_accumulator: 0,
         cycle_count: 0,
         interrupt_address: 0,
         interrupt_queue: vec![],
         interrupt_queueing: false,
         hardware_interrupt: None,
      }
   }


   pub fn step(&mut self) {
      // Skip the step if the accumulator still has cycles from the last operation
      if self.cycle_accumulator > 0 {
         self.cycle_accumulator -= 1;
         self.cycle_count += 1;
         return;
      }

      // Skip the step if there is a hardware interrupt waiting to be handled
      if self.hardware_interrupt.is_some() {
         self.cycle_count += 1;
         return;
      }


      // Decode the instruction
      let op_code = self.memory[self.program_counter as usize];
      let (instruction, operand_b, operand_a) = get_opcode_parts(op_code);
      self.program_counter = self.program_counter.wrapping_add(1);

      // Get the value of operand a, updating the various states
      let value_a = self.get_value(operand_a);
      if operand_a == 0x18 {
         self.stack_pointer = self.stack_pointer.wrapping_add(1);
      }
      self.program_counter = self.program_counter.wrapping_add(get_operand_length(operand_a));
      self.cycle_accumulator += get_operand_cost(operand_a);


      /* Handle a Special Instruction */
      if instruction == 0x0 {
         // Increment the cycle counters with the cost of the instruction
         self.cycle_accumulator += get_special_instruction_cost(operand_b);

         // Execute the instruction
         match operand_b {
            0x01 => { // jsr
               self.stack_pointer = self.stack_pointer.wrapping_sub(1);
               self.memory[self.stack_pointer as usize] = self.program_counter;
               self.program_counter = value_a;
            },

            0x08 => { // int
               if self. interrupt_address != 0 {
                  self.interrupt_queueing = true;
                  self.stack_pointer.wrapping_sub(1);
                  self.memory[self.stack_pointer as usize] = self.program_counter;
                  self.stack_pointer.wrapping_sub(1);
                  self.memory[self.stack_pointer as usize] = self.registers[0];
                  self.program_counter = self.interrupt_address;
                  self.registers[0] = value_a;
               }
            },

            0x09 => { // iag
               let interrupt_address = self.interrupt_address;
               if let Some(pointer_a) = self.get_pointer(value_a) {
                  *pointer_a = interrupt_address;
               }
            },

            0x0b => { // rfi
               self.interrupt_queueing = false;
               self.registers[0] = self.memory[self.stack_pointer as usize];
               self.stack_pointer.wrapping_add(1);
               self.program_counter = self.memory[self.stack_pointer as usize];
               self.stack_pointer.wrapping_add(1);
            },

            0x0a => self.interrupt_address = value_a, // ias
            0x0c => self.interrupt_queueing = value_a != 0, // iaq
            0x10 => self.hardware_interrupt = Some(HardwareInstruction::GetCount(operand_a)), // hwn
            0x11 => self.hardware_interrupt = Some(HardwareInstruction::GetInfo(value_a)), // hwq
            0x12 => self.hardware_interrupt = Some(HardwareInstruction::Interrupt(value_a)), // hwi

            _ => ()
         }
      }

      /* Handle a Regular Instruction */
      else {
         // Get the value of operand b, updating the state as necessary
         let value_b = self.get_value(operand_b);
         if operand_b == 0x18 {
            self.stack_pointer = self.stack_pointer.wrapping_sub(1);
         }
         self.program_counter = self.program_counter.wrapping_add(get_operand_length(operand_b));
         self.cycle_accumulator += get_operand_cost(operand_b);


         // Update the cycle counters with the cost of the instruction
         self.cycle_accumulator += get_instruction_cost(instruction);

         // Execute the instruction
         // Handle branching instructions
         if is_if_op_code(op_code) {
            let is_valid: bool = match instruction {
               0x10 => (value_b & value_a) != 0,
               0x11 => (value_b & value_a) == 0,
               0x12 => value_b == value_a,
               0x13 => value_b != value_a,
               0x14 => value_b > value_a,
               0x15 => (value_b as i16) > (value_a as i16),
               0x16 => value_b < value_a,
               0x17 => (value_b as i16) < (value_a as i16),
               _ => panic!("Unexpected if instruction! This should have been impossible.")
            };

            if !is_valid {
               // Skip chained if instructions
               while is_if_op_code(self.memory[self.program_counter as usize]) {
                  self.program_counter = self.program_counter.wrapping_add(get_opcode_length(self.memory[self.program_counter as usize]));
                  self.cycle_accumulator += 1;
               }

               // Skip the final instruction the ifs are protecting
               self.cycle_accumulator += 1;
               self.program_counter = self.program_counter.wrapping_add(get_opcode_length(self.memory[self.program_counter as usize]));
            }
         }

         // Handle non-branching instructions
         else {
            let excess = self.excess;
            if let Some(pointer_b) = self.get_pointer(operand_b) {
               *pointer_b = match instruction {
                  0x00 => *pointer_b,
                  0x01 => value_a,
                  0x02 => value_b.wrapping_add(value_a),
                  0x03 => value_b.wrapping_sub(value_a),
                  0x04 => value_b.wrapping_mul(value_a),
                  0x05 => (value_b as i16).wrapping_mul(value_a as i16) as u16,
                  0x06 => if value_a == 0 {0} else {value_b / value_a},
                  0x07 => if value_a == 0 {0} else {(value_b as i16).wrapping_div(value_a as i16) as u16},
                  0x08 => if value_a == 0 {0} else {value_b % value_a},
                  0x09 => if value_a == 0 {0} else {(value_b as i16).wrapping_rem(value_a as i16) as u16},
                  0x0a => value_b & value_a,
                  0x0b => value_b | value_a,
                  0x0c => value_b ^ value_a,
                  0x0d => value_b >> value_a,
                  0x0e => ((value_b as i16) >> value_a) as u16,
                  0x0f => value_b << value_a,
                  0x1a => value_b.wrapping_add(value_a).wrapping_add(excess),
                  0x1b => value_b.wrapping_sub(value_a).wrapping_add(excess),
                  0x1e => value_a,
                  0x1f => value_a,
                  _ => *pointer_b
               };
            }
         }


         // Update the overflow register
         let value_a_signed = value_a as i16;
         let value_b_signed = value_b as i16;
         self.excess = match instruction {
            0x02 => if (value_b as u32 + value_a as u32) > 0xffff {1} else {0},
            0x03 => if (value_b as i32 - value_a as i32) < 0 {0xffff} else {0},
            0x04 => ((value_b as u32 * value_a as u32) >> 16) as u16,
            0x05 => ((value_b as i32 * value_a as i32) >> 16) as u16,
            0x06 => if value_a == 0 {0} else {(((value_b as i32) << 16i32) / (value_a as i32)) as u16},
            0x07 => if value_a == 0 {0} else {(((value_b_signed as i32) << 16) / (value_a_signed as i32)) as u16},
            0x0d => (((value_b as u32) << 16) >> value_a) as u16,
            0x0e => (((value_b as i32) << 16) >> value_a) as u16,
            0x0f => (((value_b as u32) << value_a) >> 16) as u16,
            0x1a => if (value_b as u32 + value_a as u32 + self.excess as u32) > 0xffff {1} else {0},
            0x1b => if (value_b as i32 - value_a as i32 + self.excess as i32) < 0 {0xffff} else {0},
            _ => self.excess,
         };


         // Update the I and J registers
         match instruction {
            0x1e => {
               self.registers[6] = self.registers[6].wrapping_add(1);
               self.registers[7] = self.registers[7].wrapping_add(1);
            },

            0x1f => {
               self.registers[6] = self.registers[6].wrapping_sub(1);
               self.registers[7] = self.registers[7].wrapping_sub(1);
            },

            _ => (),
         }
      }


      // Decrement a cycle for this step, and increment the program counter
      if instruction != 0x0 {
         self.cycle_accumulator -= 1;
         self.cycle_count += 1;
      }
   }


   pub fn set_value(&mut self, operand: u16, value: u16) {
      if let Some(pointer) = self.get_pointer(operand) {
         *pointer = value;
      }
   }


   // Get a pointer to the location represented by the given operand
   fn get_pointer(&mut self, operand: u16) -> Option<&mut u16> {
      // The program counter has already been incremented past the "next word"
      // and is pointing to the next instruction. Therefore, to get the next word
      // we actually need to get the previous word.
      let next_word: u16 = self.memory[self.program_counter.wrapping_sub(1) as usize];

      match operand {
         0x00...0x07 => Some(&mut self.registers[operand as usize]),
         0x08...0x0f => Some(&mut self.memory[self.registers[(operand - 0x8) as usize] as usize]),
         0x10...0x17 => Some(&mut self.memory[self.registers[(operand - 0x10) as usize].wrapping_add(next_word) as usize]),
         0x18 => Some(&mut self.memory[self.stack_pointer as usize]),
         0x19 => Some(&mut self.memory[self.stack_pointer as usize]),
         0x1a => Some(&mut self.memory[self.stack_pointer.wrapping_add(next_word) as usize]),
         0x1b => Some(&mut self.stack_pointer),
         0x1c => Some(&mut self.program_counter),
         0x1d => Some(&mut self.excess),
         0x1e => Some(&mut self.memory[next_word as usize]),
         _ => {
            println!("Warning: Tried to get the value of a non-value operand: {}", operand);
            None
         },
      }
   }



   // Get the value of the given operand, incrementing the cycle_accumulator and program_counter as necessary
   fn get_value(&self, operand: u16) -> u16 {
      let next_word: u16 = self.memory[self.program_counter as usize];

      match operand {
         0x00...0x07 => self.registers[operand as usize],
         0x08...0x0f => self.memory[self.registers[(operand - 0x08) as usize] as usize],
         0x10...0x17 => self.memory[self.registers[(operand - 0x10) as usize].wrapping_add(next_word) as usize],
         0x18 | 0x19 => self.memory[self.stack_pointer as usize],
         0x1a => self.memory[self.stack_pointer.wrapping_add(next_word) as usize],
         0x1b => self.stack_pointer,
         0x1c => self.program_counter,
         0x1d => self.excess,
         0x1e => self.memory[next_word as usize],
         0x1f => next_word,
         0x20...0x3f => operand.wrapping_sub(0x21),
         _ => panic!("Invalid operand! This probably shouldn't have happened.")
      }
   }
}