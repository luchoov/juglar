//! Orbits: nine lights on slow, tilted orbits, photographed with a long
//! exposure. Each leaves a fading arc of neon behind it.
//!
//! | Signal | Moves                                                       |
//! |--------|-------------------------------------------------------------|
//! | bass   | the inner three lights: their speed and brightness          |
//! | mid    | the middle three                                            |
//! | treble | the outer three                                             |
//! | beat   | a push that speeds every orbit up for a moment              |
//! | mood   | how wide the orbits open, how long the arcs last            |
//! | novelty| turns the palette, so a new section gets new colors         |
//!
//! The lights are soft spots, not lines, and their levels are smoothed in
//! three stages (see `smooth3`): a light that brightens and dims with every
//! kick in one place would blink.

use std::f32::consts::TAU;

use juglar_signals::Signals;

use super::{Look, Palette, Scene, smooth3};

/// Amber, violet, sky blue and green, all the way around. Its reddest
/// color keeps red under 72% of the light.
const PALETTE: Palette = Palette {
    a: [0.38, 0.68, 0.58],
    b: [0.53, 0.33, 0.54],
    c: [1.0, 1.0, 1.0],
    d: [0.0, 0.25, 0.55],
};

/// Seconds each smoothing stage of the levels takes.
const LEVEL_SECONDS: f32 = 0.12;
const MOOD_SECONDS: f32 = 3.0;

pub(crate) struct Orbits {
    /// Each ring's angle, which advances faster with the music.
    angles: [f32; 3],
    /// Bass, mid, treble, beat and novelty, smoothed.
    levels: [[f32; 5]; 3],
    mood: f32,
    clock: f32,
    /// Where the palette has turned to.
    hue: f32,
}

impl Orbits {
    pub fn new() -> Self {
        Self { angles: [0.0, 2.1, 4.2], levels: [[0.0; 5]; 3], mood: 0.0, clock: 0.0, hue: 0.0 }
    }
}

impl Scene for Orbits {
    fn name(&self) -> &'static str {
        "orbits"
    }

    fn shader(&self) -> &'static str {
        include_str!("orbits.wgsl")
    }

    fn update(&mut self, s: &Signals, motion: f32) -> Look {
        let dt = s.dt.clamp(0.0, 0.1);
        let moving = dt * motion;
        let [bass, mid, treble, beat, novelty] =
            smooth3(&mut self.levels, &[s.bass, s.mid, s.treble, s.beat, s.novelty], moving, LEVEL_SECONDS);
        self.mood += (s.energy - self.mood) * (1.0 - (-moving / MOOD_SECONDS).exp());
        self.clock += moving;
        let live = 0.4 + 1.2 * s.intensity.clamp(0.0, 1.0);
        // Rings alternate direction; each goes faster with its own band.
        for (k, level) in [bass, mid, treble].into_iter().enumerate() {
            let direction = if k == 1 { -1.0 } else { 1.0 };
            self.angles[k] += moving * live * direction * (0.15 + 0.6 * level + 0.8 * beat);
        }
        // A slow walk around the palette, faster after a change of section.
        self.hue += moving * (0.01 + 0.08 * novelty);

        Look {
            center: [0.0, 0.0],
            zoom: 1.0,
            rotation: 0.0,
            swirl: 0.0,
            // Long exposure: arcs last a second or two.
            decay: (-dt / (0.8 + 0.8 * self.mood)).exp(),
            palette_drift: 0.05 * moving,
            knobs: [
                self.angles[0],
                self.angles[1],
                self.angles[2],
                bass,
                mid,
                treble,
                0.85 + 0.3 * self.mood,     // orbit size
                (self.clock * TAU / 90.0).sin() * 0.6, // tilt of the orbits
            ],
            palette: PALETTE,
            palette_shift: self.hue,
            palette_wrap: true,
            exposure: 1.6,
            waveform: s.waveform,
            spectrum: s.spectrum,
        }
    }
}
