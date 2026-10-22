//! A synthetic song: signals that behave like music, with no audio at all.
//!
//! It exists to develop and test visuals in silence and to drive demos that
//! can't capture sound (a web page, a screensaver preview). It is not a
//! synthesizer: nothing is ever heard. It plays a 32-bar loop at 120 beats
//! per minute in four sections of eight bars, the shape of most dance music:
//!
//! | Bars  | Section   | What moves                                       |
//! |-------|-----------|--------------------------------------------------|
//! | 1–8   | intro     | a soft chord swelling once per bar               |
//! | 9–16  | build     | hi-hats getting louder, kick on every other beat |
//! | 17–24 | drop      | kick on every beat, heavy bass, everything loud  |
//! | 25–32 | breakdown | chords only, quiet                               |

use std::f32::consts::{PI, TAU};

use crate::{SPECTRUM_BANDS, Signals, WAVEFORM_POINTS};

const BEATS_PER_MINUTE: f32 = 120.0;
const BEATS_PER_BAR: f32 = 4.0;
const BARS_PER_SECTION: f32 = 8.0;
const SECTIONS: f32 = 4.0;

/// How fast each sound fades after it starts, in seconds.
const KICK_DECAY: f32 = 0.12;
const HAT_DECAY: f32 = 0.035;
const BEAT_DECAY: f32 = 0.15;
const NOVELTY_DECAY: f32 = 0.8;

/// Spectrum bands below this one count as bass, and from it up to
/// `FIRST_TREBLE_BAND` as mid (the same split as the audio analysis).
const FIRST_MID_BAND: usize = 11;
const FIRST_TREBLE_BAND: usize = 25;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Section {
    Intro,
    Build,
    Drop,
    Breakdown,
}

/// Produces `Signals` from the clock alone. The same time always gives the
/// same signals, apart from a little seeded flicker in the spectrum.
pub struct Synthetic {
    time: f32,
    noise: Noise,
    /// Per-band flicker, smoothed so it wanders instead of jumping.
    flicker: [f32; SPECTRUM_BANDS],
}

impl Synthetic {
    pub fn new(seed: u64) -> Self {
        Self { time: 0.0, noise: Noise(seed | 1), flicker: [1.0; SPECTRUM_BANDS] }
    }

    /// Starts the song `seconds` in, e.g. at the drop.
    pub fn starting_at(seed: u64, seconds: f32) -> Self {
        Self { time: seconds, ..Self::new(seed) }
    }

    /// Advances the song by `dt` seconds and writes its signals into `out`.
    /// `out.time` keeps counting from wherever the caller started, and
    /// `out.intensity` is left alone: it belongs to the user.
    pub fn next(&mut self, dt: f32, out: &mut Signals) {
        self.time += dt;
        let beat_length = 60.0 / BEATS_PER_MINUTE;
        let beats = self.time / beat_length;
        let section_length = beat_length * BEATS_PER_BAR * BARS_PER_SECTION;
        let in_song = self.time % (section_length * SECTIONS);
        let section = match (in_song / section_length) as u32 {
            0 => Section::Intro,
            1 => Section::Build,
            2 => Section::Drop,
            _ => Section::Breakdown,
        };
        // 0 at the start of the section, 1 at its end.
        let progress = (in_song % section_length) / section_length;
        let since_beat = beats.fract() * beat_length;
        let since_eighth = (beats * 2.0).fract() * beat_length * 0.5;
        let since_section = progress * section_length;
        let bar_phase = (beats / BEATS_PER_BAR).fract();

        let kick_on_this_beat =
            section == Section::Drop || (section == Section::Build && (beats as u32).is_multiple_of(2));
        let kick = if kick_on_this_beat { (-since_beat / KICK_DECAY).exp() } else { 0.0 };
        let hat = (-since_eighth / HAT_DECAY).exp();
        // A chord that swells and fades once per bar.
        let swell = 0.5 - 0.5 * (bar_phase * TAU).cos();

        let (bass, mid, treble) = match section {
            Section::Intro => (0.08, 0.25 + 0.3 * swell, 0.05),
            Section::Build => (0.1 + 0.6 * kick, 0.3 + 0.2 * swell, (0.1 + 0.5 * progress) * (0.4 + 0.6 * hat)),
            Section::Drop => (0.35 + 0.65 * kick, 0.55 + 0.2 * swell, 0.35 + 0.4 * hat),
            Section::Breakdown => (0.05, 0.2 + 0.35 * swell, 0.04),
        };
        out.time += dt;
        out.dt = dt;
        out.bass = bass;
        out.mid = mid;
        out.treble = treble;
        out.energy = (0.5 * bass + 0.35 * mid + 0.15 * treble).min(1.0);
        out.beat = if kick_on_this_beat { (-since_beat / BEAT_DECAY).exp() } else { 0.0 };
        out.novelty = (-since_section / NOVELTY_DECAY).exp();

        self.spectrum(dt, bass, mid, treble, &mut out.spectrum);
        waveform(beats, bass, mid, treble, &mut out.waveform);
    }

    /// A plausible spectrum: one hump per band region, scaled by its level,
    /// with a slow random flicker so it doesn't look drawn with a ruler.
    fn spectrum(&mut self, dt: f32, bass: f32, mid: f32, treble: f32, out: &mut [f32; SPECTRUM_BANDS]) {
        let follow = 1.0 - (-dt / 0.08).exp();
        for (i, (value, flicker)) in out.iter_mut().zip(&mut self.flicker).enumerate() {
            *flicker += (0.7 + 0.6 * self.noise.next() - *flicker) * follow;
            let (level, start, end) = if i < FIRST_MID_BAND {
                (bass, 0, FIRST_MID_BAND)
            } else if i < FIRST_TREBLE_BAND {
                (mid, FIRST_MID_BAND, FIRST_TREBLE_BAND)
            } else {
                (treble, FIRST_TREBLE_BAND, SPECTRUM_BANDS)
            };
            // 0 at the region's edges, 1 in its middle.
            let x = (i - start) as f32 / (end - start - 1) as f32;
            let hump = 0.35 + 0.65 * (x * PI).sin();
            *value = (level * hump * *flicker).clamp(0.0, 1.0);
        }
    }
}

/// A stable wave made of a low note and its overtones, as a triggered
/// oscilloscope would show it: the bass sets the slow swing, the mid adds
/// harmonics that change its shape, the treble a fine ripple that drifts.
fn waveform(beats: f32, bass: f32, mid: f32, treble: f32, out: &mut [f32; WAVEFORM_POINTS]) {
    let loudest = bass.max(mid).max(treble);
    if loudest <= 0.0 {
        out.fill(0.0);
        return;
    }
    let drift = beats * 0.37 * TAU;
    let mut peak: f32 = 0.0;
    for (i, value) in out.iter_mut().enumerate() {
        let x = i as f32 / (WAVEFORM_POINTS - 1) as f32;
        *value = bass * (1.25 * TAU * x).sin()
            + 0.5 * mid * (3.75 * TAU * x).sin()
            + 0.3 * mid * (6.25 * TAU * x + 0.6).sin()
            + 0.15 * treble * (23.0 * TAU * x + drift).sin();
        peak = peak.max(value.abs());
    }
    // Like the real scope, scale the wave to fill the view, a little more
    // when the music is loud.
    let scale = (0.45 + 0.5 * loudest) / peak.max(1e-6);
    out.iter_mut().for_each(|v| *v *= scale);
}

/// A tiny seeded random number generator (xorshift64). Not for anything
/// that needs real randomness; it only adds flicker.
struct Noise(u64);

impl Noise {
    /// A number in `0..1`.
    fn next(&mut self) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 40) as f32 / (1u64 << 24) as f32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Runs the song at 60 fps until `seconds`, returning the last signals.
    fn at(seconds: f32) -> Signals {
        let mut song = Synthetic::new(7);
        let mut out = Signals::default();
        let frames = (seconds * 60.0).round() as u32;
        for _ in 0..frames {
            song.next(1.0 / 60.0, &mut out);
        }
        out
    }

    #[test]
    fn every_value_stays_in_range() {
        let mut song = Synthetic::new(1);
        let mut out = Signals::default();
        for _ in 0..(70 * 60) {
            song.next(1.0 / 60.0, &mut out);
            for v in [out.bass, out.mid, out.treble, out.energy, out.beat, out.novelty] {
                assert!((0.0..=1.0).contains(&v), "{v}");
            }
            assert!(out.spectrum.iter().all(|v| (0.0..=1.0).contains(v)));
            assert!(out.waveform.iter().all(|v| (-1.0..=1.0).contains(v)));
        }
    }

    #[test]
    fn the_drop_is_louder_than_the_intro() {
        // Sections last 8 bars of 2 s; sample the middle of each.
        let intro = at(8.0);
        let drop = at(40.0);
        assert!(drop.energy > intro.energy * 2.0, "{} vs {}", drop.energy, intro.energy);
    }

    #[test]
    fn kicks_land_on_the_beat_in_the_drop() {
        // One frame after the beat at 40 s, then half a beat later.
        let on = at(40.02);
        let between = at(40.25);
        assert!(on.beat > 0.8, "{}", on.beat);
        assert!(between.beat < 0.3, "{}", between.beat);
        assert!(on.bass > between.bass);
    }

    #[test]
    fn the_intro_has_no_beat() {
        assert_eq!(at(5.0).beat, 0.0);
    }

    #[test]
    fn starting_later_skips_ahead() {
        let mut song = Synthetic::starting_at(7, 40.0);
        let mut out = Signals::default();
        song.next(1.0 / 60.0, &mut out);
        assert!(out.beat > 0.8, "{}", out.beat);
        assert!((out.time - 1.0 / 60.0).abs() < 1e-6);
    }
}
