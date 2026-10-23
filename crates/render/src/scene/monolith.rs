//! Monolith: huge slabs pressed into a field of clay. Two or three dark
//! polygons, each a third of the screen, turn slowly over an ochre
//! ground; every kick presses them in, so they sink and spring back, and
//! each press leaves its outline in the clay, which spreads outward as
//! ripples of contours lit like a relief. Held notes carve chambers into
//! the slabs, outline inside outline, more the longer the harmony holds;
//! an impact drives them down hard and turns them into new shapes. For
//! doom and sludge: slow, heavy, and nothing black.
//!
//! | Signal   | Moves                                                     |
//! |----------|-----------------------------------------------------------|
//! | kick, hit| the press: slabs sink and spring back, a contour left     |
//! | snare    | a jolt of the slabs' turn                                 |
//! | impact   | a slam, new shapes, a flash                               |
//! | harmonic | chambers carved inside the slabs                          |
//! | chaos    | torn colors, sliced glitches                              |
//! | bass     | its swell over the last seconds presses too (doom's fuzz)  |
//! | loudness | how far the contours spread                               |

use juglar_signals::{Signals, WAVEFORM_POINTS};

use super::react::{Pulse, Reactor};
use super::{Character, Fx, Look, Palette, Scene};

/// Earth: bone, ochre, rust, umber.
const PALETTE: Palette = Palette {
    a: [0.55, 0.42, 0.3],
    b: [0.35, 0.28, 0.2],
    c: [1.0, 1.0, 1.0],
    d: [0.0, 0.04, 0.1],
};

/// Slabs at most; eight numbers each in the waveform's slots.
const SLABS: usize = 3;

/// The slabs at each cut: (sides, x, y, size) for each.
const SETS: [&[(f32, f32, f32, f32)]; 4] = [
    &[(4.0, -0.35, 0.05, 0.3), (3.0, 0.4, -0.08, 0.26)],
    &[(6.0, 0.0, 0.0, 0.36)],
    &[(3.0, -0.5, 0.1, 0.22), (5.0, 0.0, -0.05, 0.26), (4.0, 0.5, 0.1, 0.22)],
    &[(5.0, -0.3, -0.05, 0.3), (6.0, 0.38, 0.08, 0.24)],
];

pub(crate) struct Monolith {
    reactor: Reactor,
    angle: f32,
    /// How many chambers are carved, grown by held notes.
    chambers: f32,
}

impl Monolith {
    pub fn new() -> Self {
        Self { reactor: Reactor::new(0x3011), angle: 0.0, chambers: 0.0 }
    }
}

impl Scene for Monolith {
    fn name(&self) -> &'static str {
        "monolith"
    }

    /// Heavy, slow presses.
    fn character(&self) -> Character {
        Character { energy: 0.85, pulse: 0.6, flow: true }
    }

    fn shader(&self) -> &'static str {
        include_str!("monolith.wgsl")
    }

    fn update(&mut self, s: &Signals, motion: f32) -> Look {
        let dt = s.dt.clamp(0.0, 0.1);
        let moving = dt * motion;
        let Pulse { punch, snap, slam, shake, flash, turn, cuts, chaos, harmony, .. } = self.reactor.update(s, motion);
        let live = motion * (0.5 + s.intensity.clamp(0.0, 1.0));
        self.angle += moving * turn * (0.06 + 0.8 * snap.abs()) * live;
        // Chambers grow while the harmony holds, and close when the drums
        // take over.
        let target = 4.0 * ((harmony - 0.3) / 0.5).clamp(0.0, 1.0);
        self.chambers += (target - self.chambers) * (1.0 - (-moving / 3.0).exp());

        // Pressed in by the kick, slammed by an impact: smaller, as if
        // further away, then back past where they were.
        // Doom's fuzz hides its hits, so the low end's swell against its
        // last seconds presses too.
        let swell = (s.relative.bass - 1.0).clamp(0.0, 1.0);
        let press = 0.3 * punch + 0.4 * slam + 0.25 * swell;
        let set = SETS[cuts as usize % SETS.len()];
        let mut slots = [0.0; WAVEFORM_POINTS];
        for (k, &(sides, x, y, size)) in set.iter().enumerate().take(SLABS) {
            let side = if k % 2 == 0 { 1.0 } else { -0.7 };
            let size = size * (1.0 - press * motion).max(0.4);
            slots[8 * k..8 * k + 5].copy_from_slice(&[x, y, size, sides, self.angle * side + k as f32]);
        }

        Look {
            center: [0.0, 0.0],
            // The contours spread outward from the slabs.
            zoom: (live * (0.06 + 0.1 * s.loudness + 0.5 * swell + 1.4 * punch.max(0.0) + 2.0 * slam.max(0.0)) * dt).exp(),
            rotation: 0.0,
            swirl: 0.0,
            decay: (-moving / 2.0).exp(),
            palette_drift: 0.05 * moving,
            knobs: [
                set.len().min(SLABS) as f32,              // slabs
                0.004 + 0.012 * punch.max(0.0) + 0.02 * slam.max(0.0), // contour half-width
                0.5 + 1.5 * punch.max(0.0) + 2.0 * slam.max(0.0),      // contour ink
                self.chambers,                            // chambers
                0.0,
                0.0,
                0.0,
                0.0,
            ],
            palette: PALETTE,
            palette_shift: (cuts % 4) as f32 * 0.12,
            palette_wrap: false,
            exposure: 1.1,
            bloom: 0.2,
            waveform: slots,
            spectrum: s.spectrum,
            fx: Fx {
                shake: [0.03 * shake[0], 0.03 * shake[1]],
                chroma: 0.001 + 0.008 * chaos,
                invert: 0.0,
                flash: 0.35 * flash,
                glitch: if chaos > 0.6 { 0.25 * snap.abs().min(1.0) * motion } else { 0.0 },
                glitch_seed: cuts as f32 * 5.0,
                relief: 0.9,
                ground: 0.8,
                ground_hue: 0.15,
                stencil: 1.0,
            },
        }
    }
}
