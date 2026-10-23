//! Wire: two solids drawn as wireframes, one inside the other, turning
//! against each other, on a field of flat color. Every kick blows the
//! edges apart, each flung out from the middle and springing back into
//! place; the snare spins them, reversing on impacts and every fourth
//! snare; at every hard cut the solids change (icosahedron, cube,
//! octahedron, tetrahedron), the field jumps to a new color, and the
//! picture swaps between bright lines over a dim field and a bright field
//! cut by dark lines, like a screen print. The echoes of the lines fly
//! outward behind them. Geometry for riffs: hard edges, hard cuts.
//!
//! | Signal   | Moves                                                     |
//! |----------|-----------------------------------------------------------|
//! | kick, hit| the edges blown apart, the echoes thrown outward (springs)|
//! | snare    | the spin                                                  |
//! | impact   | new solids, a new field, a flash, a big blast             |
//! | chaos    | torn colors, thicker lines, sliced glitches               |
//! | loudness | how big the solids are                                    |
//! | harmonic | how fast the field's color drifts                         |

use juglar_signals::{Signals, WAVEFORM_POINTS};

use super::react::{Pulse, Reactor};
use super::{Character, Fx, Look, Palette, Scene};

/// The whole rainbow, flat and loud.
const PALETTE: Palette = Palette {
    a: [0.5, 0.5, 0.5],
    b: [0.5, 0.5, 0.5],
    c: [1.0, 1.0, 1.0],
    d: [0.0, 0.33, 0.67],
};

/// Edges at most: four numbers each in the waveform's slots.
const EDGES: usize = WAVEFORM_POINTS / 4;

type Point = [f32; 3];

/// The five solids' vertices, on the unit sphere.
fn solid(which: usize) -> Vec<Point> {
    let phi = (1.0 + 5f32.sqrt()) / 2.0;
    let raw: Vec<Point> = match which {
        0 => {
            // Icosahedron.
            let mut v = Vec::new();
            for a in [-1.0, 1.0] {
                for b in [-phi, phi] {
                    v.extend([[0.0, a, b], [a, b, 0.0], [b, 0.0, a]]);
                }
            }
            v
        }
        1 => (0..8).map(|i| [if i & 1 == 0 { -1.0 } else { 1.0 }, if i & 2 == 0 { -1.0 } else { 1.0 }, if i & 4 == 0 { -1.0 } else { 1.0 }]).collect(),
        2 => vec![[1.0, 0.0, 0.0], [-1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, -1.0, 0.0], [0.0, 0.0, 1.0], [0.0, 0.0, -1.0]],
        _ => vec![[1.0, 1.0, 1.0], [1.0, -1.0, -1.0], [-1.0, 1.0, -1.0], [-1.0, -1.0, 1.0]],
    };
    raw.into_iter()
        .map(|p| {
            let n = (p[0] * p[0] + p[1] * p[1] + p[2] * p[2]).sqrt();
            [p[0] / n, p[1] / n, p[2] / n]
        })
        .collect()
}

/// The edges of a solid: every pair of vertices at the shortest distance.
fn edges(vertices: &[Point]) -> Vec<(usize, usize)> {
    let distance = |a: Point, b: Point| ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt();
    let mut shortest = f32::MAX;
    for i in 0..vertices.len() {
        for j in i + 1..vertices.len() {
            shortest = shortest.min(distance(vertices[i], vertices[j]));
        }
    }
    let mut out = Vec::new();
    for i in 0..vertices.len() {
        for j in i + 1..vertices.len() {
            if distance(vertices[i], vertices[j]) < shortest * 1.01 {
                out.push((i, j));
            }
        }
    }
    out
}

fn turn(p: Point, [x, y, z]: [f32; 3]) -> Point {
    let (s, c) = x.sin_cos();
    let p = [p[0], c * p[1] - s * p[2], s * p[1] + c * p[2]];
    let (s, c) = y.sin_cos();
    let p = [c * p[0] + s * p[2], p[1], -s * p[0] + c * p[2]];
    let (s, c) = z.sin_cos();
    [c * p[0] - s * p[1], s * p[0] + c * p[1], p[2]]
}

/// Outer and inner solid at each cut.
const PAIRS: [(usize, usize); 4] = [(0, 1), (1, 2), (2, 3), (3, 0)];

pub(crate) struct Wire {
    reactor: Reactor,
    solids: Vec<(Vec<Point>, Vec<(usize, usize)>)>,
    angles: [f32; 3],
    hue: f32,
    clock: f32,
}

impl Wire {
    pub fn new() -> Self {
        let solids = (0..4).map(|i| {
            let v = solid(i);
            let e = edges(&v);
            (v, e)
        });
        Self { reactor: Reactor::new(0x3e3e), solids: solids.collect(), angles: [0.4, 0.0, 0.0], hue: 0.0, clock: 0.0 }
    }
}

impl Scene for Wire {
    fn name(&self) -> &'static str {
        "wire"
    }

    /// Hard edges, hard cuts.
    fn character(&self) -> Character {
        Character { energy: 0.85, pulse: 0.95, flow: true }
    }

    fn shader(&self) -> &'static str {
        include_str!("wire.wgsl")
    }

    fn update(&mut self, s: &Signals, motion: f32) -> Look {
        let dt = s.dt.clamp(0.0, 0.1);
        let moving = dt * motion;
        let Pulse { punch, snap, slam, shake, flash, turn: spin, cuts, chaos, harmony, .. } = self.reactor.update(s, motion);
        let live = motion * (0.5 + s.intensity.clamp(0.0, 1.0));
        self.clock += moving;
        self.hue += moving * (0.02 + 0.15 * harmony);
        let speed = live * (0.35 + 3.0 * snap.abs() + 0.8 * chaos);
        self.angles[0] += moving * 0.23 * live;
        self.angles[1] += moving * speed * spin;
        self.angles[2] += moving * 0.11 * live * spin;

        // Project both solids. Each edge is flung out along its middle's
        // direction by the kick, so the solid bursts and springs back.
        let (outer, inner) = PAIRS[cuts as usize % PAIRS.len()];
        let size = 0.28 + 0.08 * s.loudness + 0.05 * punch + 0.15 * slam.max(0.0);
        let burst = 0.6 * punch.max(-0.3) + 1.2 * slam.max(0.0);
        let mut slots = [0.0; WAVEFORM_POINTS];
        let mut count = 0;
        for (which, scale, sign) in [(outer, size, 1.0), (inner, 0.45 * size, -1.0)] {
            let (vertices, edges) = &self.solids[which];
            let angles = self.angles.map(|a| a * sign);
            let placed: Vec<Point> = vertices.iter().map(|&v| turn(v, angles)).collect();
            for &(i, j) in edges {
                if count == EDGES {
                    break;
                }
                let (a, b) = (placed[i], placed[j]);
                let middle = [(a[0] + b[0]) * 0.5, (a[1] + b[1]) * 0.5, (a[2] + b[2]) * 0.5];
                let push = |p: Point| [p[0] + middle[0] * burst, p[1] + middle[1] * burst, p[2] + middle[2] * burst];
                let project = |p: Point| {
                    let depth = 3.0 / (3.0 + p[2] * 1.2);
                    [p[0] * scale * depth, p[1] * scale * depth]
                };
                let (a, b) = (project(push(a)), project(push(b)));
                slots[4 * count..4 * count + 4].copy_from_slice(&[a[0] * motion, a[1] * motion, b[0] * motion, b[1] * motion]);
                count += 1;
            }
        }

        // Every other cut: bright lines on a dim field, or a bright field
        // cut by dark lines.
        let print = cuts % 2 == 1;
        Look {
            center: [0.0, 0.0],
            zoom: (live * (0.3 + 3.0 * punch.max(0.0) + 5.0 * slam.max(0.0)) * dt).exp(),
            rotation: live * spin * 0.3 * dt,
            swirl: 0.0,
            decay: (-moving / (0.3 + 0.2 * harmony)).exp(),
            palette_drift: 0.5 * moving,
            knobs: [
                count as f32,                       // edges
                0.0025 + 0.003 * chaos,             // line half-width
                1.1 + 1.2 * punch.max(0.0),         // ink
                0.0,
                0.0,
                0.0,
                0.0,
                0.0,
            ],
            palette: PALETTE,
            palette_shift: self.hue + (cuts % 7) as f32 * 0.29,
            palette_wrap: true,
            exposure: 1.3,
            bloom: if print { 0.15 } else { 0.5 },
            waveform: slots,
            spectrum: s.spectrum,
            fx: Fx {
                shake: [0.025 * shake[0], 0.025 * shake[1]],
                chroma: 0.002 + 0.008 * chaos + 0.006 * punch.max(0.0).min(1.0),
                invert: 0.0,
                flash: 0.5 * flash,
                glitch: if chaos > 0.6 { 0.3 * snap.abs().min(1.0) } else { 0.0 },
                glitch_seed: cuts as f32 * 13.0 + (self.clock * 10.0).floor(),
                relief: 0.0,
                ground: if print { 0.85 } else { 0.18 },
                ground_hue: 0.5,
                stencil: if print { 1.0 } else { 0.0 },
            },
        }
    }
}
