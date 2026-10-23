//! Maelstrom: a whirlpool. The old picture is sucked toward the middle and
//! turned, and each ring of it turns by its own amount, one way at some
//! distances and the other way at others, so the whirlpool shears into
//! bands. Every kick winds those bands tight and lets them spring back;
//! three blades sweep around, as long as their thirds of the spectrum are
//! loud, and a ring of spectrum bars sinks with them. The paint is lit as
//! a surface, so it reads as a heavy, carved liquid. An impact throws the
//! flow outward for a moment.
//!
//! | Signal   | Moves                                                     |
//! |----------|-----------------------------------------------------------|
//! | kick, hit| the rings' twist (a spring), a pull inward                |
//! | snare    | the turn, which reverses on impacts and every 4th snare   |
//! | spectrum | the blades' length and width, the ring's bars             |
//! | impact   | the flow thrown outward, a flash                          |
//! | chaos    | how tightly the rings are packed, torn colors             |
//! | harmonic | how fast the colors cycle                                 |

use juglar_signals::{SPECTRUM_BANDS, Signals};

use super::react::{Pulse, Reactor};
use super::{Character, Fx, Look, Material, Palette, Scene};

/// The whole rainbow, as MilkDrop's classics have it.
const PALETTE: Palette = Palette {
    a: [0.5, 0.5, 0.5],
    b: [0.5, 0.5, 0.5],
    c: [1.0, 1.0, 1.0],
    d: [0.0, 0.33, 0.67],
};

pub(crate) struct Maelstrom {
    reactor: Reactor,
    spectrum: [f32; SPECTRUM_BANDS],
    blades: f32,
    phase: f32,
    hue: f32,
}

impl Maelstrom {
    pub fn new() -> Self {
        Self { reactor: Reactor::new(0x3a1e), spectrum: [0.0; SPECTRUM_BANDS], blades: 0.0, phase: 0.0, hue: 0.0 }
    }
}

impl Scene for Maelstrom {
    fn name(&self) -> &'static str {
        "maelstrom"
    }

    /// A whirlpool wound by every kick.
    fn character(&self) -> Character {
        Character { energy: 0.85, pulse: 0.8, flow: true }
    }

    fn shader(&self) -> &'static str {
        include_str!("maelstrom.wgsl")
    }

    fn update(&mut self, s: &Signals, motion: f32) -> Look {
        let dt = s.dt.clamp(0.0, 0.1);
        let moving = dt * motion;
        let Pulse { punch, snap, slam, shake, flash, turn, chaos, harmony, .. } = self.reactor.update(s, motion);
        let follow = 1.0 - (-moving / 0.04).exp();
        for (smooth, &raw) in self.spectrum.iter_mut().zip(&s.spectrum) {
            *smooth += (raw - *smooth) * follow;
        }
        let live = motion * (0.5 + s.intensity.clamp(0.0, 1.0));
        self.blades += moving * live * turn * (0.8 + 3.0 * snap.abs() + chaos);
        self.phase += moving * (2.0 + 6.0 * punch.max(0.0));
        self.hue += moving * (0.05 + 0.35 * harmony);

        Look {
            center: [0.0, 0.0],
            // Sucked in, thrown out by an impact.
            zoom: (live * (-0.25 - 0.8 * punch.max(0.0) + 6.0 * slam.max(0.0)) * dt).exp(),
            rotation: live * turn * (0.5 + 1.5 * snap.abs()) * dt,
            swirl: live * turn * (1.0 + 2.0 * chaos) * dt,
            decay: (-moving / (0.9 + 0.6 * harmony)).exp(),
            palette_drift: 0.4 * moving,
            knobs: [
                live * (0.4 + 6.0 * punch) * dt, // twist this frame (springs back as punch recoils)
                self.phase,                       // the twist's phase
                self.blades,                      // the blades' angle
                0.6,                              // blade length
                0.3 + 0.05 * punch,               // spectrum ring radius
                0.16,                             // bar length
                1.3 + 1.2 * punch.max(0.0),       // blade ink
                10.0 + 14.0 * chaos,              // rings' spacing
            ],
            palette: PALETTE,
            palette_shift: self.hue,
            palette_wrap: true,
            exposure: 1.1 + 0.4 * s.loudness,
            bloom: 0.4,
            waveform: s.waveform,
            spectrum: self.spectrum,
            material: Material::NONE,
            fx: Fx {
                shake: [0.02 * shake[0], 0.02 * shake[1]],
                chroma: 0.002 + 0.01 * chaos + 0.008 * punch.max(0.0).min(1.0),
                invert: 0.0,
                flash: 0.55 * flash,
                glitch: 0.0,
                glitch_seed: 0.0,
                relief: 0.6,
                ground: 0.22,
                ground_hue: 0.5,
                stencil: 0.0,
            },
        }
    }
}
