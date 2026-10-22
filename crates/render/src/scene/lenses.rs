//! Lenses: four drifting lenses over marbled color. A thin coat of color is
//! laid everywhere each frame, from a marble pattern of noise that drifts;
//! four lenses wander over it, each one bending the picture under it every
//! frame, two of them magnifying and turning one way, two shrinking and
//! turning the other. Since the loop repeats the bend every frame, a
//! magnifying lens keeps blowing the marble outward from its middle, a
//! shrinking one keeps drawing it in, and between them the colors stream
//! from one to the other in swirls.
//!
//! | Signal  | Moves                                                      |
//! |---------|------------------------------------------------------------|
//! | bass,   | how strongly each lens bends (one lens each)               |
//! | mid,    |                                                            |
//! | treble, |                                                            |
//! | energy  |                                                            |
//! | beat    | every lens bends harder for a moment                       |
//! | novelty | the marble changes faster, colors jump ahead               |
//!
//! Nothing gets brighter with the music: the lenses only move paint.

use std::f32::consts::TAU;

use juglar_signals::Signals;

use super::{Character, Look, Palette, Scene, smooth3};

/// Teal, sky blue, violet, magenta and lime, all the way around. Red stays
/// under 72%.
const PALETTE: Palette = Palette {
    a: [0.44, 0.47, 0.73],
    b: [0.50, 0.47, 0.52],
    c: [1.0, 1.0, 1.0],
    d: [0.65, 0.35, 0.95],
};

const LEVEL_SECONDS: f32 = 0.2;

pub(crate) struct Lenses {
    /// Bass, mid, treble, energy, beat and novelty, smoothed.
    levels: [[f32; 6]; 3],
    /// Drives where the lenses wander, and how the marble drifts.
    wander: f32,
    marble: f32,
    clock: f32,
    hue: f32,
}

impl Lenses {
    pub fn new() -> Self {
        Self { levels: [[0.0; 6]; 3], wander: 0.0, marble: 0.0, clock: 0.0, hue: 0.0 }
    }
}

impl Scene for Lenses {
    fn name(&self) -> &'static str {
        "lenses"
    }

    /// Lenses slowly stirring a marbled coat.
    fn character(&self) -> Character {
        Character { energy: 0.3, pulse: 0.4, flow: false }
    }

    fn shader(&self) -> &'static str {
        include_str!("lenses.wgsl")
    }

    fn update(&mut self, s: &Signals, motion: f32) -> Look {
        let dt = s.dt.clamp(0.0, 0.1);
        let moving = dt * motion;
        let [bass, mid, treble, energy, beat, novelty] =
            smooth3(&mut self.levels, &[s.bass, s.mid, s.treble, s.energy, s.beat, s.novelty], moving, LEVEL_SECONDS);
        let live = motion * (0.4 + 1.2 * s.intensity.clamp(0.0, 1.0));
        self.clock += moving;
        self.wander += moving * (0.1 + 0.15 * energy);
        self.marble += moving * (0.04 + 0.3 * novelty);
        self.hue += moving * (0.02 + 0.2 * novelty);
        // Each lens's bend this frame: how much it magnifies (negative:
        // shrinks) per second, scaled to this frame.
        let bend = |level: f32| live * (0.3 + 1.2 * level + 0.8 * beat) * dt;

        Look {
            center: [0.0, 0.0],
            zoom: 1.0,
            rotation: live * 0.05 * (self.clock * TAU / 80.0).sin() * dt,
            swirl: 0.0,
            decay: (-dt / 1.2).exp(),
            palette_drift: 0.08 * moving,
            knobs: [self.wander, self.marble, bend(bass), bend(mid), bend(treble), bend(energy), 0.0, 0.0],
            palette: PALETTE,
            palette_shift: self.hue,
            palette_wrap: true,
            exposure: 1.2,
            bloom: 0.35,
            waveform: s.waveform,
            spectrum: s.spectrum,
        }
    }
}
