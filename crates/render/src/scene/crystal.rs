//! Crystal: a fractal of mirrors, grown one fold per frame.
//!
//! Each frame the old picture is turned, folded into five mirrored wedges,
//! moved out from the middle and shrunk to less than half its size: five
//! small copies of the whole picture around a ring. The next frame copies
//! those copies, and so on, so a soft ring drawn by the waveform in the
//! middle comes back as a snowflake of ever smaller rings, a fractal whose
//! shape follows the fold's turn and spacing. Color moves along the palette
//! with every fold, so each generation of copies has its own.
//!
//! | Signal  | Moves                                                      |
//! |---------|------------------------------------------------------------|
//! | mid     | how fast the fold's turn changes (the crystal twists)      |
//! | bass    | how far the copies sit from the middle (it breathes), the  |
//! |         | ring's size                                                |
//! | energy  | how big the copies are, the wave on the ring               |
//! | beat    | a push in the twist                                        |
//! | novelty | colors jump ahead                                          |
//!
//! As in Julia, the fade and the color step are per frame: each frame is one
//! generation. Small copies of a moving shape are where flicker finer than a
//! cell would hide, so the ring is soft and wide, each generation is
//! fainter than the one before, and the fold changes slowly.

use std::f32::consts::TAU;

use juglar_signals::{Signals, WAVEFORM_POINTS};

use super::{Look, Palette, Scene, smooth3};

/// Violet, blue, azure, aqua, green, olive and rose, all the way around, in
/// colors of similar brightness (linear luminance 0.11 to 0.46), like
/// Julia's. Red stays under 69%.
const PALETTE: Palette = Palette {
    a: [0.35, 0.41, 0.68],
    b: [0.43, 0.40, 0.46],
    c: [1.0, 1.0, 1.0],
    d: [0.15, 0.55, 0.9],
};

const WAVE_SECONDS: f32 = 0.25;
const LEVEL_SECONDS: f32 = 0.25;

pub(crate) struct Crystal {
    waveform: [[f32; WAVEFORM_POINTS]; 3],
    /// Bass, mid, energy, beat and novelty, smoothed.
    levels: [[f32; 5]; 3],
    /// The fold's turn, in radians.
    twist: f32,
    clock: f32,
    hue: f32,
}

impl Crystal {
    pub fn new() -> Self {
        Self { waveform: [[0.0; WAVEFORM_POINTS]; 3], levels: [[0.0; 5]; 3], twist: 0.3, clock: 0.0, hue: 0.0 }
    }
}

impl Scene for Crystal {
    fn name(&self) -> &'static str {
        "crystal"
    }

    fn shader(&self) -> &'static str {
        include_str!("crystal.wgsl")
    }

    fn update(&mut self, s: &Signals, motion: f32) -> Look {
        let dt = s.dt.clamp(0.0, 0.1);
        let moving = dt * motion;
        let waveform = smooth3(&mut self.waveform, &s.waveform, moving, WAVE_SECONDS);
        let [bass, mid, energy, beat, novelty] =
            smooth3(&mut self.levels, &[s.bass, s.mid, s.energy, s.beat, s.novelty], moving, LEVEL_SECONDS);
        let live = 0.4 + 1.2 * s.intensity.clamp(0.0, 1.0);
        self.clock += moving;
        // Radians per second; it reverses slowly, so the crystal twists
        // one way and back.
        let direction = (self.clock * TAU / 90.0).sin();
        self.twist += moving * live * direction * (0.03 + 0.08 * mid + 0.1 * beat);
        self.hue += moving * (0.02 + 0.2 * novelty);

        Look {
            center: [0.0, 0.0],
            zoom: 1.0,
            rotation: 0.0,
            swirl: 0.0,
            // Per generation, see above.
            decay: 0.8,
            palette_drift: 0.13,
            knobs: [
                self.twist,
                5.0,                       // mirrored wedges
                0.25 + 0.04 * bass,        // how far out the copies sit
                2.1 - 0.15 * energy,       // how much smaller each generation is
                0.1 + 0.02 * bass,         // ring radius
                0.04 * energy,             // wave height on the ring
                0.012,                     // ring half-width (soft)
                1.2,                       // ring ink
            ],
            palette: PALETTE,
            palette_shift: self.hue,
            palette_wrap: true,
            exposure: 1.4,
            bloom: 0.6,
            waveform,
            spectrum: s.spectrum,
        }
    }
}
