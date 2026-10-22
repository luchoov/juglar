//! Kaleido: the spectrum drawn as a six-petal rose, inside a kaleidoscope.
//! Every frame the old picture is folded into mirror images of one wedge,
//! zoomed outward and turned, and its colors keep moving around a looping
//! palette as it ages, so the echoes come out in rainbow rings.
//!
//! | Signal  | Moves                                                      |
//! |---------|------------------------------------------------------------|
//! | spectrum| the rose's shape: bass in the middle of each petal, treble |
//! |         | at its edges                                               |
//! | bass    | how fast the echoes fly outward, the rose's size           |
//! | mid     | how fast the picture turns                                 |
//! | energy  | how far the petals reach                                   |
//! | beat    | a push outward                                             |
//! | novelty | jumps the colors ahead, smoothly                           |
//!
//! The rose is a thin bright line, so its spectrum is smoothed in three
//! stages, like Tide's waveform.

use std::f32::consts::TAU;

use juglar_signals::{SPECTRUM_BANDS, Signals};

use super::{Character, Look, Palette, Scene, smooth3};

/// Magenta, blue, aqua and lime, all the way around. Red stays under 73%.
const PALETTE: Palette = Palette {
    a: [0.37, 0.66, 0.69],
    b: [0.50, 0.49, 0.46],
    c: [1.0, 1.0, 1.0],
    d: [0.1, 0.4, 0.7],
};

const SPECTRUM_SECONDS: f32 = 0.15;
const LEVEL_SECONDS: f32 = 0.12;
const MOOD_SECONDS: f32 = 3.0;

pub(crate) struct Kaleido {
    spectrum: [[f32; SPECTRUM_BANDS]; 3],
    /// Bass, mid, energy, beat and novelty, smoothed.
    levels: [[f32; 5]; 3],
    mood: f32,
    clock: f32,
    /// The rose's own slow turn, apart from the picture's.
    spin: f32,
    hue: f32,
}

impl Kaleido {
    pub fn new() -> Self {
        Self { spectrum: [[0.0; SPECTRUM_BANDS]; 3], levels: [[0.0; 5]; 3], mood: 0.0, clock: 0.0, spin: 0.0, hue: 0.0 }
    }
}

impl Scene for Kaleido {
    fn name(&self) -> &'static str {
        "kaleido"
    }

    /// A mirrored tunnel rushing outward: wild.
    fn character(&self) -> Character {
        Character { energy: 0.85, pulse: 0.7, flow: true }
    }

    fn shader(&self) -> &'static str {
        include_str!("kaleido.wgsl")
    }

    fn update(&mut self, s: &Signals, motion: f32) -> Look {
        let dt = s.dt.clamp(0.0, 0.1);
        let moving = dt * motion;
        let spectrum = smooth3(&mut self.spectrum, &s.spectrum, moving, SPECTRUM_SECONDS);
        let [bass, mid, energy, beat, novelty] =
            smooth3(&mut self.levels, &[s.bass, s.mid, s.energy, s.beat, s.novelty], moving, LEVEL_SECONDS);
        self.mood += (s.energy - self.mood) * (1.0 - (-moving / MOOD_SECONDS).exp());
        self.clock += moving;
        let live = motion * (0.4 + 1.2 * s.intensity.clamp(0.0, 1.0));
        self.spin += moving * 0.07;
        self.hue += moving * (0.02 + 0.25 * novelty);
        let direction = (self.clock * TAU / 70.0).sin();

        Look {
            center: [0.0, 0.0],
            zoom: (live * (0.3 + 0.9 * bass + 1.2 * beat) * dt).exp(),
            rotation: live * direction * (0.08 + 0.35 * mid) * dt,
            swirl: live * 0.15 * dt,
            decay: (-dt / (0.35 + 0.35 * self.mood)).exp(),
            // Echoes walk around the palette as they age: rainbow rings.
            palette_drift: 0.6 * moving,
            knobs: [
                0.13 + 0.03 * bass,   // rose radius
                0.07 + 0.1 * energy,  // how far the petals reach
                0.0025,               // line half-width
                0.15,                 // how much of the line the echoes keep
                6.0,                  // petals and mirror wedges
                1.7,                  // line ink on top
                self.spin,            // the rose's turn
                0.0,
            ],
            palette: PALETTE,
            palette_shift: self.hue,
            palette_wrap: true,
            exposure: 1.4,
            bloom: 0.6,
            waveform: s.waveform,
            spectrum,
        }
    }
}
