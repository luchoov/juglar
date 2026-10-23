//! Datamosh: the spectrum as a barcode across the whole screen, smeared apart
//! the way a broken video stream smears. The picture is cut into blocks
//! and, at every hit, a few of them get a new direction to slide, so old
//! paint drags along the block edges into torn mosaics while bright
//! columns drip down like sorted pixels. An impact is a key frame: the
//! smear clears at once and the bars punch back in, sharp. Glitch for the
//! densest riffs: nothing is ever black for long.
//!
//! | Signal   | Moves                                                     |
//! |----------|-----------------------------------------------------------|
//! | spectrum | the barcode: each band's bar as wide as it is loud        |
//! | kick, hit| a new set of sliding blocks; how hard they slide          |
//! | snare    | slices shifted sideways                                   |
//! | impact   | a key frame: the smear cleared, a flash into the negative |
//! | chaos    | smaller blocks, more drip, torn colors                    |
//! | harmonic | how fast the colors drift                                 |

use juglar_signals::{SPECTRUM_BANDS, Signals};

use super::react::{Pulse, Reactor};
use super::{Character, Fx, Look, Material, Palette, Scene};

/// Acid: magenta, violet, blue, cyan, green, yellow, around.
const PALETTE: Palette = Palette {
    a: [0.5, 0.5, 0.5],
    b: [0.5, 0.5, 0.5],
    c: [1.0, 1.0, 1.0],
    d: [0.8, 0.9, 0.3],
};

pub(crate) struct Datamosh {
    reactor: Reactor,
    spectrum: [f32; SPECTRUM_BANDS],
    blocks: u32,
    scroll: f32,
    hue: f32,
    clock: f32,
}

impl Datamosh {
    pub fn new() -> Self {
        Self { reactor: Reactor::new(0xda7a), spectrum: [0.0; SPECTRUM_BANDS], blocks: 0, scroll: 0.0, hue: 0.0, clock: 0.0 }
    }
}

impl Scene for Datamosh {
    fn name(&self) -> &'static str {
        "datamosh"
    }

    /// A video stream breaking apart on every hit.
    fn character(&self) -> Character {
        Character { energy: 0.95, pulse: 0.9, flow: true }
    }

    fn shader(&self) -> &'static str {
        include_str!("datamosh.wgsl")
    }

    fn update(&mut self, s: &Signals, motion: f32) -> Look {
        let dt = s.dt.clamp(0.0, 0.1);
        let moving = dt * motion;
        let Pulse { punch, snap, shake, flash, turn, cuts, chaos, harmony, kicked, impacted, .. } = self.reactor.update(s, motion);
        let follow = 1.0 - (-moving / 0.03).exp();
        for (smooth, &raw) in self.spectrum.iter_mut().zip(&s.spectrum) {
            *smooth += (raw - *smooth) * follow;
        }
        let punch = punch.max(0.0);
        let live = motion * (0.5 + s.intensity.clamp(0.0, 1.0));
        self.clock += moving;
        self.hue += moving * (0.03 + 0.2 * harmony);
        self.scroll += moving * turn * (0.02 + 0.1 * chaos);
        if kicked {
            self.blocks = self.blocks.wrapping_add(1);
        }

        // The smear lasts until an impact clears it.
        let fade = (1.1 * (1.0 - 0.95 * flash)).max(0.03);
        Look {
            center: [0.0, 0.0],
            zoom: 1.0,
            rotation: 0.0,
            swirl: 0.0,
            // An impact is a key frame: the smear is gone at once.
            decay: if impacted && motion > 0.0 { 0.0 } else { (-moving / fade).exp() },
            palette_drift: (0.15 + 0.4 * harmony) * moving,
            knobs: [
                live * (0.8 + 6.0 * punch + 2.0 * chaos) * dt, // block drift this frame
                6.0 + 10.0 * chaos + 4.0 * (cuts % 3) as f32,  // blocks across the short side
                self.blocks as f32,                            // the blocks' seed
                1.0 + 0.8 * punch,                             // bar ink
                self.scroll,                                   // the bars' scroll
                live * (0.05 + 0.4 * chaos) * dt,              // drip this frame
                0.04 + 0.2 * punch.min(1.0) + 0.6 * flash,     // bar coverage per frame
                0.0,
            ],
            palette: PALETTE,
            palette_shift: self.hue + (cuts % 5) as f32 * 0.23,
            palette_wrap: true,
            exposure: 1.5 + 0.5 * s.loudness,
            bloom: 0.3,
            waveform: s.waveform,
            spectrum: self.spectrum,
            material: Material::NONE,
            fx: Fx {
                shake: [0.015 * shake[0], 0.01 * shake[1]],
                chroma: 0.003 + 0.015 * chaos,
                invert: if flash > 0.6 { 1.0 } else { 0.0 },
                flash: 0.3 * flash,
                glitch: (0.5 * snap.abs().min(1.0) * (0.3 + chaos)).min(0.7) * motion,
                glitch_seed: self.blocks as f32 * 3.0 + (self.clock * 6.0).floor(),
                relief: 0.0,
                ground: 0.1,
                ground_hue: 0.6,
                stencil: 0.0,
            },
        }
    }
}
