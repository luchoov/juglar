//! Shrapnel: a barbed ring drawn by the waveform, blown apart by the drums.
//! Every frame the old picture is thrown outward, faster along the barbs
//! than between them, so the echoes tear into teeth; in a wall of sound it
//! is folded into mirror wedges, whose number changes at every hard cut.
//! Made for thrash and death metal: short trails cut by every hit, the
//! spin reversing at once, the lens tearing into its colors.
//!
//! | Signal   | Moves                                                     |
//! |----------|-----------------------------------------------------------|
//! | kick, hit| the blast outward and the barbs' length (a spring)        |
//! | snare    | the spin, which reverses on impacts and every 4th snare   |
//! | impact   | a huge blast, a flash into the negative, a new fold       |
//! | waveform | the ring's shape                                          |
//! | loudness | how far the wave bends the ring                           |
//! | chaos    | more wedges, torn colors, slices shifted sideways         |
//! | harmonic | how far the colors drift as they fly out                  |

use juglar_signals::{Signals, WAVEFORM_POINTS};

use super::react::{Pulse, Reactor};
use super::{Character, Fx, Look, Palette, Scene};

/// Fire and electric blue: orange, blood red, crimson, deep blue, gold.
const PALETTE: Palette = Palette {
    a: [0.5, 0.2, 0.25],
    b: [0.5, 0.25, 0.45],
    c: [1.0, 1.0, 1.0],
    d: [0.0, 0.15, 0.5],
};

/// Mirror wedges at each cut, in turn while the music is wild.
const FOLDS: [f32; 6] = [6.0, 0.0, 8.0, 5.0, 0.0, 7.0];
/// Barbs around the ring at each cut.
const BARBS: [f32; 5] = [9.0, 13.0, 7.0, 11.0, 16.0];

pub(crate) struct Shrapnel {
    reactor: Reactor,
    /// A lightly smoothed waveform: it may tremble, it is shrapnel.
    waveform: [f32; WAVEFORM_POINTS],
    spin: f32,
    hue: f32,
    clock: f32,
}

impl Shrapnel {
    pub fn new() -> Self {
        Self { reactor: Reactor::new(0x5eed), waveform: [0.0; WAVEFORM_POINTS], spin: 0.0, hue: 0.0, clock: 0.0 }
    }
}

impl Scene for Shrapnel {
    fn name(&self) -> &'static str {
        "shrapnel"
    }

    /// Blown apart by every hit: the wildest.
    fn character(&self) -> Character {
        Character { energy: 1.0, pulse: 1.0, flow: true }
    }

    fn shader(&self) -> &'static str {
        include_str!("shrapnel.wgsl")
    }

    fn update(&mut self, s: &Signals, motion: f32) -> Look {
        let dt = s.dt.clamp(0.0, 0.1);
        let moving = dt * motion;
        let Pulse { punch, snap, slam, shake, flash, turn, cuts, chaos, harmony, .. } = self.reactor.update(s, motion);
        let follow = 1.0 - (-moving / 0.02).exp();
        for (smooth, &raw) in self.waveform.iter_mut().zip(&s.waveform) {
            *smooth += (raw - *smooth) * follow;
        }
        let (punch, snap, slam) = (punch.max(0.0), snap.abs(), slam.max(0.0));
        let live = motion * (0.5 + s.intensity.clamp(0.0, 1.0));
        self.clock += moving;
        self.spin += moving * live * turn * (0.5 + 4.0 * snap + 1.5 * chaos);
        self.hue += moving * (0.02 + 0.3 * harmony);
        // A cut jumps the colors and picks a new fold and barbs.
        let cut = cuts as usize;
        let hue_jump = (cut % 7) as f32 * 0.19;
        let wedges = if chaos > 0.45 { FOLDS[cut % FOLDS.len()] } else { 0.0 };

        // Trails are short and every hit cuts them shorter.
        let fade = (0.32 - 0.22 * punch.min(1.0) - 0.1 * chaos).max(0.06);
        Look {
            center: [0.0, 0.0],
            zoom: (live * (0.4 + 3.5 * punch + 7.0 * slam + 1.2 * chaos) * dt).exp(),
            rotation: live * turn * (0.3 + 2.5 * snap) * dt,
            swirl: live * turn * 1.5 * snap * dt,
            decay: (-moving / fade).exp(),
            palette_drift: (0.3 + 0.8 * harmony) * moving,
            knobs: [
                0.11 + 0.05 * punch + 0.12 * slam,         // ring radius
                motion * (0.05 + 0.1 * s.loudness),         // wave height
                BARBS[cut % BARBS.len()],                    // barbs
                motion * (0.015 + 0.09 * punch + 0.05 * chaos), // barb length
                self.spin,                                   // the ring's turn
                wedges,                                      // mirror wedges
                live * (0.6 * punch + 1.5 * slam) * dt,      // serration this frame
                0.9 + 0.8 * punch,                           // ink
            ],
            palette: PALETTE,
            palette_shift: self.hue + hue_jump,
            palette_wrap: true,
            exposure: 1.1 + 0.5 * s.loudness,
            bloom: 0.55 + 0.3 * punch.min(1.0),
            waveform: self.waveform,
            spectrum: s.spectrum,
            fx: Fx {
                shake: [0.025 * shake[0], 0.025 * shake[1]],
                chroma: 0.002 + 0.012 * punch.min(1.5) + 0.01 * chaos,
                invert: if flash > 0.75 { 1.0 } else { 0.0 },
                flash: 0.6 * flash,
                glitch: if chaos > 0.6 { 0.35 * snap.min(1.0) } else { 0.0 },
                glitch_seed: cuts as f32 + (self.clock * 12.0).floor(),
                relief: 0.0,
                ground: 0.0,
                ground_hue: 0.0,
                stencil: 0.0,
            },
        }
    }
}
