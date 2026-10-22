//! Plasma: the whole screen is color, in slow waves, like the "plasma"
//! effects of 1990s demos. Four waves of different shapes (two straight,
//! two circling around wandering centers) are added up, and the sum picks a
//! color from a looping palette. Each frame paints a little of that over
//! the old picture, which the warp turns and stretches, so the waves smear
//! into each other like dye.
//!
//! | Signal  | Moves                                                      |
//! |---------|------------------------------------------------------------|
//! | bass    | how fast the waves travel                                  |
//! | mid     | how fast the picture turns                                 |
//! | energy  | how bright the color is                                    |
//! | beat    | a push in the waves' travel                                |
//! | novelty | jumps the colors ahead, smoothly                           |
//!
//! The waves are wide (about two across the screen) and slow, so a point
//! sees a crest pass less than once a second: colors this different in
//! brightness, alternating a few times a second, would be flashing.

use std::f32::consts::TAU;

use juglar_signals::Signals;

use super::{Character, Look, Palette, Scene, smooth3};

/// Hot pink, lavender, aqua, teal and plum, all the way around. Red stays
/// under 72%.
const PALETTE: Palette = Palette {
    a: [0.46, 0.41, 0.70],
    b: [0.53, 0.53, 0.33],
    c: [1.0, 1.0, 1.0],
    d: [0.0, 0.6, 0.8],
};

const LEVEL_SECONDS: f32 = 0.2;

pub(crate) struct Plasma {
    /// Bass, mid, energy, beat and novelty, smoothed.
    levels: [[f32; 5]; 3],
    /// How far the waves have traveled.
    phase: f32,
    clock: f32,
    hue: f32,
}

impl Plasma {
    pub fn new() -> Self {
        Self { levels: [[0.0; 5]; 3], phase: 0.0, clock: 0.0, hue: 0.0 }
    }
}

impl Scene for Plasma {
    fn name(&self) -> &'static str {
        "plasma"
    }

    /// Soft waves of color, gently turning.
    fn character(&self) -> Character {
        Character { energy: 0.4, pulse: 0.3, flow: true }
    }

    fn shader(&self) -> &'static str {
        include_str!("plasma.wgsl")
    }

    fn update(&mut self, s: &Signals, motion: f32) -> Look {
        let dt = s.dt.clamp(0.0, 0.1);
        let moving = dt * motion;
        let [bass, mid, energy, beat, novelty] =
            smooth3(&mut self.levels, &[s.bass, s.mid, s.energy, s.beat, s.novelty], moving, LEVEL_SECONDS);
        let live = 0.4 + 1.2 * s.intensity.clamp(0.0, 1.0);
        self.clock += moving;
        // Radians per second of the waves' phase: at full intensity, with
        // loud bass and a beat, under 0.7 waves pass a point each second.
        self.phase += moving * live * (0.35 + 0.8 * bass + 1.0 * beat);
        self.hue += moving * (0.015 + 0.2 * novelty);

        Look {
            center: [0.0, 0.0],
            zoom: (live * 0.08 * moving).exp(),
            rotation: live * (0.03 + 0.12 * mid) * (self.clock * TAU / 50.0).sin() * moving,
            swirl: live * 0.25 * (self.clock * TAU / 37.0).cos() * moving,
            decay: (-dt / 0.8).exp(),
            palette_drift: 0.05 * moving,
            knobs: [self.phase, self.clock, 0.35 + 0.65 * energy, 0.0, 0.0, 0.0, 0.0, 0.0],
            palette: PALETTE,
            palette_shift: self.hue,
            palette_wrap: true,
            exposure: 1.1,
            bloom: 0.3,
            waveform: s.waveform,
            spectrum: s.spectrum,
        }
    }
}
