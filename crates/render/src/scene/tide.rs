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
//! | beat   | a surge in the outward flow                                   |
//! | novelty| after a change of section, longer trails and cooler colors    |
//!
//! Every number below is a matter of taste, tuned by eye on real music and
//! on the synthetic song. Changing the scene means changing them, or the
//! shapes in `tide.wgsl`.
//!
//! Every kick moves things (the flow speeds up) rather than brightening
//! them. The ring's sharp line follows the music slowly: a bright line one
//! or two pixels wide that trembles, even by a fraction of a pixel, makes the
//! pixels along it blink, and the flash limiter (`limiter.rs`) counts that as
//! flashing and holds those spots back. So the waveform the line draws is
//! smoothed in three stages, which all but removes the frame-to-frame tremble
//! (one stage only shrinks it), and its size, height and thickness follow
//! smoothed levels.

use std::f32::consts::TAU;

use juglar_signals::{Signals, WAVEFORM_POINTS};

use super::{Fx, Character, Look, Palette, Scene, smooth3};

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
/// Seconds each of the waveform's three smoothing stages takes to follow.
/// One stage would leave a random step every frame, only smaller.
const WAVE_SECONDS: f32 = 0.25;
/// Seconds each of the three smoothing stages of the ring's size, height
/// and thickness takes. With one stage of 0.3 s, kicks twice a second still
/// swung the line by a pixel or two, and the limiter held a few spots.
const RING_SECONDS: f32 = 0.2;
/// Seconds each smoothing stage of beat and novelty takes.
const EVENT_SECONDS: f32 = 0.12;

pub(crate) struct Tide {
    /// Time that passes only while motion is allowed; the slow wandering and
    /// reversals are functions of it, so they freeze with reduced motion.
    clock: f32,
    /// Energy smoothed over seconds: a section's character, not a beat's.
    mood: f32,
    /// The waveform, smoothed over time in three stages.
    waveform: [[f32; WAVEFORM_POINTS]; 3],
    /// Bass, energy and treble as the ring follows them, smoothed.
    ring: [[f32; 3]; 3],
    /// Beat and novelty, smoothed in three stages.
    events: [[f32; 2]; 3],
}

impl Tide {
    pub fn new() -> Self {
        Self { clock: 0.0, mood: 0.0, waveform: [[0.0; WAVEFORM_POINTS]; 3], ring: [[0.0; 3]; 3], events: [[0.0; 2]; 3] }
    }

}

impl Scene for Tide {
    fn name(&self) -> &'static str {
        "tide"
    }

    /// A ring flowing outward: medium, follows the beats.
    fn character(&self) -> Character {
        Character { energy: 0.5, pulse: 0.6, flow: true }
    }

    fn shader(&self) -> &'static str {
        include_str!("tide.wgsl")
    }

    fn update(&mut self, s: &Signals, motion: f32) -> Look {
        // A long pause (the window was dragged) shouldn't jump the picture.
        let dt = s.dt.clamp(0.0, 0.1);
        // With reduced motion even the colors hold still.
        let follow = 1.0 - (-dt * motion / MOOD_SECONDS).exp();
        self.mood += (s.energy - self.mood) * follow;
        self.clock += dt * motion;
        // With reduced motion the line keeps its shape: nothing follows.
        let waveform = smooth3(&mut self.waveform, &s.waveform, dt * motion, WAVE_SECONDS);
        let [ring_bass, ring_energy, ring_treble] =
            smooth3(&mut self.ring, &[s.bass, s.energy, s.treble], dt * motion, RING_SECONDS);
        let [beat, novelty] = smooth3(&mut self.events, &[s.beat, s.novelty], dt * motion, EVENT_SECONDS);

        // Intensity scales every movement: 0 is calm, 1 is strong.
        let live = motion * (0.4 + 1.2 * s.intensity.clamp(0.0, 1.0));
        // Slow reversals, so the turning never feels mechanical.
        let spin_direction = (self.clock * TAU / 80.0).sin();
        let swirl_direction = (self.clock * TAU / 53.0).cos();

        // The tunnel flows outward fast enough for trails to reach the
        // edges before they fade, faster with the bass, and surges on a beat.
        let zoom_per_second = live * (0.5 + 1.4 * s.bass + 1.2 * beat);
        let turn_per_second = live * spin_direction * (0.05 + 0.25 * s.mid);
        let swirl_per_second = live * swirl_direction * (0.1 + 0.6 * s.treble);
        // Trails last longer when the music is full, and for a while after
        // the music changes character.
        let persistence = 0.22 + 0.22 * self.mood + 0.15 * novelty;

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
                0.16 + 0.04 * ring_bass,       // ring radius
                0.05 + 0.07 * ring_energy,     // wave height on the ring
                0.002 + 0.001 * ring_treble,   // line half-width
                0.05 + 0.06 * s.bass,      // how much of the line the trails keep
                0.0,                       // line palette position
                1.8,                       // line ink on top
                0.25,                      // how much cooler the trails start
                0.0,                       // unused
            ],
            palette: PALETTE,
            // Quiet music draws in blue, full music in sand; a slow sway
            // keeps the color alive in between.
            // A change of section leans the colors toward the cool end for
            // a few seconds.
            palette_shift: 0.85 - 0.8 * self.mood + 0.15 * novelty + 0.06 * (self.clock * TAU / 120.0).sin(),
            palette_wrap: false,
            exposure: 1.3,
            bloom: 0.35,
            waveform,
            spectrum: s.spectrum,
            fx: Fx::NONE,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
