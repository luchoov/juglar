//! Harmonograph: a line drawing made by two pendulums, like the Victorian
//! instrument where one pendulum swings the pen and another the paper. Each
//! direction is the sum of two swings whose frequencies are a simple ratio
//! (2:3, 3:4…) slightly out of tune, fading as they go, so the pen draws a
//! looping figure that spirals inward. The tuning drifts, so the figure
//! turns and folds as if seen in three dimensions; a change of section
//! moves to another ratio.
//!
//! | Signal  | Moves                                                      |
//! |---------|------------------------------------------------------------|
//! | bass    | how big the figure is                                      |
//! | mid     | how fast the tuning drifts (the figure turns)              |
//! | treble  | the line's thickness                                       |
//! | beat    | a push in the drift                                        |
//! | novelty | the next ratio, crossfaded over eight seconds              |
//!
//! The figure is computed here, 128 points of it, and sent to the shader
//! in place of the waveform (`Look::waveform`), which this scene doesn't
//! draw. Each pixel measures its distance to all 127 segments, so the line
//! is drawn only into the half-size trails: drawn sharply on top as well,
//! at full size, it cost 1.2 ms of GPU time per frame.

use std::f32::consts::TAU;

use juglar_signals::{Signals, WAVEFORM_POINTS};

use super::{Fx, Character, Look, Material, Palette, Scene, smooth3};

/// Green, teal, sky blue, mint and olive gold, all the way around.
const PALETTE: Palette = Palette {
    a: [0.35, 0.70, 0.45],
    b: [0.47, 0.26, 0.55],
    c: [1.0, 1.0, 1.0],
    d: [0.3, 0.45, 0.6],
};

/// Frequency ratios the figure moves through.
const RATIOS: [(f32, f32); 6] = [(2.0, 3.0), (3.0, 4.0), (1.0, 2.0), (3.0, 5.0), (4.0, 5.0), (2.0, 5.0)];
/// Points along the figure; two numbers each fill the waveform's slots.
pub(crate) const POINTS: usize = WAVEFORM_POINTS / 2;
const LEVEL_SECONDS: f32 = 0.15;
/// Seconds to crossfade from one ratio to the next. Over 3 s the strands
/// swept across the screen fast enough that the limiter held 0.1% of it.
const CROSSFADE: f32 = 8.0;
/// The shortest time between two ratios, and the longest.
const MIN_HOLD: f32 = 12.0;
const MAX_HOLD: f32 = 40.0;

pub(crate) struct Harmonograph {
    /// Bass, mid, treble, beat and novelty, smoothed.
    levels: [[f32; 5]; 3],
    /// How far the tuning has drifted.
    drift: f32,
    clock: f32,
    /// The ratio shown and the one before it, and how far the crossfade is.
    ratio: usize,
    previous: usize,
    fade: f32,
    /// Seconds since the ratio changed.
    held: f32,
}

impl Harmonograph {
    pub fn new() -> Self {
        Self { levels: [[0.0; 5]; 3], drift: 0.0, clock: 0.0, ratio: 0, previous: 0, fade: 1.0, held: 0.0 }
    }

    /// The pen's position at `t` (radians of the slower swing) for a ratio.
    fn pen(&self, (a, b): (f32, f32), t: f32, size: f32) -> [f32; 2] {
        // Fading over the figure's length, so it spirals inward.
        let fade = (-0.08 * t).exp();
        let x = (a * t + self.drift).sin() + 0.35 * (b * t * 1.002 + 1.3).sin();
        let y = (b * t).sin() + 0.35 * (a * t * 0.998 + self.drift * 0.7).cos();
        [size * 0.6 * x * fade, size * 0.5 * y * fade]
    }
}

impl Scene for Harmonograph {
    fn name(&self) -> &'static str {
        "harmonograph"
    }

    /// A slow line drawing, the picture slowly shrinking.
    fn character(&self) -> Character {
        Character { energy: 0.3, pulse: 0.3, flow: true }
    }

    fn shader(&self) -> &'static str {
        include_str!("harmonograph.wgsl")
    }

    fn update(&mut self, s: &Signals, motion: f32) -> Look {
        let dt = s.dt.clamp(0.0, 0.1);
        let moving = dt * motion;
        let [bass, mid, treble, beat, novelty] =
            smooth3(&mut self.levels, &[s.bass, s.mid, s.treble, s.beat, s.novelty], moving, LEVEL_SECONDS);
        let live = 0.4 + 1.2 * s.intensity.clamp(0.0, 1.0);
        self.clock += moving;
        self.drift += moving * live * (0.03 + 0.12 * mid + 0.2 * beat);
        self.held += moving;
        self.fade = (self.fade + moving / CROSSFADE).min(1.0);
        // A new ratio on a change of section, or when it has been a while.
        if self.fade >= 1.0 && self.held > MIN_HOLD && (novelty > 0.6 || self.held > MAX_HOLD) {
            self.previous = self.ratio;
            self.ratio = (self.ratio + 1) % RATIOS.len();
            self.fade = 0.0;
            self.held = 0.0;
        }

        let size = 0.55 + 0.1 * bass;
        // Smooth in and out of the crossfade.
        let blend = self.fade * self.fade * (3.0 - 2.0 * self.fade);
        // Four turns of the slower swing: enough loops to weave, few enough
        // that 128 points draw them smoothly and the strands stay apart.
        let length = 4.0 * TAU;
        let mut curve = [0.0; WAVEFORM_POINTS];
        for i in 0..POINTS {
            let t = length * i as f32 / (POINTS - 1) as f32;
            let old = self.pen(RATIOS[self.previous], t, size);
            let new = self.pen(RATIOS[self.ratio], t, size);
            curve[2 * i] = old[0] + (new[0] - old[0]) * blend;
            curve[2 * i + 1] = old[1] + (new[1] - old[1]) * blend;
        }

        Look {
            center: [0.0, 0.0],
            zoom: (-live * 0.06 * moving).exp(),
            rotation: live * 0.05 * (self.clock * TAU / 45.0).sin() * moving,
            swirl: 0.0,
            // Trails fade over 0.6 s. At 0.3 s each strand crossing a spot
            // lit and dimmed it fully, and with real music the limiter held
            // up to 0.1% of the screen; at 0.6 s, nothing in three runs.
            decay: (-dt / 0.6).exp(),
            palette_drift: 0.1 * moving,
            knobs: [
                0.0015 + 0.001 * treble,  // line half-width
                0.7,                      // how much of the line the trails keep
                0.0,                      // unused
                0.8,                      // palette positions along the figure
                0.0,
                0.0,
                0.0,
                0.0,
            ],
            palette: PALETTE,
            palette_shift: 0.02 * self.clock,
            palette_wrap: true,
            exposure: 1.3,
            bloom: 0.6,
            waveform: curve,
            spectrum: s.spectrum,
            // A dim field of the palette's far side, not black.
            material: Material::NONE,
            fx: Fx { ground: 0.1, ground_hue: 0.5, ..Fx::NONE },
        }
    }
}
