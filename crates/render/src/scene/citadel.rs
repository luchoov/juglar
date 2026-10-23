//! Citadel: a flight through a city carved out of a fractal. The city is
//! a Mandelbox (a box folded into itself and scaled, over and over: halls,
//! arches, windows and blocks repeated at every size), drawn by marching
//! rays through it, with fog that pales the far halls and the cracks
//! between blocks left dark. Each frame is laid over the last only in
//! part, grain by grain, so the flight leaves trails and a dust of older
//! frames. The camera finds its own way: before each step it feels the
//! same fractal here, on the CPU, in a few directions, and turns toward
//! the most open one.
//!
//! The first of the "procedural 3D worlds", after studying MilkDrop's
//! "mandelbox explorer" presets by martin, fishbrain and flexi (D61);
//! written from scratch. The Mandelbox is Tom Lowe's public formula.
//!
//! Each layer answers to its own part of the music (D54). The flight is
//! the drums': the kick shoves the camera forward and jolts the fold's
//! scale, so the whole city flexes; the busier the playing, the faster it
//! flies. The cracks are the snare's: each one lights them for a moment.
//! The stone is the harmony's and the band's: held chords drift its
//! colors, and louder than the last seconds it warms and saturates. The
//! form is the chaos': heavy passages fold the city more times, denser
//! and sharper. Impacts jump the camera to another district; the other
//! hard cuts swerve it into an open hall.
//!
//! | Signal          | Moves                                               |
//! |-----------------|-----------------------------------------------------|
//! | kick, hit       | a shove forward, the fold's scale jolted (spring)   |
//! | snare           | the cracks lit, the camera's roll                   |
//! | cymbals         | grain: more of the frame left to older frames       |
//! | density         | how fast it flies                                   |
//! | chaos, grit     | how many folds: a denser, harder city               |
//! | energy, rel.    | how saturated and bright the stone is               |
//! | harmonic        | how fast the colors drift                           |
//! | impact          | a jump to another district, a flash                 |
//! | cut (wild)      | a swerve: the camera turns at once to an open hall  |

use juglar_signals::Signals;

use super::react::{Pulse, Reactor, random};
use super::space::{V3, add, camera, cross, dot, normalize, scale, turned};
use super::{Character, Fx, Look, Material, Palette, Scene};

/// Stone: from a teal lichen green, through a pale gray, to a cold lilac.
const PALETTE: Palette = Palette {
    a: [0.52, 0.62, 0.64],
    b: [0.18, 0.04, 0.12],
    c: [0.5, 0.5, 0.5],
    d: [0.5, 0.5, 0.5],
};

/// The fold's scale at rest; the kick jolts it from here.
const SCALE: f32 = 2.6;
/// Below this radius the sphere fold inverts a point fully; below 1 in
/// part. Squared, as the formula uses them.
const MIN_RADIUS2: f32 = 0.25;
const FIXED_RADIUS2: f32 = 1.0;
/// How close the camera may come to a wall, in the fractal's units.
const CLEARANCE: f32 = 0.01;
/// Folds the camera feels the city with: enough for the walls it could
/// hit, whatever the shader shows.
const PROBE_FOLDS: u32 = 9;

/// How far `p` is from the city's nearest wall (a lower bound): the same
/// fold as `citadel.wgsl`'s `city`.
fn city(p: V3, fold: f32, folds: u32) -> f32 {
    let mut z = p;
    let mut dr = 1.0;
    for _ in 0..folds {
        for c in &mut z {
            *c = c.clamp(-1.0, 1.0) * 2.0 - *c;
        }
        let r2 = dot(z, z);
        let k = if r2 < MIN_RADIUS2 { FIXED_RADIUS2 / MIN_RADIUS2 } else if r2 < FIXED_RADIUS2 { FIXED_RADIUS2 / r2 } else { 1.0 };
        z = add(scale(z, k * fold), p);
        dr = dr * k * fold.abs() + 1.0;
    }
    (dot(z, z).sqrt() / dr.abs()).max(0.0)
}

/// How far a ray from `from` along `toward` goes before it meets a wall,
/// up to `far`.
fn open(from: V3, toward: V3, fold: f32, far: f32) -> f32 {
    let mut t = 0.0;
    for _ in 0..32 {
        let d = city(add(from, scale(toward, t)), fold, PROBE_FOLDS);
        if d < 0.002 + 0.01 * t {
            break;
        }
        t += d;
        if t > far {
            return far;
        }
    }
    t
}

pub(crate) struct Citadel {
    reactor: Reactor,
    seed: u32,
    position: V3,
    forward: V3,
    up: V3,
    /// How far is open straight ahead, as last felt.
    ahead: f32,
    speed: f32,
    roll: f32,
    hue: f32,
    /// Folds the shader draws, held a while so they don't flicker.
    folds: f32,
    cuts: u32,
    /// A cut not yet flown: the next step swerves.
    swerve: bool,
    clock: f32,
}

impl Citadel {
    pub fn new() -> Self {
        let mut citadel = Self {
            reactor: Reactor::new(0xc17a),
            seed: 0x5eed_c17a,
            position: [0.0, 0.0, -3.0],
            forward: [0.0, 0.0, 1.0],
            up: [0.0, 1.0, 0.0],
            ahead: 1.0,
            speed: 0.0,
            roll: 0.0,
            hue: 0.0,
            folds: 8.0,
            cuts: 0,
            swerve: false,
            clock: 0.0,
        };
        citadel.jump(SCALE);
        citadel
    }

    /// Moves the camera to another district: of a few random places in
    /// the city, the one looking down the longest hall, up to a point.
    fn jump(&mut self, fold: f32) {
        let mut best = (-1.0, self.position, self.forward);
        for _ in 0..24 {
            let at = [1.6 * random(&mut self.seed), 1.6 * random(&mut self.seed), 1.6 * random(&mut self.seed)];
            if city(at, fold, PROBE_FOLDS) < 0.05 {
                continue;
            }
            let toward = normalize([random(&mut self.seed), random(&mut self.seed), random(&mut self.seed)]);
            // A long hall, but not a view out of the city.
            let far = open(at, toward, fold, 3.0).min(1.2);
            if far > best.0 {
                best = (far, at, toward);
            }
        }
        if best.0 > 0.0 {
            self.position = best.1;
            self.forward = best.2;
            self.ahead = best.0;
            let side = if self.forward[1].abs() < 0.9 { [0.0, 1.0, 0.0] } else { [1.0, 0.0, 0.0] };
            self.up = normalize(cross(cross(self.forward, side), self.forward));
        }
    }

    /// Feels the city around the camera and turns toward open space;
    /// moves `step` forward; keeps clear of the walls. Swerving, it turns
    /// at once, and away from straight ahead.
    fn fly(&mut self, fold: f32, step: f32, dt: f32, swerve: bool) {
        let right = normalize(cross(self.forward, self.up));
        // Straight ahead, and eight directions around it.
        self.ahead = self.ahead.min(open(self.position, self.forward, fold, 3.0));
        let mut best = (if swerve { 0.0 } else { self.ahead * 1.15 }, self.forward);
        for k in 0..8 {
            let around = k as f32 * std::f32::consts::TAU / 8.0;
            let axis = add(scale(right, around.sin()), scale(self.up, around.cos()));
            let toward = turned(self.forward, axis, 0.45);
            let far = open(self.position, toward, fold, 3.0);
            if far > best.0 {
                best = (far, toward);
            }
        }
        // Turn harder the closer the wall ahead.
        let urgency = (0.6 / self.ahead.max(0.05)).clamp(0.3, 4.0);
        let follow = if swerve { 0.8 } else { 1.0 - (-dt * urgency).exp() };
        self.forward = normalize(add(self.forward, scale(add(best.1, scale(self.forward, -1.0)), follow)));
        self.up = normalize(cross(cross(self.forward, self.up), self.forward));
        // Never further than the wall ahead allows.
        let step = step.min(0.3 * self.ahead);
        self.position = add(self.position, scale(self.forward, step));
        // Pushed back out if the city moved onto the camera.
        let here = city(self.position, fold, PROBE_FOLDS);
        if here < CLEARANCE {
            let e = 0.002;
            let slope = [
                city(add(self.position, [e, 0.0, 0.0]), fold, PROBE_FOLDS) - here,
                city(add(self.position, [0.0, e, 0.0]), fold, PROBE_FOLDS) - here,
                city(add(self.position, [0.0, 0.0, e]), fold, PROBE_FOLDS) - here,
            ];
            self.position = add(self.position, scale(normalize(slope), CLEARANCE - here + 0.002));
        }
        // Lost outside the city, or walled in: start somewhere else.
        if dot(self.position, self.position) > 25.0 || self.ahead < 0.004 {
            self.jump(fold);
        }
    }
}

impl Scene for Citadel {
    fn name(&self) -> &'static str {
        "citadel"
    }

    /// A heavy, driving flight.
    fn character(&self) -> Character {
        Character { energy: 0.85, pulse: 0.8, flow: true }
    }

    fn shader(&self) -> &'static str {
        include_str!("citadel.wgsl")
    }

    fn update(&mut self, s: &Signals, motion: f32) -> Look {
        let dt = s.dt.clamp(0.0, 0.1);
        let moving = dt * motion;
        let Pulse { punch, snap, slam, shake, flash, cuts, chaos, harmony, impacted, .. } = self.reactor.update(s, motion);
        let live = motion * (0.5 + s.intensity.clamp(0.0, 1.0));
        self.clock += moving;

        // The kick jolts the fold: the whole city flexes, and springs back.
        let fold = SCALE + motion * (0.05 * punch + 0.08 * slam);
        // An impact jumps to another district; the other cuts (hard hits,
        // twice a second at most while the music is wild) swerve.
        if cuts != self.cuts && motion > 0.0 {
            self.cuts = cuts;
            if impacted {
                self.jump(fold);
            } else {
                self.swerve = true;
            }
        }
        // Cruising with the density of the playing, shoved by kicks.
        let cruise = 0.05 + 0.12 * s.density + 0.1 * chaos;
        let target = cruise * (0.5 + 0.5 * self.ahead.min(1.0)) + 0.5 * punch.max(0.0);
        self.speed += (target - self.speed) * (1.0 - (-dt / 0.15).exp());
        if moving > 0.0 {
            let swerve = std::mem::take(&mut self.swerve);
            self.fly(fold, self.speed * moving * live, moving, swerve);
        }
        self.roll += moving * snap * 0.8;
        self.roll *= (-moving / 3.0).exp();
        self.hue += moving * (0.01 + 0.06 * harmony);
        // Heavy passages fold the city more: held so it doesn't flicker.
        let want = 6.0 + 5.0 * (0.6 * chaos + 0.4 * s.grit);
        if (want - self.folds).abs() > 1.2 {
            self.folds = want.round();
        }

        let swell = (s.relative.energy - 1.0).clamp(-0.5, 1.0);
        let heat = swell.max(0.0) * (0.4 + 0.6 * chaos);
        let mut camera = camera(self.position, self.forward, self.up, self.roll);
        // Grain: how much of the frame keeps older frames, pixel by pixel.
        camera[16] = 0.04 + 0.25 * motion * s.cymbals;
        let punch = punch.max(0.0);
        Look {
            center: [0.0, 0.0],
            // The trails stream outward as the camera moves in.
            zoom: (live * (0.4 * self.speed + 0.3 * punch) * dt).exp(),
            rotation: 0.0,
            swirl: 0.0,
            decay: (-moving / 1.5).exp(),
            palette_drift: 0.0,
            knobs: [
                fold,                                        // the fold's scale
                self.folds,                                  // folds drawn
                motion * s.snare.max(0.6 * snap.abs()),      // the cracks lit
                0.6 - 0.25 * punch.min(1.0),                 // how much of the frame is new
                0.3 + 0.2 * s.loudness + 0.2 * swell.max(0.0),  // light on the stone
                0.15,                                        // the fog's thickness
                self.clock,                                  // for the grain
                if impacted && motion > 0.0 { 1.0 } else { 0.0 },  // all of the frame new
            ],
            // Louder than the last seconds and wild, the stone takes color.
            palette: Palette { b: PALETTE.b.map(|b| b * (1.0 + 1.5 * heat)), ..PALETTE },
            // Mostly the greens; each cut leans a little toward the lilac
            // or away.
            palette_shift: 0.05 * self.hue.sin() + [0.0, 0.05, -0.05, 0.08][cuts as usize % 4],
            palette_wrap: false,
            exposure: 1.0 + 0.3 * swell,
            bloom: 0.2,
            waveform: camera,
            spectrum: s.spectrum,
            material: Material::NONE,
            fx: Fx {
                shake: [0.01 * shake[0], 0.01 * shake[1]],
                chroma: 0.001 + 0.005 * chaos,
                flash: 0.5 * flash,
                ..Fx::NONE
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Whether `p` is in the city's stone: its orbit under the fold stays
    /// near, where a point in the open escapes.
    fn in_stone(p: V3, fold: f32) -> bool {
        let mut z = p;
        for _ in 0..14 {
            for c in &mut z {
                *c = c.clamp(-1.0, 1.0) * 2.0 - *c;
            }
            let r2 = dot(z, z);
            let k = if r2 < MIN_RADIUS2 { FIXED_RADIUS2 / MIN_RADIUS2 } else if r2 < FIXED_RADIUS2 { FIXED_RADIUS2 / r2 } else { 1.0 };
            z = add(scale(z, k * fold), p);
            if dot(z, z) > 1e4 {
                return false;
            }
        }
        true
    }

    /// A minute of heavy music: kicks twice a second, snares between,
    /// an impact every fifteen seconds.
    #[test]
    fn the_flight_never_enters_the_stone_or_stalls() {
        let mut citadel = Citadel::new();
        let dt = 1.0 / 60.0;
        let (mut inside, mut jumps, mut impacts) = (0, 0, 0);
        for frame in 0..3600 {
            let time = frame as f32 * dt;
            let beat = time % 0.5;
            let signals = Signals {
                time,
                dt,
                kick: (-beat / 0.08).exp(),
                snare: (-((beat + 0.25) % 0.5) / 0.08).exp(),
                cymbals: 0.5,
                density: 0.8,
                chaos: 0.6,
                grit: 0.5,
                loudness: 0.8,
                intensity: 0.8,
                impact: if time % 15.0 < 0.05 && time > 1.0 { 1.0 } else { 0.0 },
                ..Signals::default()
            };
            let before = citadel.position;
            let look = citadel.update(&signals, 1.0);
            let moved = add(citadel.position, scale(before, -1.0));
            if dot(moved, moved) > 0.04 {
                jumps += 1;
            }
            if look.knobs[7] > 0.0 {
                impacts += 1;
            }
            if in_stone(citadel.position, look.knobs[0]) {
                inside += 1;
            }
        }

        assert_eq!(inside, 0, "frames spent inside the stone");
        // Every impact jumps; a few more from being walled in are fine, a
        // jump every few seconds is a camera that keeps getting stuck.
        assert!(impacts > 0 && jumps <= impacts + 6, "{jumps} jumps for {impacts} impacts");
    }
}
