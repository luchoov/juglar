//! Tide, the first hand-made scene: a ring drawn by the waveform, sent
//! outward by the zoom so it leaves a tunnel of its own past shapes.
//!
//! How the music moves it:
//!
//! | Signal | Moves                                                         |
//! |--------|---------------------------------------------------------------|
//! | bass   | how fast the tunnel flows outward, the ring's size, and how   |
//! |        | much of the line the trails keep (so kicks leave echoes)      |
//! | mid    | how fast the picture turns                                    |
//! | treble | the swirl that bends trails into spirals; line thickness      |
//! | energy | the wave's height                                             |
//! | mood   | energy smoothed over seconds: how long trails last and how    |
//! |        | warm the colors get                                           |
//!
//! Every number below is a matter of taste, tuned by eye on real music and
//! on the synthetic song. Changing the scene means changing them, or the
//! shapes in `tide.wgsl`.
//!
//! Every kick moves things (the flow speeds up, the ring grows) rather than
//! brightening them. The flash limiter that checks the final picture comes
//! in Milestone 5.

use std::f32::consts::TAU;

use juglar_signals::Signals;

use super::{Look, Palette};

/// Sand to silver to steel blue: one half-turn of a cosine per channel, so
/// 0 is #dcc69a and 1 is #6f9bd1, with no green on the way (dim yellows
/// and greens look muddy). Warm for the present, cool for the past. Its
/// reddest color is under half red, far from the saturated reds that must
/// never flash.
const PALETTE: Palette = Palette {
    a: [0.649, 0.692, 0.712],
    b: [0.214, 0.084, 0.108],
    c: [0.5, 0.5, 0.5],
    d: [0.0, 0.0, 0.5],
};

/// Seconds over which `mood` follows the energy.
const MOOD_SECONDS: f32 = 3.0;

pub(crate) struct Tide {
    /// Time that passes only while motion is allowed; the slow wandering and
    /// reversals are functions of it, so they freeze with reduced motion.
    clock: f32,
    /// Energy smoothed over seconds: a section's character, not a beat's.
    mood: f32,
}

impl Tide {
    pub fn new() -> Self {
        Self { clock: 0.0, mood: 0.0 }
    }

    /// The sources this scene paints, in WGSL.
    pub const SHADER: &str = include_str!("tide.wgsl");

    /// This frame's look. `motion` is 1 normally and 0 with reduced motion,
    /// which leaves a still picture.
    pub fn update(&mut self, s: &Signals, motion: f32) -> Look {
        // A long pause (the window was dragged) shouldn't jump the picture.
        let dt = s.dt.clamp(0.0, 0.1);
        let follow = 1.0 - (-dt / MOOD_SECONDS).exp();
        self.mood += (s.energy - self.mood) * follow;
        self.clock += dt * motion;

        // Intensity scales every movement: 0 is calm, 1 is strong.
        let live = motion * (0.4 + 1.2 * s.intensity.clamp(0.0, 1.0));
        // Slow reversals, so the turning never feels mechanical.
        let spin_direction = (self.clock * TAU / 80.0).sin();
        let swirl_direction = (self.clock * TAU / 53.0).cos();

        // The tunnel flows outward fast enough for trails to reach the
        // edges before they fade, faster with the bass.
        let zoom_per_second = live * (0.5 + 1.4 * s.bass);
        let turn_per_second = live * spin_direction * (0.05 + 0.25 * s.mid);
        let swirl_per_second = live * swirl_direction * (0.1 + 0.6 * s.treble);
        // Trails last longer when the music is full.
        let persistence = 0.22 + 0.22 * self.mood;

        Look {
            center: [
                0.03 * motion * (self.clock * 0.11).sin(),
                0.03 * motion * (self.clock * 0.07 + 1.0).sin(),
            ],
            zoom: (zoom_per_second * dt).exp(),
            rotation: turn_per_second * dt,
            swirl: swirl_per_second * dt,
            decay: (-dt / persistence).exp(),
            // Trails cool toward blue as they age.
            palette_drift: 0.3 * dt * motion,
            knobs: [
                0.16 + 0.04 * s.bass,      // ring radius
                0.05 + 0.07 * s.energy,    // wave height on the ring
                0.002 + 0.001 * s.treble,  // line half-width
                0.05 + 0.06 * s.bass,      // how much of the line the trails keep
                0.0,                       // line palette position
                1.8,                       // line ink on top
                0.25,                      // how much cooler the trails start
                0.0,                       // unused
            ],
            palette: PALETTE,
            // Quiet music draws in blue, full music in sand; a slow sway
            // keeps the color alive in between.
            palette_shift: 0.85 - 0.8 * self.mood + 0.06 * (self.clock * TAU / 120.0).sin(),
            exposure: 1.3,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_palette_never_turns_saturated_red() {
        // Research policy: animated colors keep red under 75% of the light.
        for i in 0..=1000 {
            let c = PALETTE.at(i as f32 / 1000.0);
            let linear = c.map(|c| if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) });
            let red_share = linear[0] / linear.iter().sum::<f32>();
            assert!(red_share <= 0.75, "{red_share} at {i}");
        }
    }

    #[test]
    fn reduced_motion_stands_still() {
        let mut tide = Tide::new();
        let loud = Signals { dt: 1.0 / 60.0, bass: 1.0, mid: 1.0, treble: 1.0, energy: 1.0, ..Signals::default() };
        let look = tide.update(&loud, 0.0);
        assert_eq!(look.zoom, 1.0);
        assert_eq!(look.rotation, 0.0);
        assert_eq!(look.swirl, 0.0);
        assert_eq!(look.center, [0.0, 0.0]);
    }

    #[test]
    fn motion_per_second_ignores_the_frame_rate() {
        // Two frames at 60 fps zoom as much as one at 30 fps.
        let at = |dt| {
            let s = Signals { dt, bass: 0.5, ..Signals::default() };
            Tide::new().update(&s, 1.0).zoom
        };
        let two_short = at(1.0 / 60.0).powi(2);
        let one_long = at(1.0 / 30.0);
        assert!((two_short - one_long).abs() < 1e-5, "{two_short} vs {one_long}");
    }
}
