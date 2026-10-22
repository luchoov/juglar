//! Julia: a fractal that the feedback loop computes one step per frame.
//!
//! A Julia set is drawn by taking every point z of the plane, squaring it
//! and adding a constant c, over and over, and asking how soon it runs off
//! toward infinity. Here the warp *is* that step: each pixel takes its old
//! paint from z² + c. Paint laid where points escape, and on a ring drawn
//! by the waveform, flows back through the steps, so after a few dozen
//! frames every pixel shows how many steps its point took to reach the
//! paint, and the ring's echoes nest toward the fractal's edge. Paint
//! changes color with every step, so the steps come out as rainbow bands.
//!
//! | Signal  | Moves                                                      |
//! |---------|------------------------------------------------------------|
//! | mid     | how fast c travels (the fractal changes shape)             |
//! | bass    | how far c is from the middle, the ring's size              |
//! | energy  | the height of the wave on the ring                         |
//! | beat    | a push in c's travel                                       |
//! | novelty | c travels faster, colors jump ahead                        |
//!
//! Unlike the other scenes, the fade and the color change are per frame,
//! not per second: each frame is one step, and the bands are steps. At a
//! lower frame rate the picture looks the same and settles more slowly.
//!
//! Near the fractal's edge the bands get thinner than a pixel and move
//! fast when c moves, so c travels slowly and smoothly, and the deep
//! steps, where the bands are thinnest, have faded nearly to black.

use std::f32::consts::TAU;

use juglar_signals::{Signals, WAVEFORM_POINTS};

use super::{Character, Look, Palette, Scene, smooth3};

/// Leaf green, rose, magenta, violet, blue and teal, all the way around.
/// Chosen among looping palettes for a narrow range of brightness (linear
/// luminance 0.11 to 0.41, where Kaleido's spans 0.11 to 0.79): bands
/// sliding past a point swing its brightness about half as much, and
/// faded ones fall below the flash rule's 0.1 sooner. Red stays under 64%.
const PALETTE: Palette = Palette {
    a: [0.52, 0.32, 0.74],
    b: [0.53, 0.46, 0.49],
    c: [1.0, 1.0, 1.0],
    d: [0.7, 0.1, 0.55],
};

const WAVE_SECONDS: f32 = 0.25;
const LEVEL_SECONDS: f32 = 0.2;
/// c's distance from the middle: on this circle lie many of the most
/// intricate Julia sets, where c is near the edge of the Mandelbrot set.
const RADIUS: f32 = 0.7885;

pub(crate) struct Julia {
    waveform: [[f32; WAVEFORM_POINTS]; 3],
    /// Bass, mid, energy, beat and novelty, smoothed.
    levels: [[f32; 5]; 3],
    /// c's angle.
    angle: f32,
    clock: f32,
    hue: f32,
}

impl Julia {
    pub fn new() -> Self {
        Self { waveform: [[0.0; WAVEFORM_POINTS]; 3], levels: [[0.0; 5]; 3], angle: 2.2, clock: 0.0, hue: 0.0 }
    }
}

impl Scene for Julia {
    fn name(&self) -> &'static str {
        "julia"
    }

    /// A fractal whose shape drifts with the bass.
    fn character(&self) -> Character {
        Character { energy: 0.6, pulse: 0.5, flow: false }
    }

    fn shader(&self) -> &'static str {
        include_str!("julia.wgsl")
    }

    fn update(&mut self, s: &Signals, motion: f32) -> Look {
        let dt = s.dt.clamp(0.0, 0.1);
        let moving = dt * motion;
        let waveform = smooth3(&mut self.waveform, &s.waveform, moving, WAVE_SECONDS);
        let [bass, mid, energy, beat, novelty] =
            smooth3(&mut self.levels, &[s.bass, s.mid, s.energy, s.beat, s.novelty], moving, LEVEL_SECONDS);
        let live = 0.4 + 1.2 * s.intensity.clamp(0.0, 1.0);
        self.clock += moving;
        // Radians per second of c around the middle.
        self.angle += moving * live * (0.02 + 0.05 * mid + 0.06 * beat + 0.08 * novelty);
        self.hue += moving * (0.02 + 0.2 * novelty);
        let radius = RADIUS - 0.03 + 0.04 * bass + 0.01 * (self.clock * TAU / 47.0).sin();
        let c = [radius * self.angle.cos(), radius * self.angle.sin()];

        Look {
            center: [0.0, 0.0],
            zoom: 1.0,
            rotation: 0.0,
            swirl: 0.0,
            // Per step, see above.
            decay: 0.97,
            palette_drift: 0.09,
            knobs: [
                c[0],
                c[1],
                3.1,                   // fractal units per short side
                1.0 + 0.12 * bass,     // ring radius, fractal units
                0.25 * energy,         // wave height on the ring
                0.0025,                // line half-width, short sides
                0.5,                   // ink where points escape
                1.6,                   // ring ink
            ],
            palette: PALETTE,
            palette_shift: self.hue,
            palette_wrap: true,
            exposure: 1.3,
            bloom: 0.5,
            waveform,
            spectrum: s.spectrum,
        }
    }
}
