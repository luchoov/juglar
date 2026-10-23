//! Fracture: the screen split into territories of flat color, each the
//! ground nearest one seed, divided by hard seams, like a cracked glaze or
//! a map of a war. The music changes how broken it is, not how fast it
//! moves: every kick breaks the plate one stage further (a few big
//! territories become more and smaller ones), every snare drops a cluster
//! of seeds somewhere that splinters a territory into shards flashing as
//! they form, and an impact shatters everything outward from one point.
//! Between hits the stage heals back, slowly. At each kick the old paint
//! slides cell by cell, so colors smear along the seams before the new
//! territories fill in. Glitch made of geometry, with nothing black.
//!
//! | Signal   | Moves                                                     |
//! |----------|-----------------------------------------------------------|
//! | kick, hit| one stage more broken; the seeds jolted; the cells slide  |
//! | snare    | a splinter: a cluster of new seeds, flashing              |
//! | impact   | a shatter from one point, a flash; seams turn bright      |
//! | chaos    | how broken it may get, how far the cells slide            |
//! | loudness | how bright the territories are                            |
//! | harmonic | how fast the colors drift                                 |

use juglar_signals::{Signals, WAVEFORM_POINTS};

use super::react::{Pulse, Reactor, random};
use super::{Character, Fx, Look, Material, Palette, Scene};

/// The whole rainbow: each territory takes one of two opposite colors,
/// and the pair turns at every cut.
const PALETTE: Palette = Palette {
    a: [0.5, 0.5, 0.5],
    b: [0.5, 0.5, 0.5],
    c: [1.0, 1.0, 1.0],
    d: [0.0, 0.33, 0.67],
};

/// Seeds at most: four numbers each (x, y, color, glow) in the waveform's
/// slots.
const SEEDS: usize = WAVEFORM_POINTS / 4;
/// Seeds in use when the plate is whole, and when it is as broken as the
/// music lets it be.
const WHOLE: f32 = 5.0;
const BROKEN: f32 = 48.0;
/// How far the seeds may wander from the middle, in units of the short
/// side: a little past a 16:9 screen's edges.
const BOUNDS: [f32; 2] = [0.95, 0.55];

#[derive(Clone, Copy)]
struct Seed {
    at: [f32; 2],
    velocity: [f32; 2],
    color: f32,
    glow: f32,
}

pub(crate) struct Fracture {
    reactor: Reactor,
    seeds: [Seed; SEEDS],
    /// How broken the plate is: how many seeds are in use.
    stage: f32,
    /// The oldest seed, the next to be moved by a splinter.
    oldest: usize,
    slide: u32,
    dice: u32,
    hue: f32,
}

impl Fracture {
    pub fn new() -> Self {
        let mut dice = 0xf4ac;
        let seeds = std::array::from_fn(|_| Seed {
            at: [BOUNDS[0] * random(&mut dice), BOUNDS[1] * random(&mut dice)],
            velocity: [0.0; 2],
            color: Self::color(&mut dice),
            glow: 0.0,
        });
        Self { reactor: Reactor::new(0xf4ac), seeds, stage: WHOLE, oldest: 0, slide: 0, dice, hue: 0.0 }
    }

    /// One of two opposite colors, give or take a little.
    fn color(dice: &mut u32) -> f32 {
        let side = if random(dice) > 0.0 { 0.5 } else { 0.0 };
        side + 0.06 * random(dice)
    }

    /// A cluster of `count` seeds flung out from `at`, flashing.
    fn burst(&mut self, at: [f32; 2], count: usize, radius: f32, speed: f32) {
        let color = Self::color(&mut self.dice);
        for _ in 0..count {
            let angle = std::f32::consts::PI * random(&mut self.dice);
            let distance = radius * (0.5 + 0.5 * random(&mut self.dice)).powi(2);
            let (sin, cos) = angle.sin_cos();
            let seed = &mut self.seeds[self.oldest];
            seed.at = [at[0] + cos * distance, at[1] + sin * distance];
            seed.velocity = [cos * speed, sin * speed];
            seed.color = color + 0.5 * (random(&mut self.dice) > 0.4) as u32 as f32;
            seed.glow = 1.0;
            self.oldest = (self.oldest + 1) % SEEDS;
        }
    }
}

impl Scene for Fracture {
    fn name(&self) -> &'static str {
        "fracture"
    }

    /// Broken further at every hit.
    fn character(&self) -> Character {
        Character { energy: 0.9, pulse: 0.95, flow: true }
    }

    fn shader(&self) -> &'static str {
        include_str!("fracture.wgsl")
    }

    fn update(&mut self, s: &Signals, motion: f32) -> Look {
        let dt = s.dt.clamp(0.0, 0.1);
        let moving = dt * motion;
        let Pulse { punch, snap, shake, flash, cuts, chaos, harmony, kicked, snared, impacted, .. } = self.reactor.update(s, motion);
        let punch = punch.max(0.0);
        let live = motion * (0.5 + s.intensity.clamp(0.0, 1.0));
        self.hue += moving * (0.02 + 0.15 * harmony);

        // The stage: a kick breaks it further, up to as far as the chaos
        // lets it; it heals back over a few seconds.
        let deepest = WHOLE + (BROKEN - WHOLE) * (0.35 + 0.65 * chaos);
        if motion > 0.0 {
            if kicked {
                self.stage = (self.stage * 1.35 + 2.0).min(deepest);
                self.slide = self.slide.wrapping_add(1);
            }
            self.stage += (WHOLE - self.stage) * (1.0 - (-moving / 4.0).exp());
            if snared {
                let at = [0.8 * BOUNDS[0] * random(&mut self.dice), 0.8 * BOUNDS[1] * random(&mut self.dice)];
                self.burst(at, 5, 0.12, 0.25);
            }
            if impacted {
                let at = [0.4 * random(&mut self.dice), 0.3 * random(&mut self.dice)];
                self.burst(at, SEEDS, 1.0, 0.6);
                self.stage = BROKEN;
            }
        }

        // The seeds drift, jolt at kicks and bounce off the bounds.
        let jolt = if kicked { 0.35 * live } else { 0.0 };
        let damping = (-moving / 0.35).exp();
        let fade = (-moving / 0.3).exp();
        for seed in &mut self.seeds {
            for axis in 0..2 {
                let push = jolt * random(&mut self.dice) + 0.3 * moving * random(&mut self.dice);
                seed.velocity[axis] = seed.velocity[axis] * damping + push;
                seed.at[axis] += seed.velocity[axis] * moving;
                if seed.at[axis].abs() > BOUNDS[axis] {
                    seed.at[axis] = seed.at[axis].clamp(-BOUNDS[axis], BOUNDS[axis]);
                    seed.velocity[axis] = -seed.velocity[axis];
                }
            }
            seed.glow *= fade;
        }
        // The newest seeds first, so a splinter is among those in use.
        let count = (self.stage.round() as usize).clamp(1, SEEDS);
        let mut slots = [0.0; WAVEFORM_POINTS];
        for k in 0..count {
            let seed = &self.seeds[(self.oldest + SEEDS - 1 - k) % SEEDS];
            slots[4 * k..4 * k + 4].copy_from_slice(&[seed.at[0], seed.at[1], seed.color, seed.glow * motion]);
        }

        // On every other cut, the seams turn bright and the territories
        // darken, like lead between stained glass turned to light.
        let lit_seams = cuts % 2 == 1;
        Look {
            center: [0.0, 0.0],
            zoom: (live * 0.6 * punch * dt).exp(),
            rotation: 0.0,
            swirl: 0.0,
            decay: (-moving / 1.5).exp(),
            palette_drift: 0.2 * moving,
            knobs: [
                count as f32,                                        // seeds
                live * (0.6 + 3.0 * chaos) * punch.min(1.5) * dt,     // slide this frame
                self.slide as f32,                                    // the slide's seed
                0.08 + 0.3 * punch.min(1.0) + 0.5 * flash,            // fill per frame
                0.0025 + 0.002 * chaos + 0.004 * flash,               // seam half-width
                0.55 + 0.3 * s.loudness + 0.3 * punch.min(1.0),       // territories' ink
                if lit_seams { 1.0 } else { 0.0 },                    // bright seams
                0.0,
            ],
            palette: PALETTE,
            palette_shift: self.hue + (cuts % 7) as f32 * 0.13,
            palette_wrap: true,
            exposure: if lit_seams { 1.4 } else { 1.1 },
            bloom: if lit_seams { 0.5 } else { 0.2 },
            waveform: slots,
            spectrum: s.spectrum,
            material: Material::NONE,
            fx: Fx {
                shake: [0.02 * shake[0], 0.02 * shake[1]],
                chroma: 0.002 + 0.012 * chaos + 0.008 * punch.min(1.0),
                invert: 0.0,
                flash: 0.45 * flash,
                glitch: (0.4 * snap.abs().min(1.0) * chaos).min(0.5) * motion,
                glitch_seed: self.slide as f32 * 7.0,
                relief: 0.45,
                ground: 0.0,
                ground_hue: 0.0,
                stencil: 0.0,
            },
        }
    }
}
