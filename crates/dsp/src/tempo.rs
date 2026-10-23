//! Tempo: how fast the beats come, and where the next one falls.
//!
//! The onset detector says *that* a drum was hit; a musician also knows
//! *when the next hit will come*, and keeps counting through a fill or a
//! bar with no kick. This follows the same idea in two steps, once per hop
//! (10.7 ms), from the spectral flux (`onset`):
//!
//! **Period.** The flux, minus its average over the last second, is a
//! train of bumps, one per attack. Multiplying it with itself some lag
//! earlier and averaging over the last several seconds gives its
//! *autocorrelation*: large at lags where bumps line up with bumps, so at
//! the beat period and its multiples. Each lag is scored by its own value
//! plus half the value at twice the lag (a real beat lines up again one
//! beat later), weighted toward 120 beats per minute (tempos far from it,
//! in octaves, are less likely to be the one people tap). The best lag,
//! refined between hops, is the period. A new period has to win for a few
//! seconds before it replaces the old one, so a fill or a break doesn't
//! flip the tempo.
//!
//! **Phase.** A counter advances by one period per period. Each hop's flux
//! is added to a ring of bins by where the counter is within the beat;
//! older additions fade over a few seconds. Where the music hits, the ring
//! grows a bump: the counter's position there is the beat. `beat_phase` is
//! 0 on the beat and grows to 1 just before the next one.
//!
//! **Confidence.** The autocorrelation at the period, as a share of its
//! value at lag zero: near 1 for a metronome, near 0 for noise. Below
//! `CONFIDENT` the tempo is reported as unknown (0).

use std::f32::consts::TAU;

/// The slowest and fastest tempos considered, in beats per minute.
const SLOWEST_BPM: f32 = 60.0;
const FASTEST_BPM: f32 = 190.0;
/// Tempos are weighted toward this one, falling off by this many octaves
/// (one standard deviation). Most music people tap along to sits within an
/// octave of 120.
const LIKELIEST_BPM: f32 = 120.0;
const SPREAD_OCTAVES: f32 = 1.0;
/// Seconds the autocorrelation averages over.
const MEMORY_SECONDS: f32 = 6.0;
/// The flux's average is taken over this long, and subtracted.
const MEAN_SECONDS: f32 = 1.0;
/// The period is re-estimated this often, in seconds.
const ESTIMATE_SECONDS: f32 = 0.25;
/// A period within this share of the current one refines it; a different
/// one must win for `SWITCH_SECONDS` in a row to replace it.
const SAME_PERIOD: f32 = 0.05;
const SWITCH_SECONDS: f32 = 2.0;
/// How fast a refined period is followed, per estimate.
const REFINE: f32 = 0.3;
/// Bins in the phase ring, and how fast what they hold fades.
const PHASE_BINS: usize = 48;
const PHASE_SECONDS: f32 = 4.0;
/// Below this confidence, the tempo is unknown.
const CONFIDENT: f32 = 0.15;

pub struct Tempo {
    hop_seconds: f32,
    /// Lags of the slowest and fastest tempos, in hops.
    longest: usize,
    shortest: usize,
    /// Recent flux above its mean, smoothed, circular, `2 * longest + 1`
    /// long.
    past: Vec<f32>,
    /// The last two hops' flux above its mean, newest first.
    bumps: [f32; 2],
    write: usize,
    mean: f32,
    /// Autocorrelation by lag, `0..=2 * longest`.
    correlation: Vec<f32>,
    /// The prior weight of each lag.
    prior: Vec<f32>,
    since_estimate: f32,
    /// The period in hops (0 before the first estimate).
    period: f32,
    /// A different period that has been winning, and for how long.
    challenger: Option<(f32, f32)>,
    confidence: f32,
    /// Beats counted so far, as a fraction of the current beat.
    counter: f32,
    ring: [f32; PHASE_BINS],
    /// Where in the counter's beat the music hits, `0..1`.
    offset: f32,
}

impl Tempo {
    pub fn new(hop_seconds: f32) -> Self {
        let lag = |bpm: f32| 60.0 / bpm / hop_seconds;
        let longest = lag(SLOWEST_BPM).ceil() as usize;
        let shortest = lag(FASTEST_BPM).floor() as usize;
        let prior = (0..=2 * longest)
            .map(|l| {
                if l == 0 {
                    return 0.0;
                }
                let bpm = 60.0 / (l as f32 * hop_seconds);
                let octaves = (bpm / LIKELIEST_BPM).log2() / SPREAD_OCTAVES;
                (-0.5 * octaves * octaves).exp()
            })
            .collect();
        Self {
            hop_seconds,
            longest,
            shortest,
            past: vec![0.0; 2 * longest + 1],
            write: 0,
            bumps: [0.0; 2],
            mean: 0.0,
            correlation: vec![0.0; 2 * longest + 1],
            prior,
            since_estimate: 0.0,
            period: 0.0,
            challenger: None,
            confidence: 0.0,
            counter: 0.0,
            ring: [0.0; PHASE_BINS],
            offset: 0.0,
        }
    }

    /// Takes one hop's spectral flux.
    pub fn hop(&mut self, flux: f32) {
        let follow = |seconds: f32| 1.0 - (-self.hop_seconds / seconds).exp();
        self.mean += (flux - self.mean) * follow(MEAN_SECONDS);
        let bump = (flux - self.mean).max(0.0);

        // Autocorrelation, as a running average per lag, of the bumps
        // spread over three hops: a period that falls between two hops
        // (37.5 hops at 150 per minute) would otherwise line bumps up only
        // every other beat, and its double would win.
        let len = self.past.len();
        let spread = 0.25 * bump + 0.5 * self.bumps[0] + 0.25 * self.bumps[1];
        self.bumps = [bump, self.bumps[0]];
        self.past[self.write] = spread;
        let memory = follow(MEMORY_SECONDS);
        for (lag, value) in self.correlation.iter_mut().enumerate() {
            let earlier = self.past[(self.write + len - lag) % len];
            *value += (spread * earlier - *value) * memory;
        }
        self.write = (self.write + 1) % len;

        self.since_estimate += self.hop_seconds;
        if self.since_estimate >= ESTIMATE_SECONDS {
            self.since_estimate = 0.0;
            self.estimate();
        }

        // Phase: the counter, and where the music hits within its beat.
        if self.period > 0.0 {
            self.counter = (self.counter + 1.0 / self.period).fract();
            let fade = (-self.hop_seconds / PHASE_SECONDS).exp();
            self.ring.iter_mut().for_each(|v| *v *= fade);
            self.ring[(self.counter * PHASE_BINS as f32) as usize % PHASE_BINS] += bump;
            // The ring's center of mass, as an angle: squared, so the beat's
            // bump outweighs off-beat hits (hi-hats between beats).
            let (mut x, mut y) = (0.0, 0.0);
            for (i, &v) in self.ring.iter().enumerate() {
                let angle = TAU * (i as f32 + 0.5) / PHASE_BINS as f32;
                x += v * v * angle.cos();
                y += v * v * angle.sin();
            }
            if x != 0.0 || y != 0.0 {
                self.offset = (y.atan2(x) / TAU).rem_euclid(1.0);
            }
        }
    }

    fn estimate(&mut self) {
        let power = self.correlation[0];
        if power <= 1e-9 {
            self.confidence = 0.0;
            return;
        }
        let c = &self.correlation;
        let comb = |l: usize| c[l] + 0.5 * c.get(2 * l).copied().unwrap_or(0.0);
        // The prior picks among the peaks...
        let Some(picked) = (self.shortest..=self.longest).max_by(|&a, &b| {
            (comb(a) * self.prior[a]).total_cmp(&(comb(b) * self.prior[b]))
        }) else {
            return;
        };
        // ...but doesn't move the one it picked: weighted, a broad peak
        // leans toward 120 per minute, and over a minute of real music the
        // tempo crept from 134 to 118.
        let best = (picked.saturating_sub(2).max(self.shortest)..=(picked + 2).min(self.longest))
            .max_by(|&a, &b| comb(a).total_cmp(&comb(b)))
            .unwrap_or(picked);
        // Between hops: the peak of a parabola through the best lag and
        // its neighbors.
        let (left, middle, right) = (comb(best - 1), comb(best), comb(best + 1));
        let curve = left - 2.0 * middle + right;
        let shift = if curve < 0.0 { (0.5 * (left - right) / curve).clamp(-0.5, 0.5) } else { 0.0 };
        let found = best as f32 + shift;
        self.confidence = (c[best] / power).clamp(0.0, 1.0);

        if self.period == 0.0 || (found / self.period - 1.0).abs() < SAME_PERIOD {
            self.period = if self.period == 0.0 { found } else { self.period + (found - self.period) * REFINE };
            self.challenger = None;
            return;
        }
        // A different period: it must keep winning.
        self.challenger = match self.challenger {
            Some((period, held)) if (found / period - 1.0).abs() < SAME_PERIOD => Some((found, held + ESTIMATE_SECONDS)),
            _ => Some((found, 0.0)),
        };
        if let Some((period, held)) = self.challenger
            && held >= SWITCH_SECONDS
        {
            // Keep the counter's position in the beat, so the phase doesn't
            // jump more than the new period makes it.
            self.period = period;
            self.challenger = None;
        }
    }

    /// Beats per minute, or 0 while unknown.
    pub fn bpm(&self) -> f32 {
        if self.period > 0.0 && self.confidence >= CONFIDENT { 60.0 / (self.period * self.hop_seconds) } else { 0.0 }
    }

    /// Where the music is within the beat: 0 on it, rising to 1 just
    /// before the next. 0 while the tempo is unknown.
    pub fn beat_phase(&self) -> f32 {
        if self.bpm() > 0.0 { (self.counter - self.offset).rem_euclid(1.0) } else { 0.0 }
    }

    /// How sure the tempo is, `0..=1` (see the module notes).
    pub fn confidence(&self) -> f32 {
        self.confidence
    }
}

#[cfg(test)]
mod tests {
    use crate::onset::Onsets;
    use crate::onset::tests_support::*;

    /// Feeds `signal` to a detector; returns it, and the beat phase sampled
    /// a hop after each time in `at`.
    fn listen(signal: &[f32], at: &[f32]) -> (Onsets, Vec<f32>) {
        let mut detector = Onsets::new(RATE);
        let mut phases = Vec::new();
        let mut done = 0;
        for &t in at {
            let until = seconds(t + 0.02).min(signal.len());
            if until > done {
                detector.push(&signal[done..until]);
                done = until;
            }
            phases.push(detector.tempo().beat_phase());
        }
        detector.push(&signal[done..]);
        (detector, phases)
    }

    /// The phase's distance from the beat, either side.
    fn off_beat(phase: f32) -> f32 {
        phase.min(1.0 - phase)
    }

    #[test]
    fn finds_the_tempo_and_phase_of_clicks() {
        for bpm in [90.0, 128.0, 150.0] {
            let times = tempo(bpm, 14.0, 0.3);
            let signal = clicks(14.0, &times, 0.02);
            let late: Vec<f32> = times.iter().copied().filter(|&t| t > 9.0).collect();
            let (detector, phases) = listen(&signal, &late);
            let found = detector.tempo().bpm();
            assert!((found / bpm - 1.0).abs() < 0.02, "{bpm}: found {found}");
            // Just after each click, the phase says "on the beat" (within
            // a tenth of a beat).
            for phase in phases {
                assert!(off_beat(phase) < 0.1, "{bpm}: phase {phase}");
            }
        }
    }

    #[test]
    fn kicks_with_hi_hats_between_are_the_beat_not_the_hats() {
        // Kicks at 100 per minute, hi-hats on the eighths in between: the tempo is 100, not 200, and the beat is on the kicks.
        let kicks = tempo(100.0, 14.0, 0.4);
        let hats: Vec<f32> = kicks.iter().map(|t| t + 0.3).collect();
        let mut signal = clicks(14.0, &[], 0.01);
        add_hats(&mut signal, &hats, 0.2);
        add_kicks(&mut signal, &kicks, 0.6);
        let late: Vec<f32> = kicks.iter().copied().filter(|&t| t > 9.0).collect();
        let (detector, phases) = listen(&signal, &late);
        let found = detector.tempo().bpm();
        assert!((found / 100.0 - 1.0).abs() < 0.02, "found {found}");
        for phase in phases {
            assert!(off_beat(phase) < 0.12, "phase {phase}");
        }
    }

    #[test]
    fn follows_a_change_of_tempo() {
        let first = tempo(100.0, 12.0, 0.3);
        let mut signal = clicks(24.0, &first, 0.02);
        let second: Vec<f32> = tempo(140.0, 12.0, 0.2).iter().map(|t| t + 12.0).collect();
        let more = clicks(24.0, &second, 0.0);
        for (s, m) in signal.iter_mut().zip(&more) {
            *s += m;
        }
        let (detector, _) = listen(&signal, &[]);
        let found = detector.tempo().bpm();
        assert!((found / 140.0 - 1.0).abs() < 0.02, "found {found}");
    }

    #[test]
    fn a_steady_tempo_far_from_120_stays_put() {
        // A minute of kicks at 165 per minute under hiss: the estimate
        // neither drifts toward 120 nor wanders.
        let times = tempo(165.0, 60.0, 0.2);
        let mut signal = clicks(60.0, &[], 0.05);
        add_kicks(&mut signal, &times, 0.5);
        let mut detector = Onsets::new(RATE);
        let mut found = Vec::new();
        for second in signal.chunks(seconds(1.0)) {
            detector.push(second);
            found.push(detector.tempo().bpm());
        }
        for (t, bpm) in found.iter().enumerate().skip(12) {
            assert!((bpm / 165.0 - 1.0).abs() < 0.01, "at {t} s: {bpm}");
        }
    }

    #[test]
    fn noise_and_silence_have_no_tempo() {
        let (detector, _) = listen(&clicks(15.0, &[], 0.3), &[]);
        assert_eq!(detector.tempo().bpm(), 0.0, "confidence {}", detector.tempo().confidence());
        let (detector, _) = listen(&vec![0.0; seconds(10.0)], &[]);
        assert_eq!(detector.tempo().bpm(), 0.0);
        assert_eq!(detector.tempo().beat_phase(), 0.0);
    }
}
