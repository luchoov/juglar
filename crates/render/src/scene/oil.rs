//! Oil: a liquid light show, after the projections of the 1960s, where
//! colored oil and water were swirled on a glass plate under a lamp. Four
//! blobs of color well up and a slowly changing current drags them into
//! marbled streaks. Nothing here is a line: the music moves the current.
//!
//! | Signal          | Moves                                              |
//! |-----------------|----------------------------------------------------|
//! | bass, mid,      | how much each of four blobs wells up, and its size |
//! | treble, energy  |                                                    |
//! | energy          | how strong the current is, how fast blobs wander   |
//! | beat            | a surge in the current                             |
//! | novelty         | how fast the current's pattern changes             |
//! | mood            | how long the color lasts                           |
//!
//! The current is a "curl" of a smooth noise field: the field's level
//! lines, followed sideways. Such a flow never piles color up in one spot
//! or leaves holes, as stirring a liquid doesn't.

use std::f32::consts::TAU;

use juglar_signals::Signals;

use super::{Character, Look, Palette, Scene, smooth3};

/// Lime, magenta, blue and mint, all the way around. Red stays under 73%.
const PALETTE: Palette = Palette {
    a: [0.37, 0.67, 0.58],
    b: [0.47, 0.54, 0.52],
    c: [1.0, 1.0, 1.0],
    d: [0.85, 0.15, 0.5],
};

const LEVEL_SECONDS: f32 = 0.2;
const MOOD_SECONDS: f32 = 3.0;

pub(crate) struct Oil {
    /// Bass, mid, treble, energy, beat and novelty, smoothed.
    levels: [[f32; 6]; 3],
    mood: f32,
    /// Drives where the blobs wander.
    wander: f32,
    /// Drives how the current's pattern changes.
    churn: f32,
    clock: f32,
}

impl Oil {
    pub fn new() -> Self {
        Self { levels: [[0.0; 6]; 3], mood: 0.0, wander: 0.0, churn: 0.0, clock: 0.0 }
    }
}

impl Scene for Oil {
    fn name(&self) -> &'static str {
        "oil"
    }

    /// Slow liquid light: the calmest.
    fn character(&self) -> Character {
        Character { energy: 0.2, pulse: 0.2, flow: false }
    }

    fn shader(&self) -> &'static str {
        include_str!("oil.wgsl")
    }

    fn update(&mut self, s: &Signals, motion: f32) -> Look {
        let dt = s.dt.clamp(0.0, 0.1);
        let moving = dt * motion;
        let [bass, mid, treble, energy, beat, novelty] =
            smooth3(&mut self.levels, &[s.bass, s.mid, s.treble, s.energy, s.beat, s.novelty], moving, LEVEL_SECONDS);
        self.mood += (s.energy - self.mood) * (1.0 - (-moving / MOOD_SECONDS).exp());
        let live = 0.4 + 1.2 * s.intensity.clamp(0.0, 1.0);
        self.clock += moving;
        self.wander += moving * live * (0.12 + 0.3 * energy);
        self.churn += moving * (0.03 + 0.2 * novelty);
        // How far the current carries color per second, in short sides.
        let current = live * (0.04 + 0.1 * energy + 0.3 * beat);

        Look {
            center: [0.0, 0.0],
            zoom: 1.0,
            rotation: live * 0.04 * (self.clock * TAU / 60.0).sin() * moving,
            swirl: 0.0,
            decay: (-dt / (2.0 + 2.0 * self.mood)).exp(),
            palette_drift: 0.02 * moving,
            knobs: [self.wander, self.churn, current * moving, bass, mid, treble, energy, 2.2],
            palette: PALETTE,
            palette_shift: 0.03 * self.clock,
            palette_wrap: true,
            exposure: 1.2,
            bloom: 0.4,
            waveform: s.waveform,
            spectrum: s.spectrum,
        }
    }
}
