//! Furnace: a wall of fire. Flames are fed all along the bottom edge, the
//! spectrum rising out of them as tongues, and climb through curl-noise
//! turbulence that tosses them into billows, slowing as they rise so the
//! fire piles up; sparks fly up with them. The paint cools as it ages,
//! from white through yellow and orange to dark red. Every kick makes the
//! fire roar (it climbs faster and bulges out from the middle) and an
//! impact tears a white-hot crack across it, empties the furnace and lets
//! the slam fill it again. Made for sludge, doom and grunge: weight and
//! heat, broken open by the drums.
//!
//! | Signal   | Moves                                                     |
//! |----------|-----------------------------------------------------------|
//! | spectrum | the flames along the bottom, bass in the middle           |
//! | kick, hit| the bulge (a spring)                                      |
//! | snare    | a twist, one way or the other                             |
//! | impact   | a crack drawn by the waveform, the old slag cleared, a jolt|
//! | chaos    | how hard the fire churns, torn colors, shifted slices     |
//! | loudness | how fast it rises, how tall the flames are                |
//! | harmonic | how slowly the fire cools                                 |

use juglar_signals::{SPECTRUM_BANDS, Signals};

use super::react::{Pulse, Reactor};
use super::{Character, Fx, Look, Material, Palette, Scene};

/// From yellow (0) through orange to dark red and violet smoke (1), never
/// looping: paint starts hot and cools as it ages. Green falls faster than
/// red, so the middle is orange, not brown.
const PALETTE: Palette = Palette {
    a: [0.65, 0.43, 0.15],
    b: [0.35, 0.42, 0.15],
    c: [0.4, 0.6, 0.8],
    d: [0.0, 0.0, 0.0],
};

pub(crate) struct Furnace {
    reactor: Reactor,
    spectrum: [f32; SPECTRUM_BANDS],
    clock: f32,
    churn: f32,
    /// What is left of the last impact's crack, 1 at the slam.
    crack: f32,
}

impl Furnace {
    pub fn new() -> Self {
        Self { reactor: Reactor::new(0xf0a9), spectrum: [0.0; SPECTRUM_BANDS], clock: 0.0, churn: 0.0, crack: 0.0 }
    }
}

impl Scene for Furnace {
    fn name(&self) -> &'static str {
        "furnace"
    }

    /// Heavy and slow, with hits that land like hammers.
    fn character(&self) -> Character {
        Character { energy: 0.8, pulse: 0.75, flow: true }
    }

    fn shader(&self) -> &'static str {
        include_str!("furnace.wgsl")
    }

    fn update(&mut self, s: &Signals, motion: f32) -> Look {
        let dt = s.dt.clamp(0.0, 0.1);
        let moving = dt * motion;
        let Pulse { punch, snap, slam, shake, flash, turn, chaos, harmony, impacted, .. } = self.reactor.update(s, motion);
        let follow = 1.0 - (-moving / 0.05).exp();
        for (smooth, &raw) in self.spectrum.iter_mut().zip(&s.spectrum) {
            *smooth += (raw - *smooth) * follow;
        }
        let (punch, snap, slam) = (punch.max(0.0), snap, slam.max(0.0));
        let live = motion * (0.5 + s.intensity.clamp(0.0, 1.0));
        self.clock += moving;
        self.churn += moving * (0.15 + 0.5 * chaos + 0.6 * punch);
        if impacted {
            self.crack = 1.0;
        }
        self.crack *= (-moving / 0.35).exp();

        // Fire lasts about a second; an impact clears it at once.
        let fade = (0.6 + 0.4 * harmony) * (1.0 - 0.9 * flash);
        Look {
            center: [0.0, 0.05],
            zoom: (live * (-0.15 + 2.2 * punch + 3.0 * slam) * dt).exp(),
            rotation: live * turn * 1.2 * snap * dt,
            swirl: live * turn * 2.0 * snap.abs() * dt,
            decay: if impacted && motion > 0.0 { 0.0 } else { (-moving / fade.max(0.05)).exp() },
            palette_drift: (0.6 - 0.25 * harmony) * moving,
            knobs: [
                live * (0.04 + 0.08 * chaos + 0.1 * punch) * dt, // turbulence this frame
                live * (0.35 + 0.3 * s.loudness + 0.8 * punch) * dt, // rise this frame
                live * 0.9 * punch * dt,                          // bulge this frame
                motion * (0.12 + 0.4 * s.loudness),               // flame height
                self.crack,                                       // crack ink
                self.churn,                                       // the churn's clock
                motion * (0.05 + 0.12 * slam),                    // crack height
                0.75,                                             // flame ink
            ],
            palette: PALETTE,
            palette_shift: 0.0,
            palette_wrap: false,
            exposure: 0.7 + 0.35 * s.loudness,
            bloom: 0.35 + 0.4 * self.crack,
            waveform: s.waveform,
            spectrum: self.spectrum,
            material: Material { sharpen: 0.06, cap: 2.5, erode: 0.0, reach: 1.5 },
            fx: Fx {
                shake: [0.04 * shake[0], 0.04 * shake[1]],
                chroma: 0.002 + 0.006 * punch.min(1.5) + 0.006 * chaos,
                invert: 0.0,
                flash: 0.5 * flash,
                glitch: if chaos > 0.65 { 0.2 * snap.abs().min(1.0) } else { 0.0 },
                glitch_seed: (self.clock * 15.0).floor(),
                relief: 0.3,
                ground: 0.0,
                ground_hue: 0.0,
                stencil: 0.0,
            },
        }
    }
}
