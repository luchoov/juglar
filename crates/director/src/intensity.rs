//! Natural intensity: how lively the visuals are, read from the music.
//!
//! Intensity scales how fast and how far every scene moves (never how
//! bright it is). With keys 1 to 3 the listener picks it; in natural mode
//! it follows the music: an ambient intro is gentle, a chorus with a steady
//! kick is strong, and it moves between them slowly enough that the change
//! itself goes unnoticed.

use crate::feel::Feel;

/// The quietest and liveliest natural intensities: never fully still,
/// never past the strong preset.
const LOWEST: f32 = 0.15;
const HIGHEST: f32 = 0.95;
/// Seconds to rise and to fall. Rising is faster, so a drop is met within
/// a couple of seconds; falling is slower, so a short pause in a loud song
/// doesn't calm everything down.
const RISE_SECONDS: f32 = 2.0;
const FALL_SECONDS: f32 = 6.0;

/// How lively the music is, `0..=1`: half how fast it pulses, the rest how
/// bright it is and whether this part is louder than the last half minute.
///
/// Loudness itself would say little: the analysis levels every song to its
/// own peaks, so with real music it sits near 0.9 whatever plays. Measured
/// over two and a half minutes of a playlist, it stayed between 0.87 and
/// 0.98 while the pulse ran from 0.01 (a passage with no kick) to 1, and
/// the brightness from 0.01 (a dark, bass-heavy song) to 0.8.
pub fn arousal(feel: &Feel) -> f32 {
    (0.5 * feel.pulse + 0.3 * feel.brightness + 0.2 * feel.swell).clamp(0.0, 1.0)
}

#[derive(Clone, Debug)]
pub struct NaturalIntensity {
    value: f32,
}

impl NaturalIntensity {
    /// Starts at `value`, the intensity in use before natural mode.
    pub fn new(value: f32) -> Self {
        Self { value }
    }

    pub fn value(&self) -> f32 {
        self.value
    }

    pub fn update(&mut self, feel: &Feel, dt: f32) -> f32 {
        let target = if feel.sounding { LOWEST + (HIGHEST - LOWEST) * arousal(feel) } else { LOWEST };
        let seconds = if target > self.value { RISE_SECONDS } else { FALL_SECONDS };
        self.value += (target - self.value) * (1.0 - (-dt.clamp(0.0, 0.25) / seconds).exp());
        self.value
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::feel::Listener;
    use juglar_signals::{Signals, Synthetic};

    #[test]
    fn follows_the_song() {
        let mut song = Synthetic::new(5);
        let mut listener = Listener::new();
        let mut natural = NaturalIntensity::new(0.5);
        let mut s = Signals::default();
        let mut at = |until: f32, t: &mut f32| {
            while *t < until {
                song.next(1.0 / 60.0, &mut s);
                let feel = listener.update(&s);
                natural.update(&feel, 1.0 / 60.0);
                *t += 1.0 / 60.0;
            }
            natural.value()
        };
        let mut t = 0.0;
        let intro = at(15.0, &mut t);
        let build = at(31.0, &mut t);
        let drop = at(36.0, &mut t);
        let late_drop = at(47.0, &mut t);
        let breakdown = at(63.0, &mut t);
        assert!(intro < 0.35, "intro {intro}");
        assert!(build > intro + 0.1, "build {build}, intro {intro}");
        // Livelier within a few seconds of the drop, and liveliest in it.
        assert!(drop > build + 0.05, "drop {drop}, build {build}");
        assert!(late_drop > 0.55, "late drop {late_drop}");
        assert!(breakdown < intro, "breakdown {breakdown}, intro {intro}");
    }

    #[test]
    fn changes_slowly() {
        // From silence to the loudest music, a frame moves it by little.
        let mut natural = NaturalIntensity::new(LOWEST);
        let loud = Feel { pulse: 1.0, loudness: 1.0, brightness: 1.0, swell: 1.0, rise: 0.0, sounding: true };
        let before = natural.value();
        natural.update(&loud, 1.0 / 60.0);
        assert!(natural.value() - before < 0.01);
    }
}
