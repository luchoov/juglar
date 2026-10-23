//! Etch: the whole screen a living texture, grown rather than drawn, like
//! acid eating a plate of metal. A faint grain is sown everywhere and the
//! old picture sharpens itself every frame (`Material`), so the grain
//! grows into spots, worms and veins that crowd each other into a
//! pattern, lit as an embossed surface and stirred slowly like a liquid.
//! Every kick sends a ring out from the middle that re-crystallizes the
//! pattern as it passes, the cymbals sow more grain, the snare twists the
//! stirring, and an impact wipes the plate clean so the pattern grows
//! back from nothing. At each cut the pattern changes species: finer or
//! coarser spots, a new set of colors. The music's chaos makes it boil.
//!
//! | Signal   | Moves                                                     |
//! |----------|-----------------------------------------------------------|
//! | kick, hit| a ring of fresh paint thrown out from the middle          |
//! | snare    | the stirring twisted                                      |
//! | cymbals  | grain sown                                                |
//! | impact   | the plate wiped, regrowing; a flash                       |
//! | chaos    | how hard the pattern sharpens: calm spots or boiling      |
//! | harmonic | how fast the colors drift                                 |
//! | waveform | the ring's edge                                           |

use juglar_signals::{Signals, WAVEFORM_POINTS};

use super::react::{Pulse, Reactor};
use super::{Character, Fx, Look, Material, Palette, Scene};

/// Verdigris and brass: dark teal, green, gold, copper.
const PALETTE: Palette = Palette {
    a: [0.45, 0.5, 0.35],
    b: [0.45, 0.4, 0.35],
    c: [1.0, 1.0, 1.0],
    d: [0.6, 0.45, 0.3],
};

/// The pattern's species, one per cut: how far the neighborhood reaches
/// (bigger, coarser spots) and the colors' place on the palette.
const SPECIES: [(f32, f32); 4] = [(3.0, 0.0), (5.0, 0.3), (2.5, 0.55), (7.0, 0.8)];

pub(crate) struct Etch {
    reactor: Reactor,
    waveform: [f32; WAVEFORM_POINTS],
    /// The ring thrown out by the last kick: how far out, how bright.
    ring: f32,
    ring_ink: f32,
    stir: f32,
    hue: f32,
    clock: f32,
}

impl Etch {
    pub fn new() -> Self {
        Self { reactor: Reactor::new(0xe7c4), waveform: [0.0; WAVEFORM_POINTS], ring: 1.0, ring_ink: 0.0, stir: 0.0, hue: 0.0, clock: 0.0 }
    }
}

impl Scene for Etch {
    fn name(&self) -> &'static str {
        "etch"
    }

    /// A texture that boils with the music.
    fn character(&self) -> Character {
        Character { energy: 0.9, pulse: 0.8, flow: true }
    }

    fn shader(&self) -> &'static str {
        include_str!("etch.wgsl")
    }

    fn update(&mut self, s: &Signals, motion: f32) -> Look {
        let dt = s.dt.clamp(0.0, 0.1);
        let moving = dt * motion;
        let Pulse { punch, snap, shimmer, shake, flash, turn, cuts, chaos, harmony, kicked, impacted, .. } = self.reactor.update(s, motion);
        let follow = 1.0 - (-moving / 0.02).exp();
        for (smooth, &raw) in self.waveform.iter_mut().zip(&s.waveform) {
            *smooth += (raw - *smooth) * follow;
        }
        let live = motion * (0.5 + s.intensity.clamp(0.0, 1.0));
        self.clock += moving;
        self.hue += moving * (0.01 + 0.08 * harmony);
        self.stir += moving * turn * (0.15 + 1.2 * snap.abs()) * live;
        // A new ring once the last is well on its way, so rings travel out
        // instead of starting over at every kick of a fast beat.
        if kicked && motion > 0.0 && self.ring > 0.3 {
            self.ring = 0.05;
            self.ring_ink = 1.0;
        }
        self.ring += moving * (0.9 + 1.5 * punch.max(0.0));
        self.ring_ink *= (-moving / 0.5).exp();

        let (reach, place) = SPECIES[cuts as usize % SPECIES.len()];
        Look {
            center: [0.0, 0.0],
            zoom: (live * (0.02 + 0.25 * punch.max(0.0)) * dt).exp(),
            rotation: 0.0,
            swirl: 0.0,
            decay: if impacted && motion > 0.0 { 0.0 } else { 1.0 },
            palette_drift: 0.05 * moving,
            knobs: [
                live * (0.03 + 0.05 * chaos) * dt,                   // stirring this frame
                self.stir,                                           // where the stirring is
                0.012 + 0.05 * shimmer.abs().min(1.0) + 0.04 * chaos, // grain sown per frame
                (self.clock * 30.0).floor(),                         // the grain's seed
                self.ring,                                           // the ring's radius
                self.ring_ink * motion,                              // the ring's ink
                0.05 + 0.08 * s.loudness,                            // the waveform's height on it
                place,                                               // the colors' place
            ],
            palette: PALETTE,
            palette_shift: self.hue,
            palette_wrap: true,
            exposure: 1.3,
            bloom: 0.25,
            waveform: self.waveform,
            spectrum: s.spectrum,
            material: Material {
                sharpen: 0.35 + 0.45 * chaos + 0.3 * punch.max(0.0).min(1.0),
                cap: 1.4,
                erode: 0.006,
                reach: reach * (1.0 + 0.3 * punch.max(0.0).min(1.0)),
            },
            fx: Fx {
                shake: [0.015 * shake[0], 0.015 * shake[1]],
                chroma: 0.002 + 0.008 * chaos,
                invert: if flash > 0.7 { 1.0 } else { 0.0 },
                flash: 0.35 * flash,
                glitch: 0.0,
                glitch_seed: 0.0,
                relief: 0.85,
                ground: 0.0,
                ground_hue: 0.0,
                stencil: 0.0,
            },
        }
    }
}
