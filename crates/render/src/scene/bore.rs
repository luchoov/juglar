//! Bore: a flight down a tunnel drilled through fractal rock, toward a
//! white-hot core that is always further on. The rock is a kaleidoscopic
//! fold (KIFS: space folded onto itself across a few mirrors, turned,
//! scaled, again and again), twisted around the tunnel as it winds, so the
//! walls are ribs, teeth and side caverns that spin past. Drawn by marching
//! rays, lit only by the core ahead: what is far and to the side is black.
//!
//! The second of the "procedural 3D worlds", after Citadel (D61); its look
//! after studying MilkDrop's "tunnel race" by martin (a hot core at the
//! vanishing point, speed), written from scratch (D63).
//!
//! The tunnel's course is known in advance (a sum of slow waves), so the
//! camera needs no feeling its way, unlike Citadel's: it rides the course,
//! looking a little ahead along it.
//!
//! Each layer answers to its own part of the music (D54). The speed is the
//! kick's and the density's: each kick shoves the camera on, and the busier
//! the playing, the faster it drills. The rock's folds are the snare's:
//! each snare turns the mirrors a notch, so the walls reshape as they pass.
//! The cymbals strike sparks off the walls. The core is the band's: louder
//! than the last seconds, it burns whiter and lights more of the rock.
//! The rock's roughness is the chaos': wild passages fold it more times,
//! jagged. An impact blasts the tunnel open into a cavern, down another
//! branch, and the rock closes back in over a second.
//!
//! | Signal          | Moves                                               |
//! |-----------------|-----------------------------------------------------|
//! | kick, hit       | a shove forward, the view widening for a moment     |
//! | density         | how fast it drills                                  |
//! | snare           | the mirrors turned a notch: the walls reshape       |
//! | cymbals         | sparks struck off the walls                         |
//! | energy, rel.    | how hot and far-reaching the core's light is        |
//! | chaos, grit     | how many folds: rougher, more jagged rock           |
//! | harmonic        | how fast the colors drift                           |
//! | impact          | a new branch, blasted open, the rock closing back in |

use juglar_signals::Signals;

use super::react::{Pulse, Reactor, Spring};
use super::space::{V3, camera, normalize};
use super::{Character, Fx, Look, Material, Palette, Scene};

/// Heat: from a deep red, through orange, to a pale amber.
const PALETTE: Palette = Palette {
    a: [0.7, 0.42, 0.25],
    b: [0.25, 0.3, 0.25],
    c: [0.5, 0.5, 0.5],
    d: [0.5, 0.5, 0.5],
};

/// How far ahead along the course the camera looks.
const LOOK_AHEAD: f32 = 1.2;
/// How far the camera flies before the course starts over (an impact's
/// branch); far enough that it never shows, near enough for precision.
const WRAP: f32 = 2048.0;

/// The tunnel's course: where its middle is at depth `z`, on branch
/// `branch`. The same waves as `bore.wgsl`'s `course`.
pub(crate) fn course(z: f32, branch: f32) -> [f32; 2] {
    [
        1.4 * (0.21 * z + branch).sin() + 0.9 * (0.13 * z + 2.0 * branch).sin(),
        1.1 * (0.17 * z + 1.7 * branch).cos() + 0.5 * (0.29 * z).sin(),
    ]
}

fn at(z: f32, branch: f32) -> V3 {
    let [x, y] = course(z, branch);
    [x, y, z]
}

pub(crate) struct Bore {
    reactor: Reactor,
    /// How deep the camera is along the course.
    depth: f32,
    speed: f32,
    branch: f32,
    /// The mirrors' turn: a phase notched by each snare, swinging the
    /// angle through the range where the fold makes rock; eased toward.
    turn: f32,
    turning: f32,
    roll: f32,
    /// The tunnel blasted open at an impact, and closing back in.
    blast: Spring,
    hue: f32,
    folds: f32,
    clock: f32,
}

impl Bore {
    pub fn new() -> Self {
        Self {
            reactor: Reactor::new(0xb03e),
            depth: 0.0,
            speed: 0.0,
            branch: 0.0,
            turn: 0.0,
            turning: 0.4,
            roll: 0.0,
            blast: Spring::new(0.5, 0.9),
            hue: 0.0,
            folds: 4.0,
            clock: 0.0,
        }
    }
}

impl Scene for Bore {
    fn name(&self) -> &'static str {
        "bore"
    }

    /// A fast, driving flight.
    fn character(&self) -> Character {
        Character { energy: 0.9, pulse: 0.9, flow: true }
    }

    fn shader(&self) -> &'static str {
        include_str!("bore.wgsl")
    }

    fn update(&mut self, s: &Signals, motion: f32) -> Look {
        let dt = s.dt.clamp(0.0, 0.1);
        let moving = dt * motion;
        let Pulse { punch, snap, slam, shake, flash, chaos, harmony, snared, impacted, .. } = self.reactor.update(s, motion);
        let live = motion * (0.5 + s.intensity.clamp(0.0, 1.0));
        self.clock += moving;

        // An impact: a new branch, blasted open, closing back in.
        if impacted && motion > 0.0 {
            self.branch += 2.4;
            self.blast.strike(1.0);
        }
        let blast = self.blast.step(moving).clamp(0.0, 1.0);
        // Drilling with the density of the playing, shoved by kicks.
        let punch = punch.max(0.0);
        let target = 0.6 + 1.6 * s.density + 0.8 * chaos + 4.0 * punch;
        self.speed += (target - self.speed) * (1.0 - (-dt / 0.12).exp());
        self.depth += self.speed * moving * live;
        if self.depth > WRAP {
            self.depth -= WRAP;
            self.branch += 2.4;
        }
        // Each snare turns the mirrors a notch; they ease there. Past about
        // a radian the fold leaves no rock, so the angle swings back and
        // forth: through ribbed rings, porous rock, shards and loose debris.
        if snared && motion > 0.0 {
            self.turn += 0.3;
        }
        let angle = 0.4 + 0.5 * self.turn.sin();
        self.turning += (angle - self.turning) * (1.0 - (-moving / 0.08).exp());
        self.roll += moving * (0.15 + 0.6 * snap);
        self.hue += moving * (0.01 + 0.05 * harmony);
        // Wild passages fold the rock more: held so it doesn't flicker.
        let want = 4.0 + 3.0 * (0.6 * chaos + 0.4 * s.grit);
        if (want - self.folds).abs() > 0.8 {
            self.folds = want.round();
        }

        let eye = at(self.depth, self.branch);
        let ahead = at(self.depth + LOOK_AHEAD, self.branch);
        let forward = normalize([ahead[0] - eye[0], ahead[1] - eye[1], ahead[2] - eye[2]]);
        let side = normalize([forward[2], 0.0, -forward[0]]);
        let up = normalize(super::space::cross(side, forward));
        let swell = (s.relative.energy - 1.0).clamp(-0.5, 1.0);
        let mut camera = camera(eye, forward, up, self.roll);
        // The course's branch, and the core's heat, for the shader.
        camera[16] = self.branch;
        camera[17] = 1.5 + 1.2 * s.loudness + 1.8 * swell.max(0.0) + 2.0 * slam.max(0.0);
        Look {
            center: [0.0, 0.0],
            // Streaks rush outward from the core as the camera drills in.
            zoom: (live * (0.08 * self.speed + 0.4 * punch) * dt).exp(),
            rotation: 0.0,
            swirl: 0.0,
            decay: (-moving / 0.6).exp(),
            palette_drift: 0.0,
            knobs: [
                self.depth,                                    // how deep
                self.turning,                                  // the mirrors' turn
                self.folds,                                    // folds drawn
                0.7 - 0.3 * punch.min(1.0),                    // how much of the frame is new
                1.0 + 1.8 * blast,                             // how open the tunnel is
                motion * s.cymbals.max(0.0),                   // sparks
                self.clock,                                    // for the sparks
                if impacted && motion > 0.0 { 1.0 } else { 0.0 }, // all of the frame new
            ],
            palette: PALETTE,
            palette_shift: 0.05 * self.hue.sin(),
            palette_wrap: false,
            exposure: 1.0 + 0.3 * swell,
            bloom: 0.35,
            waveform: camera,
            spectrum: s.spectrum,
            material: Material::NONE,
            fx: Fx {
                shake: [0.012 * shake[0], 0.012 * shake[1]],
                chroma: 0.001 + 0.004 * chaos,
                flash: 0.6 * flash,
                ..Fx::NONE
            },
        }
    }
}
