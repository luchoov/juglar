//! What the music feels like over the last several seconds, as opposed to
//! this instant: how fast it pulses, how loud and how bright it is, and
//! whether it is rising or falling.
//!
//! The analysis' signals react within a frame: `beat` is 1 on a kick and
//! gone a quarter of a second later, `energy` follows every note. Decisions
//! that should feel deliberate (how lively to be, when to change scene,
//! which scene suits the song) need slower summaries of the same signals.

use juglar_signals::Signals;

/// Beats closer than this are one beat (a kick and a snare on top of it).
const SHORTEST_BEAT: f32 = 0.2;
/// The window the beat rate is averaged over, in seconds.
const PULSE_SECONDS: f32 = 3.0;
/// Loudness rises in about this long and falls in this long.
const LOUDNESS_RISE: f32 = 1.5;
const LOUDNESS_FALL: f32 = 5.0;
const BRIGHTNESS_SECONDS: f32 = 3.0;
/// The two averages whose difference says whether the music is rising.
const RISE_FAST: f32 = 1.0;
const RISE_SLOW: f32 = 8.0;
/// Loudness is compared with its average over this long for `swell`.
const SWELL_SECONDS: f32 = 30.0;
/// Beats per second that count as the busiest pulse: a kick and a snare
/// on every beat at 120 per minute, or anything faster.
const BUSIEST_PULSE: f32 = 4.0;

/// Slow summaries of the music. Every value is in `0..=1` except `rise`,
/// in `-1..=1`.
#[derive(Clone, Copy, Debug, Default)]
pub struct Feel {
    /// Beats per second over the last few seconds, as a share of 4 (and
    /// above).
    pub pulse: f32,
    /// How loud, relative to the song's recent peak. With music playing it
    /// stays high, since the analysis levels every song to its own peaks;
    /// it falls in quiet passages and between songs.
    pub loudness: f32,
    /// How loud compared with the last half minute: 0.5 when steady, above
    /// in a part louder than what came before (a drop), below in a quieter
    /// one (a breakdown).
    pub swell: f32,
    /// How much treble: hi-hats, cymbals, noise.
    pub brightness: f32,
    /// Positive while the music grows (a build-up), negative while it
    /// thins out (a breakdown).
    pub rise: f32,
    /// Whether anything is playing at all.
    pub sounding: bool,
}

/// Follows the signals and keeps `Feel` up to date.
#[derive(Clone, Debug, Default)]
pub struct Listener {
    feel: Feel,
    /// Beats counted with an exponential memory: each adds 1, the sum
    /// fades over `PULSE_SECONDS`. The sum divided by that time is the rate.
    beats: f32,
    since_beat: f32,
    last_beat_signal: f32,
    fast: f32,
    slow: f32,
    long: f32,
    /// Seconds of silence so far.
    quiet: f32,
}

impl Listener {
    pub fn new() -> Self {
        Self { since_beat: f32::MAX, ..Self::default() }
    }

    pub fn feel(&self) -> Feel {
        self.feel
    }

    /// Whether `s` holds the first frame of a beat: the signal jumps up
    /// (it only ever falls between beats).
    pub fn is_beat(&self, s: &Signals) -> bool {
        s.beat > 0.5 && s.beat > self.last_beat_signal + 0.3
    }

    pub fn update(&mut self, s: &Signals) -> Feel {
        let dt = s.dt.clamp(0.0, 0.25);
        self.beats *= (-dt / PULSE_SECONDS).exp();
        self.since_beat += dt;
        if self.is_beat(s) && self.since_beat >= SHORTEST_BEAT {
            self.beats += 1.0;
            self.since_beat = 0.0;
        }
        self.last_beat_signal = s.beat;

        let follow = |value: &mut f32, target: f32, seconds: f32| *value += (target - *value) * (1.0 - (-dt / seconds).exp());
        let loudness_seconds = if s.energy > self.feel.loudness { LOUDNESS_RISE } else { LOUDNESS_FALL };
        follow(&mut self.feel.loudness, s.energy, loudness_seconds);
        follow(&mut self.feel.brightness, s.treble, BRIGHTNESS_SECONDS);
        follow(&mut self.fast, s.energy, RISE_FAST);
        follow(&mut self.slow, s.energy, RISE_SLOW);
        follow(&mut self.long, self.feel.loudness, SWELL_SECONDS);

        self.quiet = if s.energy < 0.02 { self.quiet + dt } else { 0.0 };
        self.feel.pulse = (self.beats / PULSE_SECONDS / BUSIEST_PULSE).min(1.0);
        self.feel.swell = (0.5 + 2.5 * (self.feel.loudness - self.long)).clamp(0.0, 1.0);
        self.feel.rise = (3.0 * (self.fast - self.slow)).clamp(-1.0, 1.0);
        self.feel.sounding = self.quiet < 1.0;
        self.feel
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use juglar_signals::Synthetic;

    /// The synthetic song's feel, sampled at each of `times` (seconds).
    fn feel_at(times: &[f32]) -> Vec<Feel> {
        let mut song = Synthetic::new(3);
        let mut listener = Listener::new();
        let mut s = Signals::default();
        let mut out = Vec::new();
        let mut t = 0.0;
        for &until in times {
            while t < until {
                song.next(1.0 / 60.0, &mut s);
                listener.update(&s);
                t += 1.0 / 60.0;
            }
            out.push(listener.feel());
        }
        out
    }

    #[test]
    fn pulse_counts_the_beats() {
        // Sections of 16 s at 120 per minute: no kicks in the intro, one every
        // other beat in the build (1/s), every beat in the drop (2/s).
        let [intro, build, drop] = feel_at(&[15.0, 31.0, 47.0])[..] else { unreachable!() };
        // The newest beat counts in full before it starts to fade, so the
        // rate reads about half a beat per window high.
        assert!(intro.pulse < 0.02, "{}", intro.pulse);
        assert!((build.pulse - 1.0 / BUSIEST_PULSE).abs() < 0.06, "{}", build.pulse);
        assert!((drop.pulse - 2.0 / BUSIEST_PULSE).abs() < 0.06, "{}", drop.pulse);
    }

    #[test]
    fn rise_follows_the_song_shape() {
        // Just after the drop starts it is rising; early in the breakdown
        // it is falling.
        let [drop, breakdown] = feel_at(&[34.0, 56.0])[..] else { unreachable!() };
        assert!(drop.rise > 0.2, "{}", drop.rise);
        assert!(breakdown.rise < -0.2, "{}", breakdown.rise);
        // Louder than the half minute before it, and quieter.
        assert!(drop.swell > 0.6, "{}", drop.swell);
        assert!(breakdown.swell < 0.4, "{}", breakdown.swell);
    }

    #[test]
    fn silence_is_noticed() {
        let mut listener = Listener::new();
        let quiet = Signals { dt: 1.0 / 60.0, ..Signals::default() };
        for _ in 0..90 {
            listener.update(&quiet);
        }
        assert!(!listener.feel().sounding);
    }
}
