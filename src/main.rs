mod audio;
mod characters;
mod flags;
mod virtual_machine;

use anyhow::{anyhow, Result};
use audio::SquareWave;
use sdl2::audio::AudioSpecDesired;
use sdl2::event::Event;
use sdl2::keyboard::{Keycode, Scancode};
use sdl2::pixels::Color;
use sdl2::rect::Rect;
use std::time::{Duration, Instant};
use virtual_machine::VirtualMachine;

const PIXEL_SIZE: usize = 12;
const WIDTH: usize = 64;
const HEIGHT: usize = 32;
const WINDOW_Y: u32 = (PIXEL_SIZE * HEIGHT) as u32;
const WINDOW_X: u32 = (PIXEL_SIZE * WIDTH) as u32;
const REFRESH_RATE: u32 = 60;
const FRAME_TIME: Duration = Duration::from_nanos(1_000_000_000 / REFRESH_RATE as u64);
const CLOCK_HZ: u32 = 1000;

fn main() -> Result<()> {
    let flags = flags::Main::from_env_or_exit();

    if flags.benchmark {
        let mut machine = VirtualMachine::new(&flags.path)?;
        let start = Instant::now();
        let millions = flags.count.unwrap_or(100);
        for _ in 0..(millions * 1_000_000) {
            machine.execute_opcode()?;
        }
        let elapsed = start.elapsed();
        println!("{:.2}", millions as f64 / elapsed.as_secs_f64());
        return Ok(());
    }

    // Set default video driver to wayland
    sdl2::hint::set("SDL_VIDEODRIVER", "wayland,x11");

    let sdl_context = sdl2::init().map_err(|err| anyhow!(err))?;
    let video_subsystem = sdl_context.video().map_err(|err| anyhow!(err))?;
    let audio_subsystem = sdl_context.audio().map_err(|err| anyhow!(err))?;

    let window = video_subsystem
        .window("CHIP-8", WINDOW_X, WINDOW_Y)
        .position_centered()
        .opengl()
        .build()?;

    // Audio configuration

    let desired_spec = AudioSpecDesired {
        freq: Some(44_100),
        channels: Some(1),
        samples: None,
    };

    let device = audio_subsystem
        .open_playback(None, &desired_spec, |spec| SquareWave {
            phase_inc: 200.0 / spec.freq as f32,
            phase: 0.0,
            volume: 0.2,
        })
        .map_err(|err| anyhow!(err))?;

    // Window interaction
    let mut canvas = window.into_canvas().accelerated().present_vsync().build()?;

    let mut event_pump = sdl_context.event_pump().map_err(|err| anyhow!(err))?;

    // Our virtual machine
    let mut machine = VirtualMachine::new(&flags.path)?;
    let frequency = flags.frequency.unwrap_or(CLOCK_HZ);
    let instructions_per_frame = frequency / REFRESH_RATE;
    device.resume();

    let mut rects = Vec::with_capacity(WIDTH * HEIGHT);

    'main: loop {
        let now = Instant::now();
        rects.clear();

        machine.delay_timer = machine.delay_timer.saturating_sub(1);
        machine.sound_timer = machine.sound_timer.saturating_sub(1);

        for _ in 0..instructions_per_frame {
            machine.execute_opcode()?;
        }

        if machine.sound_timer > 0 {
            device.resume()
        } else {
            device.pause();
        }

        canvas.set_draw_color(Color::WHITE);
        canvas.clear();

        canvas.set_draw_color(Color::BLACK);
        for x in 0..WIDTH {
            for y in 0..HEIGHT {
                if (machine.canvas[y] >> x) & 1 == 1 {
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

        canvas.fill_rects(&rects).map_err(|err| anyhow!(err))?;
        canvas.present();

        // Read events for the remaining frame time
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
                        if !repeat {
                            // Set pressed key
                            machine.pressed_key = scancode_to_chip8_code(scancode);
                        }
                    }
                    Event::KeyUp { .. } => {
                        // Reset pressed key
                        machine.pressed_key = None;
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
