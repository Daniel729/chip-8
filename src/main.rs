mod characters;
mod virtual_machine;

use sdl2::event::Event;
use sdl2::keyboard::{Keycode, Scancode};
use sdl2::pixels::Color;
use sdl2::rect::Rect;
use std::sync::atomic::Ordering;
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};
use virtual_machine::{CanvasColor, VirtualMachine};

use sdl2::audio::{AudioCallback, AudioSpecDesired};

const PIXEL_SIZE: usize = 12;
const WIDTH: usize = 64;
const HEIGHT: usize = 32;
const WINDOW_Y: u32 = (PIXEL_SIZE * HEIGHT) as u32;
const WINDOW_X: u32 = (PIXEL_SIZE * WIDTH) as u32;
const REFRESH_RATE: u32 = 60;
const FRAME_TIME: Duration = Duration::from_nanos(1_000_000_000 / REFRESH_RATE as u64);
const CLOCK_HZ: u32 = 1000;

struct SquareWave {
    phase_inc: f32,
    phase: f32,
    volume: f32,
}

impl AudioCallback for SquareWave {
    type Channel = f32;

    fn callback(&mut self, out: &mut [f32]) {
        for x in out.iter_mut() {
            *x = if self.phase <= 0.5 {
                self.volume
            } else {
                -self.volume
            };
            self.phase = (self.phase + self.phase_inc) % 1.0;
        }
    }
}

pub fn main() -> Result<(), String> {
    let sdl_context = sdl2::init()?;
    let video_subsystem = sdl_context.video()?;
    let audio_subsystem = sdl_context.audio()?;

    let window = video_subsystem
        .window("Chip 8", WINDOW_X, WINDOW_Y)
        .position_centered()
        .opengl()
        .build()
        .map_err(|e| e.to_string())?;

    let desired_spec = AudioSpecDesired {
        freq: Some(44_100),
        channels: Some(1),
        samples: None,
    };

    let device = audio_subsystem.open_playback(None, &desired_spec, |spec| SquareWave {
        phase_inc: 440.0 / spec.freq as f32,
        phase: 0.0,
        volume: 0.25,
    })?;

    let mut canvas = window.into_canvas().build().map_err(|e| e.to_string())?;

    let mut event_pump = sdl_context.event_pump()?;

    let path = PathBuf::from(std::env::args().nth(1).unwrap());

    let mut machine = VirtualMachine::new(&path);

    let machine_canvas_mutex = machine.canvas();
    let pressed_key_mutex = machine.pressed_key();
    let sound_timer = machine.sound_timer();

    std::thread::spawn({
        let frequency = std::env::args()
            .nth(2)
            .map(|x| x.parse().unwrap())
            .unwrap_or(CLOCK_HZ);

        move || loop {
            machine.execute_opcode();
            std::thread::sleep(Duration::from_secs_f64(1.0 / frequency as f64));
        }
    });

    'main: loop {
        let now = Instant::now();

        if sound_timer.load(Ordering::Relaxed) > 0 {
            device.resume();
        } else {
            device.pause();
        }

        canvas.set_draw_color(Color::WHITE);
        canvas.clear();

        let machine_canvas = machine_canvas_mutex.lock().unwrap();
        let mut rects = Vec::with_capacity(WIDTH * HEIGHT);

        canvas.set_draw_color(Color::BLACK);
        for x in 0..WIDTH {
            for y in 0..HEIGHT {
                if machine_canvas[y][x] == CanvasColor::Black {
                    let rect = Rect::new(
                        (PIXEL_SIZE * x) as i32,
                        (PIXEL_SIZE * y) as i32,
                        PIXEL_SIZE as u32,
                        PIXEL_SIZE as u32,
                    );
                    rects.push(rect);
                }
            }
        }

        drop(machine_canvas);

        canvas.fill_rects(&rects)?;
        canvas.present();

        while now.elapsed() < FRAME_TIME {
            if let Some(event) = event_pump.wait_event_timeout(now.elapsed().as_millis() as u32) {
                match event {
                    Event::Quit { .. }
                    | Event::KeyDown {
                        keycode: Some(Keycode::Escape),
                        ..
                    } => break 'main,
                    Event::KeyDown {
                        scancode, repeat, ..
                    } => {
                        if repeat {
                            continue;
                        }
                        let mut pressed_key = pressed_key_mutex.lock().unwrap();
                        *pressed_key = scancode_to_chip8_code(scancode);
                    }
                    Event::KeyUp { .. } => {
                        let mut pressed_key = pressed_key_mutex.lock().unwrap();
                        *pressed_key = None;
                    }
                    _ => {}
                }
            }
        }
    }

    Ok(())
}

fn scancode_to_chip8_code(scancode: Option<Scancode>) -> Option<u8> {
    scancode.and_then(|code| match code {
        Scancode::Num1 => Some(0x1),
        Scancode::Num2 => Some(0x2),
        Scancode::Num3 => Some(0x3),
        Scancode::Num4 => Some(0xC),
        Scancode::Q => Some(0x4),
        Scancode::W => Some(0x5),
        Scancode::E => Some(0x6),
        Scancode::R => Some(0xD),
        Scancode::A => Some(0x7),
        Scancode::S => Some(0x8),
        Scancode::D => Some(0x9),
        Scancode::F => Some(0xE),
        Scancode::Z => Some(0xA),
        Scancode::X => Some(0x0),
        Scancode::C => Some(0xB),
        Scancode::V => Some(0xF),
        _ => None,
    })
}
