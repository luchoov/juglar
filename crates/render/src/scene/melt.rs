//! Melt: the spectrum painted across the top of the picture, melting down
//! it like wax. A band of color hangs across the top, each point of it as
//! bright as its band of the spectrum (bass in the middle, treble at the
//! sides, mirrored left to right);
//! the warp pulls the paint down at a speed that changes from column to
//! column and sways the columns sideways, so the colors run in long,
//! wavering drips. The waveform, drawn faintly across, leaves curtains.
//!
//! | Signal  | Moves                                                      |
//! |---------|------------------------------------------------------------|
//! | spectrum| how much each point of the band gives off                  |
//! | bass    | how fast the paint runs down                               |
//! | mid     | how far the columns sway                                   |
//! | energy  | the wave's height                                          |
//! | beat    | a surge in the running                                     |
//! | novelty | the columns' pattern changes, colors jump ahead            |
//!
//! The band follows the spectrum smoothed in three stages of 0.3 s, which
//! damps kicks twice a second to under a fiftieth of their swing: a band
//! that pulsed with every kick would make every point of the drips below
//! it pulse too. It is one continuous band, not a row of drops: a row of
//! dozens of drops would be the regular stripes the research warns about.

use std::f32::consts::TAU;

use juglar_signals::{SPECTRUM_BANDS, Signals, WAVEFORM_POINTS};

use super::{Look, Palette, Scene, smooth3};

/// Rose, orchid, violet, blue, aqua, green, lime and old gold, all the way
/// around. Red stays under 70%.
const PALETTE: Palette = Palette {
    a: [0.50, 0.62, 0.66],
    b: [0.38, 0.40, 0.42],
    c: [1.0, 1.0, 1.0],
    d: [0.0, 0.33, 0.67],
};

const SPECTRUM_SECONDS: f32 = 0.3;
const WAVE_SECONDS: f32 = 0.25;
const LEVEL_SECONDS: f32 = 0.2;
const MOOD_SECONDS: f32 = 3.0;

pub(crate) struct Melt {
    spectrum: [[f32; SPECTRUM_BANDS]; 3],
    waveform: [[f32; WAVEFORM_POINTS]; 3],
    /// Bass, mid, energy, beat and novelty, smoothed.
    levels: [[f32; 5]; 3],
    mood: f32,
    /// How the columns' pattern has changed, and how far they have swayed.
    pattern: f32,
    sway: f32,
    clock: f32,
    hue: f32,
}

impl Melt {
    pub fn new() -> Self {
        Self {
            spectrum: [[0.0; SPECTRUM_BANDS]; 3],
            waveform: [[0.0; WAVEFORM_POINTS]; 3],
            levels: [[0.0; 5]; 3],
            mood: 0.0,
            pattern: 0.0,
            sway: 0.0,
            clock: 0.0,
            hue: 0.0,
        }
    }
}

impl Scene for Melt {
    fn name(&self) -> &'static str {
        "melt"
    }

    fn shader(&self) -> &'static str {
        include_str!("melt.wgsl")
    }

    fn update(&mut self, s: &Signals, motion: f32) -> Look {
        let dt = s.dt.clamp(0.0, 0.1);
        let moving = dt * motion;
        let spectrum = smooth3(&mut self.spectrum, &s.spectrum, moving, SPECTRUM_SECONDS);
        let waveform = smooth3(&mut self.waveform, &s.waveform, moving, WAVE_SECONDS);
        let [bass, mid, energy, beat, novelty] =
            smooth3(&mut self.levels, &[s.bass, s.mid, s.energy, s.beat, s.novelty], moving, LEVEL_SECONDS);
        self.mood += (s.energy - self.mood) * (1.0 - (-moving / MOOD_SECONDS).exp());
        let live = motion * (0.4 + 1.2 * s.intensity.clamp(0.0, 1.0));
        self.clock += moving;
        self.pattern += moving * (0.06 + 0.4 * novelty);
        self.sway += moving * (0.5 + 1.0 * mid);
        self.hue += moving * (0.02 + 0.2 * novelty);

        Look {
            center: [0.0, 0.0],
            zoom: 1.0,
            rotation: 0.0,
            swirl: 0.0,
            decay: (-dt / (1.2 + 0.6 * self.mood)).exp(),
            // Drips change color as they run.
            palette_drift: 0.25 * moving,
            knobs: [
                live * (0.15 + 0.25 * bass + 0.3 * beat) * dt,  // how far paint runs down this frame
                live * (0.03 + 0.08 * mid) * dt,               // how far the columns sway this frame
                self.pattern,                                  // the columns' pattern
                self.sway,                                     // the sway's phase
                -0.3,                                          // the band's height
                0.03 + 0.05 * energy,                          // wave height
                0.0,
                0.0,
            ],
            palette: PALETTE,
            palette_shift: self.hue + 0.05 * (self.clock * TAU / 90.0).sin(),
            palette_wrap: true,
            exposure: 1.3,
            bloom: 0.45,
            waveform,
            spectrum,
        }
    }
}
