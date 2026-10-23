//! Vortex: a tunnel in which every ring turns its own way. Five lights
//! circle the middle and the tunnel carries their trails outward, so they
//! draw five spiral arms. The warp turns each point by an amount that
//! depends on its distance from the middle, one way at some distances and
//! the other way at others, so the arms kink and whorl; and it pushes each
//! point in or out along a ripple that travels outward, so they undulate.
//! An oscilloscope line drawn by the waveform turns the other way in the
//! middle.
//!
//! | Signal  | Moves                                                      |
//! |---------|------------------------------------------------------------|
//! | bass    | how fast the tunnel flows outward                          |
//! | mid     | how hard the rings turn against each other                 |
//! | treble  | how fast the lights circle                                 |
//! | spectrum| each light's size: five slices of it, bass to treble       |
//! | energy  | the wave's height on the line                              |
//! | beat    | a surge outward and a stronger ripple                      |
//! | novelty | the rings' pattern changes, colors jump ahead              |
//!
//! The line is thin and bright, so it follows a waveform smoothed in three
//! stages, as Tide's ring does, and turns at most a turn every few seconds.
//! Five arms, not more: a regular pattern of more than five light and dark
//! pairs is the kind of stripes the research warns about (note 09).

use std::f32::consts::TAU;

use juglar_signals::{SPECTRUM_BANDS, Signals, WAVEFORM_POINTS};

use super::{Fx, Character, Look, Palette, Scene, smooth3};

/// Green, teal, violet, orchid, pink, gold and lime, all the way around.
/// Red stays under 68%.
const PALETTE: Palette = Palette {
    a: [0.50, 0.71, 0.51],
    b: [0.52, 0.53, 0.54],
    c: [1.0, 1.0, 1.0],
    d: [0.5, 0.15, 0.7],
};

const WAVE_SECONDS: f32 = 0.25;
const LEVEL_SECONDS: f32 = 0.15;
const MOOD_SECONDS: f32 = 3.0;

pub(crate) struct Vortex {
    waveform: [[f32; WAVEFORM_POINTS]; 3],
    spectrum: [[f32; SPECTRUM_BANDS]; 3],
    /// Bass, mid, treble, energy, beat and novelty, smoothed.
    levels: [[f32; 6]; 3],
    mood: f32,
    /// The lights' angle; the line turns the other way.
    spin: f32,
    /// How far the ripple has traveled, and the rings' pattern.
    ripple: f32,
    pattern: f32,
    clock: f32,
    hue: f32,
}

impl Vortex {
    pub fn new() -> Self {
        Self {
            waveform: [[0.0; WAVEFORM_POINTS]; 3],
            spectrum: [[0.0; SPECTRUM_BANDS]; 3],
            levels: [[0.0; 6]; 3],
            mood: 0.0,
            spin: 0.0,
            ripple: 0.0,
            pattern: 0.0,
            clock: 0.0,
            hue: 0.0,
        }
    }
}

impl Scene for Vortex {
    fn name(&self) -> &'static str {
        "vortex"
    }

    /// A tunnel of rings turning against each other, pushed by every beat.
    fn character(&self) -> Character {
        Character { energy: 0.9, pulse: 0.85, flow: true }
    }

    fn shader(&self) -> &'static str {
        include_str!("vortex.wgsl")
    }

    fn update(&mut self, s: &Signals, motion: f32) -> Look {
        let dt = s.dt.clamp(0.0, 0.1);
        let moving = dt * motion;
        let waveform = smooth3(&mut self.waveform, &s.waveform, moving, WAVE_SECONDS);
        let spectrum = smooth3(&mut self.spectrum, &s.spectrum, moving, LEVEL_SECONDS);
        let [bass, mid, treble, energy, beat, novelty] =
            smooth3(&mut self.levels, &[s.bass, s.mid, s.treble, s.energy, s.beat, s.novelty], moving, LEVEL_SECONDS);
        self.mood += (s.energy - self.mood) * (1.0 - (-moving / MOOD_SECONDS).exp());
        let live = motion * (0.4 + 1.2 * s.intensity.clamp(0.0, 1.0));
        self.clock += moving;
        self.spin += moving * live * (0.6 + 0.8 * treble) * (self.clock * TAU / 70.0).sin().signum();
        self.ripple += moving * (1.5 + 2.0 * bass);
        self.pattern += moving * (0.05 + 0.4 * novelty);
        self.hue += moving * (0.03 + 0.25 * novelty);

        Look {
            center: [0.02 * motion * (self.clock * 0.13).sin(), 0.02 * motion * (self.clock * 0.09).cos()],
            zoom: (live * (0.6 + 1.2 * bass + 1.2 * beat) * dt).exp(),
            rotation: live * 0.1 * (self.clock * TAU / 60.0).sin() * dt,
            swirl: 0.0,
            decay: (-dt / (0.35 + 0.25 * self.mood)).exp(),
            palette_drift: 0.35 * moving,
            knobs: [
                live * (0.15 + 0.5 * mid) * dt,      // the rings' turn this frame, at most
                live * (0.06 + 0.25 * beat) * dt,    // the ripple's push this frame
                self.ripple,                         // how far the ripple has traveled
                self.pattern,                        // the rings' pattern
                self.spin,                           // the lights' angle
                0.04 + 0.08 * energy,                // wave height on the line
                0.09 + 0.02 * bass,                  // the lights' circle
                0.0,
            ],
            palette: PALETTE,
            palette_shift: self.hue,
            palette_wrap: true,
            exposure: 1.3,
            bloom: 0.5,
            waveform,
            spectrum,
            fx: Fx::NONE,
        }
    }
}
