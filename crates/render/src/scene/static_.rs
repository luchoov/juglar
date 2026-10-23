//! Static: the waveform drawn huge across the screen while the picture
//! tears apart behind it. Every row of old paint slides sideways by its own
//! amount and every column drips down, so the trails smear into streaks
//! like a tape chewed by the machine; each hit throws blocks of color onto
//! the screen, and in a wall of noise the whole picture is cut into shifted
//! slices and torn into its colors. Made for the densest music: death
//! metal, grind, a wall of feedback.
//!
//! | Signal   | Moves                                                     |
//! |----------|-----------------------------------------------------------|
//! | waveform | the line across the screen                                |
//! | kick, hit| the rows' tear, new blocks                                |
//! | snare    | slices shifted sideways                                   |
//! | spectrum | each block's size: four slices of it                      |
//! | impact   | a flash into the negative, a new pattern of rows          |
//! | chaos    | everything: tear, drip, slices, torn colors               |
//! | harmonic | how far the colors drift as the streaks age               |

use juglar_signals::{Signals, WAVEFORM_POINTS};

use super::react::{Pulse, Reactor};
use super::{Character, Fx, Look, Palette, Scene};

/// Orange, magenta, violet, blue, teal and olive, all the way around.
const PALETTE: Palette = Palette {
    a: [0.45, 0.3, 0.45],
    b: [0.55, 0.35, 0.5],
    c: [1.0, 1.0, 1.0],
    d: [0.95, 0.15, 0.55],
};

pub(crate) struct Static {
    reactor: Reactor,
    waveform: [f32; WAVEFORM_POINTS],
    clock: f32,
    hue: f32,
    /// Changes at each hit: where the blocks land and how the rows slide.
    blocks: u32,
    tear: u32,
}

impl Static {
    pub fn new() -> Self {
        Self { reactor: Reactor::new(0x57a7), waveform: [0.0; WAVEFORM_POINTS], clock: 0.0, hue: 0.0, blocks: 0, tear: 0 }
    }
}

impl Scene for Static {
    fn name(&self) -> &'static str {
        "static"
    }

    /// Torn apart by the noise itself.
    fn character(&self) -> Character {
        Character { energy: 0.95, pulse: 0.9, flow: true }
    }

    fn shader(&self) -> &'static str {
        include_str!("static_.wgsl")
    }

    fn update(&mut self, s: &Signals, motion: f32) -> Look {
        let dt = s.dt.clamp(0.0, 0.1);
        let moving = dt * motion;
        let Pulse { punch, snap, shake, flash, cuts, chaos, harmony, kicked, .. } = self.reactor.update(s, motion);
        let follow = 1.0 - (-moving / 0.015).exp();
        for (smooth, &raw) in self.waveform.iter_mut().zip(&s.waveform) {
            *smooth += (raw - *smooth) * follow;
        }
        let (punch, snap) = (punch.max(0.0), snap.abs());
        let live = motion * (0.5 + s.intensity.clamp(0.0, 1.0));
        self.clock += moving;
        self.hue += moving * (0.03 + 0.2 * harmony);
        if kicked {
            self.blocks = self.blocks.wrapping_add(1);
        }
        // The rows find a new way to slide at each hit, and a few times a
        // second anyway while the music is a wall.
        if kicked || (chaos > 0.5 && (self.clock * 8.0) as u32 != ((self.clock - moving) * 8.0) as u32) {
            self.tear = self.tear.wrapping_add(1);
        }

        let fade = (0.5 - 0.25 * chaos).max(0.1);
        Look {
            center: [0.0, 0.0],
            zoom: (live * (0.05 + 0.6 * punch) * dt).exp(),
            rotation: 0.0,
            swirl: 0.0,
            decay: (-moving / fade).exp(),
            palette_drift: (0.3 + 0.8 * harmony) * moving,
            knobs: [
                live * (0.12 + 1.5 * punch + 0.9 * chaos) * dt, // row tear this frame
                live * (0.05 + 0.25 * chaos) * dt,             // drip this frame
                24.0 + 8.0 * (cuts % 5) as f32,                // rows
                self.tear as f32,                              // the tear's seed
                motion * (0.12 + 0.25 * s.loudness),           // wave height
                self.blocks as f32,                            // blocks' seed
                0.15 * punch.min(1.5) * motion,                // blocks' size
                1.2 + punch,                                   // ink
            ],
            palette: PALETTE,
            palette_shift: self.hue + (cuts % 5) as f32 * 0.21,
            palette_wrap: true,
            exposure: 1.2 + 0.5 * s.loudness,
            bloom: 0.35,
            waveform: self.waveform,
            spectrum: s.spectrum,
            fx: Fx {
                shake: [0.02 * shake[0], 0.012 * shake[1]],
                chroma: 0.003 + 0.02 * chaos + 0.01 * punch.min(1.0),
                invert: if flash > 0.5 { 1.0 } else { 0.0 },
                flash: 0.3 * flash,
                glitch: (0.6 * chaos * snap.min(1.0) + 0.25 * punch.min(1.0)).min(0.8) * motion,
                glitch_seed: self.tear as f32,
                relief: 0.0,
            },
        }
    }
}
