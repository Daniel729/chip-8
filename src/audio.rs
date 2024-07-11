use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

use sdl2::audio::AudioCallback;

pub struct SquareWave {
    pub phase_inc: f32,
    pub phase: f32,
    pub volume: f32,
    pub playing: Arc<AtomicBool>,
}

impl AudioCallback for SquareWave {
    type Channel = f32;

    fn callback(&mut self, out: &mut [f32]) {
        let playing = self.playing.load(Ordering::Relaxed);
        if playing {
            for x in out.iter_mut() {
                *x = if self.phase <= 0.5 {
                    self.volume
                } else {
                    -self.volume
                };
                self.phase = (self.phase + self.phase_inc) % 1.0;
            }
        } else {
            out.fill(0.0);
        }
    }
}
