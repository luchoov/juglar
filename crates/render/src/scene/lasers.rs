//! Lasers: fans of beams cut across a flat field of red from emitters at
//! the screen's edges, like stage lasers through smoke. Each kick fires
//! them: the beams flare white and the fans spread wide, then narrow back
//! to thin lines. The snare sweeps them, the cymbals make single beams
//! flicker out and back, and at every cut the emitters jump to a new
//! rig (one fan from below, two crossing from the corners, three from the
//! sides, a star from the middle) with a new number of beams. The field
//! goes from red to steel blue and back with the cuts; its afterimages
//! are short, so the beams stay sharp.
//!
//! | Signal   | Moves                                                     |
//! |----------|-----------------------------------------------------------|
//! | kick, hit| the beams flare and the fans spread (springs)             |
//! | snare    | the sweep                                                 |
//! | cymbals  | beams flickering out                                      |
//! | impact   | a new rig, a flash                                        |
//! | chaos    | more beams, torn colors                                   |
//! | loudness | how bright the field is                                   |

use juglar_signals::{Signals, WAVEFORM_POINTS};

use super::react::{Pulse, Reactor};
use super::{Character, Fx, Look, Material, Palette, Scene};

/// Deep red at 0, steel blue halfway, pale gold and warm white (0.75-0.85)
/// for the beams.
const PALETTE: Palette = Palette {
    a: [0.6, 0.3, 0.3],
    b: [0.4, 0.3, 0.3],
    c: [1.0, 1.0, 1.0],
    d: [0.0, 0.25, 0.4],
};

/// Emitters at most; eight numbers each in the waveform's slots.
const EMITTERS: usize = 4;

/// An emitter: where it stands (units of the short side), where its fan
/// points, and how wide the fan is (radians).
type Emitter = ([f32; 2], f32, f32);

/// The rigs, one per cut in turn.
fn rig(which: u32) -> &'static [Emitter] {
    use std::f32::consts::{FRAC_PI_2, PI};
    const RIGS: [&[Emitter]; 4] = [
        &[([0.0, 0.62], -FRAC_PI_2, 1.9)],
        &[([-0.95, 0.6], -0.9, 1.1), ([0.95, 0.6], -PI + 0.9, 1.1)],
        &[([-1.0, 0.0], 0.0, 1.2), ([1.0, 0.0], PI, 1.2), ([0.0, -0.62], FRAC_PI_2, 1.4)],
        &[([0.0, 0.0], 0.0, 2.0 * PI)],
    ];
    RIGS[which as usize % RIGS.len()]
}

pub(crate) struct Lasers {
    reactor: Reactor,
    sweep: f32,
    clock: f32,
    flicker: u32,
}

impl Lasers {
    pub fn new() -> Self {
        Self { reactor: Reactor::new(0x1a5e), sweep: 0.0, clock: 0.0, flicker: 0 }
    }
}

impl Scene for Lasers {
    fn name(&self) -> &'static str {
        "lasers"
    }

    /// Light fired on every kick.
    fn character(&self) -> Character {
        Character { energy: 0.9, pulse: 1.0, flow: false }
    }

    fn shader(&self) -> &'static str {
        include_str!("lasers.wgsl")
    }

    fn update(&mut self, s: &Signals, motion: f32) -> Look {
        let dt = s.dt.clamp(0.0, 0.1);
        let moving = dt * motion;
        let Pulse { punch, snap, shimmer, shake, flash, turn, cuts, chaos, .. } = self.reactor.update(s, motion);
        let punch = punch.max(0.0);
        let live = motion * (0.5 + s.intensity.clamp(0.0, 1.0));
        self.clock += moving;
        // A slow scan, kicked along by the snare.
        self.sweep += moving * turn * (0.25 + 2.5 * snap.abs()) * live;
        // The cymbals pick which beams drop out, many times a second.
        if shimmer.abs() > 0.15 && (self.clock * 24.0) as u32 != ((self.clock - moving) * 24.0) as u32 {
            self.flicker = self.flicker.wrapping_add(1);
        }

        let rig = rig(cuts);
        let beams = (5 + 2 * (cuts % 3) + (8.0 * chaos) as u32) as f32;
        let mut slots = [0.0; WAVEFORM_POINTS];
        for (k, &(at, angle, spread)) in rig.iter().enumerate().take(EMITTERS) {
            // Each emitter scans its own way; fans spread on the kick.
            let side = if k % 2 == 0 { 1.0 } else { -1.0 };
            let scan = 0.35 * (self.sweep * side + k as f32).sin();
            let wide = (spread * (0.75 + 0.35 * punch.min(1.5))).min(std::f32::consts::TAU);
            slots[8 * k..8 * k + 5].copy_from_slice(&[at[0], at[1], angle + scan * motion, wide, beams]);
        }

        // Red and steel blue take turns with the cuts.
        let blue = cuts % 2 == 1;
        Look {
            center: [0.0, 0.0],
            zoom: (live * (0.02 + 0.3 * punch) * dt).exp(),
            rotation: 0.0,
            swirl: 0.0,
            decay: (-moving / (0.06 + 0.06 * chaos)).exp(),
            palette_drift: 0.0,
            knobs: [
                rig.len().min(EMITTERS) as f32,                // emitters
                0.003 + 0.006 * punch.min(1.5),                // beam half-width at the emitter
                1.3 + 4.0 * punch.min(1.5) + 2.0 * flash,      // beam ink
                self.flicker as f32,                           // which beams drop out
                (0.5 * shimmer.abs() + 0.2 * chaos).min(0.6),  // how many drop out
                0.0,
                0.0,
                0.0,
            ],
            palette: PALETTE,
            palette_shift: 0.0,
            palette_wrap: true,
            exposure: 1.2,
            bloom: 0.6,
            waveform: slots,
            spectrum: s.spectrum,
            material: Material::NONE,
            fx: Fx {
                shake: [0.012 * shake[0], 0.012 * shake[1]],
                chroma: 0.002 + 0.01 * chaos,
                invert: 0.0,
                flash: 0.5 * flash,
                glitch: 0.0,
                glitch_seed: 0.0,
                relief: 0.0,
                ground: 0.35 + 0.25 * s.loudness,
                ground_hue: if blue { 0.55 } else { 0.0 },
                stencil: 0.0,
            },
        }
    }
}
