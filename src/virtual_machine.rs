use std::path::Path;

use arrayvec::ArrayVec;

use crate::{characters, HEIGHT};
use anyhow::{Context, Result};

#[derive(Debug)]
enum Relation {
    Equal,
    NotEqual,
}

pub struct VirtualMachine {
    memory: [u8; 0x1000],
    stack: ArrayVec<u16, 100>,
    registers: [u8; 16],
    i: u16,
    pc: u16,
    pub delay_timer: u8,
    pub sound_timer: u8,
    pub pressed_key: Option<u8>,
    pub canvas: [u64; HEIGHT],
}

impl VirtualMachine {
    pub fn new(path: &Path) -> Result<Self> {
        let rom = std::fs::read(path).with_context(|| format!("Failed to read ROM: {:?}", path))?;
        let mut machine = Self {
            memory: [0; 0x1000],
            stack: ArrayVec::new(),
            registers: [0; 16],
            i: 0x200,
            pc: 0x200,
            delay_timer: 0,
            sound_timer: 0,
            pressed_key: None,
            canvas: [0; HEIGHT],
        };

        // Game ROM starts at 0x200
        machine.memory[0x200..(0x200 + rom.len())].copy_from_slice(&rom);

        // Font ROM starts at 0x50
        machine.memory[0x50..0xA0].copy_from_slice(&characters::CHARS);

        Ok(machine)
    }

    fn get_memory(&self, address: u16) -> u8 {
        debug_assert!(address < 0x1000, "Address out of bounds: {:#X}", address);
        unsafe { *self.memory.get_unchecked(address as usize) }
    }

    fn set_memory(&mut self, address: u16, byte: u8) {
        debug_assert!(address < 0x1000, "Address out of bounds: {:#X}", address);
        unsafe { *self.memory.get_unchecked_mut(address as usize) = byte }
    }

    fn get_register(&self, register: u8) -> u8 {
        debug_assert!(register < 0x10, "Register does not exist: {:#X}", register);
        unsafe { *self.registers.get_unchecked(register as usize) }
    }

    fn set_register(&mut self, register: u8, byte: u8) {
        debug_assert!(register < 0x10, "Register does not exist: {:#X}", register);
        unsafe { *self.registers.get_unchecked_mut(register as usize) = byte }
    }

    fn set_flag(&mut self, flag: u8) {
        self.registers[15] = flag;
    }

    fn update_pc(&mut self, address: u16) {
        let new_pc = self.get_register(0) as u16 + address;
        self.pc = new_pc;
    }

    fn inc_pc(&mut self) {
        self.pc += 2;
    }

    fn dec_pc(&mut self) {
        self.pc -= 2;
    }

    fn call(&mut self, address: u16) {
        assert!(self.stack.len() < self.stack.capacity(), "Stack overflow");
        self.stack.push(self.pc);
        self.pc = address;
    }

    fn _return(&mut self) {
        debug_assert!(!self.stack.is_empty(), "Stack underflow");
        self.pc = self.stack.pop().unwrap();
    }

    fn jump_to(&mut self, address: u16) {
        self.pc = address;
    }

    fn skip_if_byte(&mut self, register: u8, byte: u8, relation: Relation) {
        let value = self.get_register(register);
        let condition = match relation {
            Relation::Equal => value == byte,
            Relation::NotEqual => value != byte,
        };

        if condition {
            self.inc_pc();
        }
    }

    fn skip_if_register(&mut self, register1: u8, register2: u8, relation: Relation) {
        let value1 = self.get_register(register1);
        let value2 = self.get_register(register2);
        let condition = match relation {
            Relation::Equal => value1 == value2,
            Relation::NotEqual => value1 != value2,
        };

        if condition {
            self.inc_pc();
        }
    }

    fn skip_if_key(&mut self, register: u8, relation: Relation) {
        let value = self.get_register(register);

        let condition = match relation {
            Relation::Equal => self.pressed_key.is_some_and(|key| key == value),
            Relation::NotEqual => {
                self.pressed_key.is_none() || self.pressed_key.is_some_and(|key| key != value)
            }
        };

        if condition {
            self.inc_pc();
        }
    }

    fn add_byte(&mut self, register: u8, byte: u8) {
        let value = self.get_register(register);
        self.set_register(register, value.wrapping_add(byte));
    }

    /// Source: https://en.wikipedia.org/wiki/CHIP-8#Opcode_table
    pub fn execute_opcode(&mut self) {
        let (byte1, byte2) = (self.get_memory(self.pc), self.get_memory(self.pc + 1));

        let address = ((byte1 as u16 & 0x0F) << 8) | (byte2 as u16);
        let register_x = byte1 & 0x0F;
        let register_y = byte2 >> 4;
        let last_nibble = byte2 & 0x0F;

        self.inc_pc();

        match (byte1 & 0xF0) >> 4 {
            0x0 => match byte2 {
                0xE0 => self.clear_canvas(),
                0xEE => self._return(),
                _ => self.call(address),
            },
            0x1 => self.jump_to(address),
            0x2 => self.call(address),
            0x3 => self.skip_if_byte(register_x, byte2, Relation::Equal),
            0x4 => self.skip_if_byte(register_x, byte2, Relation::NotEqual),
            0x5 => {
                assert_eq!(last_nibble, 0);
                self.skip_if_register(register_x, register_y, Relation::Equal);
            }
            0x6 => self.set_register(register_x, byte2),
            0x7 => self.add_byte(register_x, byte2),
            0x8 => self.execute_math(last_nibble, register_x, register_y),
            0x9 => {
                assert_eq!(last_nibble, 0);
                self.skip_if_register(register_x, register_y, Relation::NotEqual);
            }
            0xA => self.i = address,
            0xB => self.update_pc(address),
            0xC => self.set_register(register_x, fastrand::u8(..) & byte2),
            0xD => {
                let x = self.get_register(register_x);
                let y = self.get_register(register_y);
                let height = last_nibble;

                self.draw(x, y, height);
            }
            0xE => match byte2 {
                0x9E => self.skip_if_key(register_x, Relation::Equal),
                0xA1 => self.skip_if_key(register_x, Relation::NotEqual),
                _ => unreachable!(),
            },
            0xF => match byte2 {
                0x07 => self.set_register(register_x, self.delay_timer),
                0x0A => {
                    let value = self.pressed_key.take();

                    if let Some(code) = value {
                        self.set_register(register_x, code);
                    } else {
                        self.dec_pc();
                    }
                }
                0x15 => self.delay_timer = self.get_register(register_x),
                0x18 => {
                    self.sound_timer = self.get_register(register_x);
                    // SDL doesnt alway play audio if it only lasts for 1 frame
                    if self.sound_timer < 2 {
                        self.sound_timer = 2;
                    }
                }
                0x1E => self.i += self.get_register(register_x) as u16,
                0x29 => self.i = 0x50 + self.get_register(register_x) as u16 * 5,
                0x33 => self.set_bcd(register_x),
                0x55 => self.dump_registers(register_x),
                0x65 => self.load_registers(register_x),
                _ => unreachable!(),
            },
            _ => unreachable!(),
        }
    }

    fn dump_registers(&mut self, register: u8) {
        for index in 0u8..=register {
            self.set_memory(self.i + index as u16, self.get_register(index));
        }
    }

    fn load_registers(&mut self, register: u8) {
        for index in 0u8..=register {
            self.set_register(index, self.get_memory(self.i + index as u16));
        }
    }

    fn set_bcd(&mut self, register: u8) {
        let mut value = self.get_register(register);
        let units = value % 10;
        value /= 10;
        let tens = value % 10;
        value /= 10;
        let hundreds = value;

        self.set_memory(self.i, hundreds);
        self.set_memory(self.i + 1, tens);
        self.set_memory(self.i + 2, units);
    }

    fn execute_math(&mut self, operation: u8, register_x: u8, register_y: u8) {
        let value_x = self.get_register(register_x);
        let value_y = self.get_register(register_y);

        let result = match operation {
            0x0 => value_y,
            0x1 => value_x | value_y,
            0x2 => value_x & value_y,
            0x3 => value_x ^ value_y,
            0x4 => {
                let (result, flag) = value_x.overflowing_add(value_y);
                self.set_flag(flag as u8);
                result
            }
            0x5 => {
                let (result, flag) = value_x.overflowing_sub(value_y);
                self.set_flag(!flag as u8);
                result
            }
            0x6 => {
                self.set_flag(value_x & 1);
                value_x >> 1
            }
            0x7 => {
                let (result, flag) = value_y.overflowing_sub(value_x);
                self.set_flag(!flag as u8);
                result
            }
            0xE => {
                self.set_flag(value_x >> 7);
                value_x << 1
            }
            _ => unreachable!(),
        };

        self.set_register(register_x, result);
    }

    pub fn clear_canvas(&mut self) {
        self.canvas.fill(0);
    }

    fn draw(&mut self, x: u8, y: u8, height: u8) {
        let mut collision = false;
        for dy in 0..height {
            let byte = (self.get_memory(self.i + dy as u16).reverse_bits() as u64) << x;

            let canvas_row = &mut self.canvas[y.wrapping_add(dy) as usize % HEIGHT];

            let mask = byte & *canvas_row;

            if mask != 0 {
                collision = true;
            }

            *canvas_row ^= byte;
        }

        self.set_flag(collision as u8);
    }
}
