//! An oscilloscope view of the newest audio: a short stretch of waveform that
//! holds still on screen instead of scrolling past.
//!
//! Drawing "the last 21 ms" every frame would show a wave that jumps sideways,
//! because each frame cuts it at a different point of its cycle. A hardware
//! oscilloscope solves this with a *trigger*: it always starts drawing where
//! the signal crosses zero going up. Repeating sounds then land in the same
//! place frame after frame and look still.

use crate::smoothing::Envelope;

/// Points drawn per frame.
pub const SCOPE_POINTS: usize = 256;
/// Samples averaged into each point. 256 × 4 = 1024 samples: 21 ms at 48 kHz,
/// one full cycle of anything above 47 Hz.
const DECIMATION: usize = 4;
/// Samples covered by one view.
pub const SCOPE_SPAN: usize = SCOPE_POINTS * DECIMATION;
/// The trigger watches the signal through a low-pass filter at this
/// frequency, so it locks onto the bass and kick, which repeat slowly, instead
/// of hi-hats crossing zero at random.
const TRIGGER_LOWPASS_HZ: f32 = 150.0;
/// Peaks quieter than this (−40 dBFS) are not stretched to full height, so a
/// quiet passage looks quiet and silence looks flat.
const MIN_PEAK: f32 = 0.01;

pub struct Scope {
    /// One-pole low-pass coefficient for `TRIGGER_LOWPASS_HZ`.
    lowpass_k: f32,
    filtered: Vec<f32>,
    /// Recent peak, for scaling the wave to the screen.
    peak: Envelope,
    points: [f32; SCOPE_POINTS],
}

impl Scope {
    pub fn new(sample_rate: u32) -> Self {
        let dt = 1.0 / sample_rate as f32;
        let tau = 1.0 / (std::f32::consts::TAU * TRIGGER_LOWPASS_HZ);
        Self {
            lowpass_k: 1.0 - (-dt / tau).exp(),
            filtered: Vec::new(),
            peak: Envelope::new(0.005, 0.5),
            points: [0.0; SCOPE_POINTS],
        }
    }

    /// Picks a stable view of `recent` (the newest samples, oldest first,
    /// longer than `SCOPE_SPAN`) and returns it scaled to `-1..=1`. `dt` is
    /// the time since the previous call, in seconds.
    pub fn update(&mut self, recent: &[f32], dt: f32) -> &[f32; SCOPE_POINTS] {
        assert!(recent.len() > SCOPE_SPAN, "scope needs more than {SCOPE_SPAN} samples");

        // A low-pass filter delays what it lets through. Running it forward
        // and then backward makes the two delays cancel, so a crossing in the
        // filtered signal sits where the real wave crosses.
        self.filtered.clear();
        let mut y = 0.0;
        for &x in recent {
            y += (x - y) * self.lowpass_k;
            self.filtered.push(y);
        }
        let mut y = 0.0;
        for slot in self.filtered.iter_mut().rev() {
            y += (*slot - y) * self.lowpass_k;
            *slot = y;
        }

        // The newest upward zero crossing that still leaves a full view after
        // it. Without one (silence, or a bass below 47 Hz), show the newest.
        let last_start = recent.len() - SCOPE_SPAN;
        let start = (1..=last_start)
            .rev()
            .find(|&i| self.filtered[i - 1] < 0.0 && self.filtered[i] >= 0.0)
            .unwrap_or(last_start);
        let view = &recent[start..start + SCOPE_SPAN];

        let loudest = view.iter().fold(0.0f32, |m, x| m.max(x.abs()));
        let scale = 1.0 / self.peak.update(loudest, dt).max(MIN_PEAK);
        for (point, chunk) in self.points.iter_mut().zip(view.as_chunks::<DECIMATION>().0) {
            let mean = chunk.iter().sum::<f32>() / DECIMATION as f32;
            *point = (mean * scale).clamp(-1.0, 1.0);
        }
        &self.points
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::TAU;

    const RATE: u32 = 48_000;

    fn sine(hz: f32, amplitude: f32, phase: f32, len: usize) -> Vec<f32> {
        (0..len).map(|t| amplitude * (TAU * hz * t as f32 / RATE as f32 + phase).sin()).collect()
    }

    #[test]
    fn view_starts_on_an_upward_crossing() {
        let mut scope = Scope::new(RATE);
        let points = scope.update(&sine(100.0, 0.5, 1.0, 2048), 0.016);
        // The trigger runs on the filtered signal, which lags the raw one a
        // little, so the first point sits near zero rather than exactly on it.
        assert!(points[0].abs() < 0.15, "first point {}", points[0]);
        assert!(points[4] > points[0], "wave should be rising at the start");
    }

    #[test]
    fn view_holds_still_whatever_the_phase() {
        let mut a = Scope::new(RATE);
        let mut b = Scope::new(RATE);
        let first = *a.update(&sine(100.0, 0.5, 0.3, 2048), 0.016);
        let second = *b.update(&sine(100.0, 0.5, 2.9, 2048), 0.016);
        let worst = first.iter().zip(&second).map(|(x, y)| (x - y).abs()).fold(0.0, f32::max);
        assert!(worst < 0.1, "views differ by up to {worst}");
    }

    #[test]
    fn quiet_wave_is_scaled_up_and_silence_stays_flat() {
        let mut scope = Scope::new(RATE);
        let mut points = [0.0; SCOPE_POINTS];
        for _ in 0..60 {
            points = *scope.update(&sine(100.0, 0.1, 0.0, 2048), 0.016);
        }
        let top = points.iter().fold(0.0f32, |m, x| m.max(x.abs()));
        assert!(top > 0.9, "quiet wave only reaches {top}");

        let mut silent = Scope::new(RATE);
        let points = silent.update(&[0.0; 2048], 0.016);
        assert!(points.iter().all(|&x| x == 0.0));
    }
}
