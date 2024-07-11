use std::{
    path::Path,
    sync::{
        atomic::{AtomicU8, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};

use crate::{characters, HEIGHT, WIDTH};

enum Relation {
    Equal,
    NotEqual,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum CanvasColor {
    White,
    Black,
}

impl CanvasColor {
    fn change(&mut self) -> bool {
        match self {
            Self::White => {
                *self = Self::Black;
                false
            }
            Self::Black => {
                *self = Self::White;
                true
            }
        }
    }
}

pub struct VirtualMachine {
    memory: [u8; 0x1000],
    registers: [u8; 16],
    stack: [u16; 100],
    stack_size: u8,
    i: u16,
    pc: u16,
    delay_timer: Arc<AtomicU8>,
    sound_timer: Arc<AtomicU8>,
    pressed_key: Arc<Mutex<Option<u8>>>,
    should_increment_pc: bool,
    canvas: Arc<Mutex<[[CanvasColor; WIDTH]; HEIGHT]>>,
}

impl VirtualMachine {
    pub fn new(path: &Path) -> Self {
        let rom = std::fs::read(path).unwrap();
        let mut machine = Self {
            memory: [0; 0x1000],
            registers: [0; 16],
            stack: [0; 100],
            stack_size: 0,
            i: 0x200,
            pc: 0x200,
            delay_timer: Arc::new(AtomicU8::new(0)),
            sound_timer: Arc::new(AtomicU8::new(0)),
            pressed_key: Arc::new(Mutex::new(None)),
            should_increment_pc: false,
            canvas: Arc::new(Mutex::new([[CanvasColor::White; WIDTH]; HEIGHT])),
        };

        machine.memory[0x200..(0x200 + rom.len())].copy_from_slice(&rom);
        machine.memory[0x50..0xA0].copy_from_slice(&characters::CHARS);

        std::thread::spawn({
            let sound_timer = machine.sound_timer.clone();
            let delay_timer = machine.delay_timer.clone();
            move || loop {
                let sound_value = sound_timer.load(Ordering::Relaxed);
                if sound_value > 0 {
                    sound_timer.store(sound_value - 1, Ordering::Relaxed);
                }

                let delay_value = delay_timer.load(Ordering::Relaxed);
                if delay_value > 0 {
                    delay_timer.store(delay_value - 1, Ordering::Relaxed);
                }

                std::thread::sleep(Duration::from_secs_f64(1.0 / 60.0));
            }
        });

        machine
    }

    pub fn canvas(&self) -> Arc<Mutex<[[CanvasColor; WIDTH]; HEIGHT]>> {
        self.canvas.clone()
    }

    pub fn sound_timer(&self) -> Arc<AtomicU8> {
        self.sound_timer.clone()
    }

    pub fn pressed_key(&self) -> Arc<Mutex<Option<u8>>> {
        self.pressed_key.clone()
    }

    const fn get_memory(&self, address: u16) -> u8 {
        self.memory[address as usize]
    }

    fn set_memory(&mut self, address: u16, byte: u8) {
        self.memory[address as usize] = byte
    }

    const fn get_register(&self, register: u8) -> u8 {
        self.registers[register as usize]
    }

    fn set_register(&mut self, register: u8, byte: u8) {
        self.registers[register as usize] = byte;
    }

    fn set_flag(&mut self, flag: u8) {
        self.registers[15] = flag;
    }

    fn call(&mut self, address: u16) {
        self.inc_pc();
        self.stack[self.stack_size as usize] = self.pc;
        self.stack_size += 1;
        self.pc = address;
        self.should_increment_pc = false;
    }

    fn _return(&mut self) {
        assert!(self.stack_size >= 1);
        self.stack_size -= 1;
        self.pc = self.stack[self.stack_size as usize];
        self.should_increment_pc = false;
    }

    fn jump_to(&mut self, address: u16) {
        self.pc = address;
        self.should_increment_pc = false;
    }

    fn inc_pc(&mut self) {
        self.pc += 2;
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

        let mutex = self.pressed_key.lock().unwrap();
        let key = *mutex;
        drop(mutex);

        let condition = match relation {
            Relation::Equal => key.is_some_and(|key| key == value),
            Relation::NotEqual => key.is_none() || key.is_some_and(|key| key != value),
        };

        if condition {
            self.inc_pc();
        }
    }

    fn add_byte(&mut self, register: u8, byte: u8) {
        let value = self.get_register(register);
        self.set_register(register, value.wrapping_add(byte));
    }

    pub fn execute_opcode(&mut self) {
        self.should_increment_pc = true;
        let (byte1, byte2) = (self.get_memory(self.pc), self.get_memory(self.pc + 1));
        // println!("Byte1: {:02X}, Byte2: {:02X}", byte1, byte2);
        let address = ((byte1 as u16 & 0x0F) << 8) | (byte2 as u16);
        let register_x = byte1 & 0x0F;
        let register_y = byte2 >> 4;
        let last_nibble = byte2 & 0x0F;

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
            0xB => {
                let new_pc = self.get_register(0) as u16 + address;
                self.pc = new_pc;
                self.should_increment_pc = false;
            }
            0xC => {
                let random_num = fastrand::u8(..) & byte2;
                self.set_register(register_x, random_num);
            }
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
                0x07 => {
                    let value = self.delay_timer.load(Ordering::Relaxed);
                    self.set_register(register_x, value);
                }
                0x0A => {
                    let mut mutex = self.pressed_key.lock().unwrap();
                    let value = *mutex;
                    *mutex = None;
                    drop(mutex);

                    if let Some(code) = value {
                        self.set_register(register_x, code);
                    } else {
                        return;
                    }
                }
                0x15 => {
                    let value = self.get_register(register_x);
                    self.delay_timer.store(value, Ordering::Relaxed);
                }
                0x18 => {
                    let value = self.get_register(register_x);
                    self.sound_timer.store(value, Ordering::Relaxed);
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

        if self.should_increment_pc {
            self.inc_pc()
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
            _ => panic!(),
        };

        self.set_register(register_x, result);
    }

    pub fn clear_canvas(&self) {
        let mut canvas = self.canvas.lock().unwrap();
        canvas
            .iter_mut()
            .for_each(|row| row.fill(CanvasColor::White));
    }

    fn draw(&mut self, x: u8, y: u8, height: u8) {
        let mut canvas = self.canvas.lock().unwrap();
        let mut collision = false;
        for dy in 0..height {
            let byte = self.get_memory(self.i + dy as u16);
            for dx in 0..8 {
                let pixel = byte & (0x80 >> dx) != 0;
                if pixel {
                    collision |=
                        canvas[(y + dy) as usize % HEIGHT][(x + dx) as usize % WIDTH].change();
                }
            }
        }

        drop(canvas);

        self.set_flag(collision as u8);
    }
}
