//! Corona: the spectrum as flames around an eclipsed sun. A ring around a
//! dark disc gives off paint, each point of it as much as its band of the
//! spectrum is loud (bass at the top, treble at the bottom, mirrored left
//! to right), and the warp carries the paint outward through a slowly
//! churning turbulence, so it rises in tongues and curls like the sun's
//! corona in an eclipse. The paint cools along the palette as it rises.
//!
//! | Signal  | Moves                                                      |
//! |---------|------------------------------------------------------------|
//! | spectrum| how much each point of the ring gives off                  |
//! | bass    | how fast the flames rise, the sun's size                   |
//! | energy  | how strong the turbulence is                               |
//! | beat    | a surge outward                                            |
//! | novelty | how fast the turbulence churns, colors jump ahead          |
//! | mood    | how far the flames reach before they fade                  |
//!
//! The flames' brightness follows the spectrum smoothed in three stages,
//! and the turbulence changes slowly: flames that flickered like real fire,
//! several times a second, would be flashing.

use std::f32::consts::TAU;

use juglar_signals::{SPECTRUM_BANDS, Signals};

use super::{Character, Look, Palette, Scene, smooth3};

/// Peach gold, pink, orchid, violet, blue, teal, green and lemon, all the
/// way around. Red stays under 59%.
const PALETTE: Palette = Palette {
    a: [0.64, 0.50, 0.77],
    b: [0.55, 0.50, 0.52],
    c: [1.0, 1.0, 1.0],
    d: [0.95, 0.15, 0.65],
};

const SPECTRUM_SECONDS: f32 = 0.15;
const LEVEL_SECONDS: f32 = 0.2;
const MOOD_SECONDS: f32 = 3.0;

pub(crate) struct Corona {
    spectrum: [[f32; SPECTRUM_BANDS]; 3],
    /// Bass, energy, beat and novelty, smoothed.
    levels: [[f32; 4]; 3],
    mood: f32,
    /// How the turbulence has churned.
    churn: f32,
    clock: f32,
    hue: f32,
}

impl Corona {
    pub fn new() -> Self {
        Self { spectrum: [[0.0; SPECTRUM_BANDS]; 3], levels: [[0.0; 4]; 3], mood: 0.0, churn: 0.0, clock: 0.0, hue: 0.0 }
    }
}

impl Scene for Corona {
    fn name(&self) -> &'static str {
        "corona"
    }

    /// Flames streaming out of an eclipse.
    fn character(&self) -> Character {
        Character { energy: 0.75, pulse: 0.7, flow: true }
    }

    fn shader(&self) -> &'static str {
        include_str!("corona.wgsl")
    }

    fn update(&mut self, s: &Signals, motion: f32) -> Look {
        let dt = s.dt.clamp(0.0, 0.1);
        let moving = dt * motion;
        let spectrum = smooth3(&mut self.spectrum, &s.spectrum, moving, SPECTRUM_SECONDS);
        let [bass, energy, beat, novelty] =
            smooth3(&mut self.levels, &[s.bass, s.energy, s.beat, s.novelty], moving, LEVEL_SECONDS);
        self.mood += (s.energy - self.mood) * (1.0 - (-moving / MOOD_SECONDS).exp());
        let live = motion * (0.4 + 1.2 * s.intensity.clamp(0.0, 1.0));
        self.clock += moving;
        self.churn += moving * (0.08 + 0.3 * novelty);
        self.hue += moving * (0.01 + 0.2 * novelty);

        Look {
            center: [0.0, 0.0],
            zoom: (live * (0.35 + 0.7 * bass + 0.8 * beat) * dt).exp(),
            rotation: live * 0.04 * (self.clock * TAU / 75.0).sin() * dt,
            swirl: 0.0,
            decay: (-dt / (0.4 + 0.3 * self.mood)).exp(),
            // Flames cool as they rise.
            palette_drift: 0.45 * moving,
            knobs: [
                self.churn,                           // the turbulence's pattern
                live * (0.05 + 0.1 * energy) * dt,    // how far it moves paint this frame
                0.14 + 0.015 * bass,                  // the sun's radius
                0.012,                                // the ring's soft width
                0.35,                                 // how much the ring gives off
                0.0,
                0.0,
                0.0,
            ],
            palette: PALETTE,
            palette_shift: self.hue,
            palette_wrap: true,
            exposure: 1.3,
            bloom: 0.7,
            waveform: s.waveform,
            spectrum,
        }
    }
}
