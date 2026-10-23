//! Hammer: a tunnel of frames. Every kick throws a new frame from the
//! middle, its edges bent by the waveform, and it flies outward faster and
//! faster, growing thicker, as if it came at you down a corridor; the
//! frames leave glowing trails that bend into cushions. At every hard cut
//! the picture snaps an eighth of a turn and new frames take a new shape
//! (square, diamond, circle, cross), so the tunnel fills with the last few
//! shapes at different angles; in a wall of sound each snare flashes the
//! picture into its negative for a few frames, a strobe on the backbeat.
//! Made for groove and thrash: riffs that hit like hammers.
//!
//! The frames are drawn sharp from their own numbers (passed in the
//! spectrum's slots), not left to the blur of the feedback loop.
//!
//! | Signal   | Moves                                                     |
//! |----------|-----------------------------------------------------------|
//! | kick, hit| a new frame, as bright as the hit; the tunnel's speed     |
//! | snare    | the negative strobe while wild                            |
//! | impact   | a snap of the whole picture, a new shape, a flash         |
//! | waveform | the newest frame's edges                                  |
//! | chaos    | the strobe, the speed, torn colors, thicker lines         |
//! | harmonic | how far colors drift down the tunnel                      |

use std::f32::consts::FRAC_PI_4;

use juglar_signals::{SPECTRUM_BANDS, Signals, WAVEFORM_POINTS};

use super::react::{Pulse, Reactor};
use super::{Character, Fx, Look, Material, Palette, Scene};

/// Ice white, cyan, electric blue, violet and hot pink, around.
const PALETTE: Palette = Palette {
    a: [0.55, 0.5, 0.75],
    b: [0.45, 0.5, 0.3],
    c: [1.0, 1.0, 1.0],
    d: [0.75, 0.5, 0.0],
};

/// Wild enough for the snares to strobe, and how long each flash of
/// negative lasts.
const STROBE: f32 = 0.6;
const STROBE_SECONDS: f32 = 0.07;
/// Frames in flight at most, and where a new one starts.
const FRAMES: usize = SPECTRUM_BANDS / 4;
const BORN: f32 = 0.04;
/// Frames are forgotten once this far out.
const GONE: f32 = 1.3;
/// A new frame needs this long since the last one.
const REST: f32 = 0.07;

/// A frame in flight.
#[derive(Clone, Copy, Debug)]
struct Flying {
    size: f32,
    ink: f32,
    angle: f32,
    shape: f32,
}

pub(crate) struct Hammer {
    reactor: Reactor,
    waveform: [f32; WAVEFORM_POINTS],
    /// Newest first.
    frames: Vec<Flying>,
    since_frame: f32,
    hue: f32,
    /// The picture's angle, which snaps an eighth of a turn at each cut.
    angle: f32,
    /// Seconds of negative left from the last strobe.
    negative: f32,
}

impl Hammer {
    pub fn new() -> Self {
        Self {
            reactor: Reactor::new(0x4a33),
            waveform: [0.0; WAVEFORM_POINTS],
            frames: Vec::with_capacity(FRAMES + 1),
            since_frame: 0.0,
            hue: 0.0,
            angle: 0.0,
            negative: 0.0,
        }
    }
}

impl Scene for Hammer {
    fn name(&self) -> &'static str {
        "hammer"
    }

    /// A frame flung at every kick, strobing on the backbeat.
    fn character(&self) -> Character {
        Character { energy: 0.95, pulse: 1.0, flow: true }
    }

    fn shader(&self) -> &'static str {
        include_str!("hammer.wgsl")
    }

    fn update(&mut self, s: &Signals, motion: f32) -> Look {
        let dt = s.dt.clamp(0.0, 0.1);
        let moving = dt * motion;
        let pulse = self.reactor.update(s, motion);
        let Pulse { punch, slam, shake, flash, turn, cuts, cut, chaos, harmony, snared, kicked, .. } = pulse;
        let follow = 1.0 - (-moving / 0.03).exp();
        for (smooth, &raw) in self.waveform.iter_mut().zip(&s.waveform) {
            *smooth += (raw - *smooth) * follow;
        }
        let punch = punch.max(0.0);
        let live = motion * (0.5 + s.intensity.clamp(0.0, 1.0));
        self.hue += moving * (0.04 + 0.3 * harmony);
        if cut {
            self.angle += turn * FRAC_PI_4;
        }

        // Frames fly outward faster the further they are, as down a
        // corridor; the kicks push the whole tunnel.
        let speed = live * (1.6 + 1.5 * chaos + 3.0 * punch + 4.0 * slam.max(0.0));
        for frame in &mut self.frames {
            frame.size *= (speed * dt).exp();
            frame.ink *= (-moving / 0.9).exp();
        }
        self.frames.retain(|frame| frame.size < GONE);
        self.since_frame += moving;
        if kicked && self.since_frame >= REST {
            let ink = 0.7 + 1.3 * (s.kick.max(s.hit)).min(1.0);
            self.frames.insert(0, Flying { size: BORN, ink, angle: self.angle, shape: (cuts % 4) as f32 });
            self.frames.truncate(FRAMES);
            self.since_frame = 0.0;
        }
        let mut slots = [0.0; SPECTRUM_BANDS];
        for (k, frame) in self.frames.iter().enumerate() {
            slots[4 * k..4 * k + 4].copy_from_slice(&[frame.size, frame.ink, frame.angle, frame.shape]);
        }

        // A strobe: two or three frames of negative at each snare.
        self.negative -= moving;
        if snared && chaos > STROBE {
            self.negative = STROBE_SECONDS;
        }

        Look {
            center: [0.0, 0.0],
            zoom: 1.0,
            rotation: 0.0,
            swirl: 0.0,
            decay: (-moving / (0.35 - 0.15 * chaos)).exp(),
            palette_drift: (0.4 + 0.8 * harmony) * moving,
            knobs: [
                live * (0.6 + 2.0 * punch) * dt,         // the tunnel's pull this frame
                motion * (0.01 + 0.03 * s.loudness),     // wave height on the newest frame
                0.002 + 0.003 * chaos,                   // line width at the middle
                self.frames.len() as f32,                // frames in flight
                0.0,
                0.0,
                0.0,
                0.0,
            ],
            palette: PALETTE,
            palette_shift: self.hue + (cuts % 6) as f32 * 0.17,
            palette_wrap: true,
            exposure: 1.2 + 0.4 * s.loudness,
            bloom: 0.5 + 0.3 * punch.min(1.0),
            waveform: self.waveform,
            spectrum: slots,
            material: Material::NONE,
            fx: Fx {
                shake: [0.03 * shake[0], 0.03 * shake[1]],
                chroma: 0.002 + 0.012 * chaos * punch.min(1.0),
                invert: if self.negative > 0.0 { 1.0 } else { 0.0 },
                flash: 0.5 * flash,
                glitch: 0.0,
                glitch_seed: 0.0,
                relief: 0.0,
                ground: 0.0,
                ground_hue: 0.0,
                stencil: 0.0,
            },
        }
    }
}
