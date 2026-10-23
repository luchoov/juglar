//! How heavy the sound is, and when it slams: an ear for dense, distorted
//! music, where the other detectors go quiet.
//!
//! Everything else in the analysis is leveled or thresholded against its
//! own recent past, and a wall of distorted guitars defeats both: the
//! levels sit near the top through a riff and through the break before it,
//! and in a compressed master every hit rises only a few decibels over the
//! guitars, under thresholds made for clean drums. Measured on Acid Bath's
//! "Jezebel": the explosion at 3:43, after a near-silent break, moved
//! neither the levels nor novelty, and fewer drums were heard in it than in
//! the quiet parts. So this module measures differently, on `onset`'s
//! transform (21 ms window, 10.7 ms hop):
//!
//! - **Loudness against the song.** The whole band's level (40 Hz–16 kHz),
//!   averaged over 0.3 s, against a reference that follows the loudest
//!   moments and forgets them at 0.25 dB a second, a minute of quiet song
//!   to come down 15 dB. 1 within 3 dB of the reference, 0 at 20 dB below.
//!   A break reads as a break; a chorus at full blast reads 1.
//! - **Impact.** An explosion: the level, over 80 ms, jumps at least 10 dB
//!   above the quietest it has been in the last 2.5 s (since the last
//!   impact, if that was more recent), landing within 6 dB of the song's
//!   reference. Nothing builds up to it, so nothing else sees it coming.
//!   On "Jezebel" it fires at 3:42.9; across 21 songs it also fires where
//!   a stop-start riff slams back in, and when a song starts. Its event is
//!   1 and falls back over 0.6 s.
//! - **Grit.** How noise-like the middle of the spectrum (250 Hz–4 kHz) is:
//!   its spectral flatness, the geometric mean of the bins' power over
//!   their arithmetic mean. Clean notes are a few peaks (flatness near 0),
//!   distortion fills the gaps between harmonics. Averaged over 0.5 s and
//!   mapped from 0.10 (0) to 0.40 (1): over 21 songs the loud parts of
//!   classic rock sit at 0.10–0.17, thrash and sludge at 0.29–0.36.
//! - **Fuzz** (D62), for what grit misses. A fuzz pedal makes a dense comb
//!   of harmonics, not noise, piled into the low middle, so flatness reads
//!   Electric Wizard's wall as clean as classic rock. Two cues together
//!   catch it: the same band's flatness after dividing each bin by the
//!   mean of its neighbors within 250 Hz (whitened, so the slope of the
//!   spectrum no longer hides the valleys between harmonics), 0.40 to
//!   0.55; and how compressed the sound is, its RMS over its peak in the
//!   window (a wall of fuzz barely peaks above itself), 0.40 to 0.48.
//!   Each averaged over 0.5 s; the smaller of the two raises grit, never
//!   lowers it. Over 21 songs it lifts Electric Wizard's loud parts from
//!   0.09 to 0.78, and leaves clean intros and classic rock where they were.
//! - **Hits.** Any attack, wherever it is: the mean rise over all of
//!   `onset`'s bands, peaking above the median of its last second by more
//!   than 1.5 median deviations, at most one per 60 ms. Medians don't rise
//!   with a wall of hits the way a mean does, so this never goes deaf: in
//!   "Jezebel" it hears 7–9 attacks a second in the quiet part and in the
//!   explosion alike. The event's height says how far above the median it
//!   was (0.4 to 1), and it falls back over 0.06 s.
//! - **Density.** Attacks per second, counted over the last 1.5 s: 0 at 3
//!   or fewer, 1 at 12 or more.
//! - **Chaos.** All of it in one number for scenes: loudness times a mix of
//!   grit (70%) and density (30%). About 0.2 for an acoustic intro, 0.45
//!   for a clean, busy verse, 0.7–0.85 for a wall of distortion.

use crate::analyzer::Bins;
use crate::fft::Complex;

/// Band edges of the whole sound, and of the middle whose flatness is grit.
const FULL_HZ: (f32, f32) = (40.0, 16_000.0);
const GRIT_HZ: (f32, f32) = (250.0, 4_000.0);
/// Time constants of the fast and the steady level, seconds.
const FAST_SECONDS: f32 = 0.08;
const STEADY_SECONDS: f32 = 0.3;
/// How fast the song's reference level forgets its loudest moment.
const REFERENCE_FALL_DB_PER_S: f32 = 0.25;
/// Steady level against the reference that maps to loudness 0 and 1.
const LOUDNESS_DB: (f32, f32) = (-20.0, -3.0);
/// An impact: a jump of at least this many decibels over the quietest
/// steady level of the last `IMPACT_LOOK` seconds, landing within
/// `IMPACT_NEAR_DB` of the reference, at most once per `IMPACT_REST`.
const IMPACT_JUMP_DB: f32 = 10.0;
const IMPACT_NEAR_DB: f32 = 6.0;
const IMPACT_LOOK: f32 = 2.5;
const IMPACT_REST: f32 = 1.0;
const IMPACT_DECAY: f32 = 0.6;
/// Below this the reference isn't a song yet.
const SILENCE_DB: f32 = -60.0;
/// Grit: the flatness's time constant, and the flatness at 0 and 1.
const GRIT_SECONDS: f32 = 0.5;
const GRIT_FLATNESS: (f32, f32) = (0.10, 0.40);
/// Fuzz: how far around each bin its neighbors are averaged, the whitened
/// flatness at 0 and 1, and the RMS over peak at 0 and 1.
const FUZZ_REACH_HZ: f32 = 250.0;
const FUZZ_FLATNESS: (f32, f32) = (0.40, 0.55);
const FUZZ_PRESSED: (f32, f32) = (0.40, 0.48);
/// Hits: seconds of flux their median is taken over, how many median
/// deviations above it a peak must be, the shortest gap, and the fall.
const HIT_SECONDS: f32 = 1.0;
const HIT_SPREAD: f32 = 1.5;
const HIT_GAP: f32 = 0.06;
const HIT_DECAY: f32 = 0.06;
/// A median deviation of a normal distribution, in standard deviations.
const MAD_TO_SIGMA: f32 = 1.4826;
/// Density: seconds hits are counted over, and the rates at 0 and 1.
const DENSITY_SECONDS: f32 = 1.5;
const DENSITY_RATE: (f32, f32) = (3.0, 12.0);
/// How chaos mixes grit and density.
const CHAOS_GRIT: f32 = 0.7;

pub struct Chaos {
    hop_seconds: f32,
    full: Bins,
    grit_bins: Bins,
    /// Fuzz: bins on either side a bin is whitened against, the bins'
    /// powers from `reach` below the grit band to `reach` above it, and
    /// the averaged whitened flatness and RMS over peak.
    reach: usize,
    powers: Vec<f32>,
    whitened: f32,
    pressed: f32,
    /// Fast power (a mean square); the steady level and the reference, in
    /// decibels.
    fast: f32,
    steady: f32,
    reference: f32,
    /// Steady levels of the last `IMPACT_LOOK` seconds, circular, and how
    /// many of them count (fewer right after an impact).
    steady_past: Vec<f32>,
    steady_write: usize,
    steady_count: usize,
    since_impact: f32,
    impact: f32,
    impacts: u64,
    flatness: f32,
    /// The last second of flux, circular, and a copy to find medians in.
    flux_past: Vec<f32>,
    flux_write: usize,
    flux_count: usize,
    sorted: Vec<f32>,
    /// Flux of the last three hops, oldest first.
    recent: [f32; 3],
    since_hit: f32,
    hit: f32,
    hits: u64,
    /// Hits counted with an exponential memory of `DENSITY_SECONDS`.
    rate: f32,
    loudness: f32,
}

/// What the module heard on the last hop (see the module's notes).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Heavy {
    pub loudness: f32,
    pub grit: f32,
    pub density: f32,
    pub chaos: f32,
    /// Events: 1 at an impact or a hit, falling back to 0.
    pub impact: f32,
    pub hit: f32,
}

impl Chaos {
    /// For `onset`'s transform: `size` samples, `hop` apart.
    pub(crate) fn new(sample_rate: f32, size: usize, hop: usize) -> Self {
        let hop_seconds = hop as f32 / sample_rate;
        let bins = |(low, high): (f32, f32)| {
            let bin = sample_rate / size as f32;
            Bins { start: ((low / bin).round() as usize).max(1), end: ((high / bin).round() as usize).min(size / 2) }
        };
        let flux_len = (HIT_SECONDS / hop_seconds).round() as usize;
        let grit_bins = bins(GRIT_HZ);
        let reach = ((FUZZ_REACH_HZ * size as f32 / sample_rate).round() as usize).max(1);
        Self {
            hop_seconds,
            full: bins(FULL_HZ),
            reach,
            powers: vec![0.0; grit_bins.end - grit_bins.start + 2 * reach],
            whitened: 0.0,
            pressed: 0.0,
            grit_bins,
            fast: 0.0,
            steady: SILENCE_DB,
            reference: SILENCE_DB,
            steady_past: vec![0.0; (IMPACT_LOOK / hop_seconds).round() as usize],
            steady_write: 0,
            steady_count: 0,
            since_impact: f32::INFINITY,
            impact: 0.0,
            impacts: 0,
            flatness: 0.0,
            flux_past: vec![0.0; flux_len],
            flux_write: 0,
            flux_count: 0,
            sorted: vec![0.0; flux_len],
            recent: [0.0; 3],
            since_hit: f32::INFINITY,
            hit: 0.0,
            hits: 0,
            rate: 0.0,
            loudness: 0.0,
        }
    }

    /// One hop: the transform (`power_scale` turns a sum of bin powers into
    /// a mean square), the window's RMS over its peak, `onset`'s flux over
    /// all its bands, and whether this is sound right after real silence.
    pub(crate) fn hop(&mut self, scratch: &[Complex], power_scale: f32, pressed: f32, flux: f32, waking: bool) {
        let dt = self.hop_seconds;
        let follow = |tau: f32| 1.0 - (-dt / tau).exp();

        // Loudness against the song.
        let power = scratch[self.full.start..self.full.end].iter().map(|x| x.norm_sqr()).sum::<f32>() * power_scale;
        if waking {
            // What came before was silence: a song starting loud slams.
            (self.fast, self.steady) = (power, db(power));
            self.reference = db(power);
            self.steady_past[0] = SILENCE_DB;
            (self.steady_write, self.steady_count) = (1, 1);
        }
        self.fast += (power - self.fast) * follow(FAST_SECONDS);
        // The steady level is averaged in decibels, so it sinks into a
        // break as fast as it rises out of one.
        self.steady += (db(power) - self.steady) * follow(STEADY_SECONDS);
        let (fast, steady) = (db(self.fast), self.steady);
        self.reference = steady.max(self.reference - REFERENCE_FALL_DB_PER_S * dt);
        self.loudness = ramp(LOUDNESS_DB, steady - self.reference);

        // Impact: against the quietest of the last few seconds (since the
        // last impact), not counting this hop.
        let quietest = self.steady_past.iter().cycle().skip(self.steady_write + self.steady_past.len() - self.steady_count);
        let floor = quietest.take(self.steady_count).copied().fold(f32::INFINITY, f32::min);
        self.since_impact += dt;
        self.impact *= (-dt / IMPACT_DECAY).exp();
        if fast - floor >= IMPACT_JUMP_DB
            && fast >= self.reference - IMPACT_NEAR_DB
            && self.since_impact >= IMPACT_REST
            && self.reference > SILENCE_DB
        {
            self.impact = 1.0;
            self.impacts += 1;
            self.since_impact = 0.0;
            self.steady_count = 0;
            // The slam is the new level: the next impact needs a new break.
            self.steady = self.steady.max(fast);
        }
        self.steady_past[self.steady_write] = self.steady;
        self.steady_write = (self.steady_write + 1) % self.steady_past.len();
        self.steady_count = (self.steady_count + 1).min(self.steady_past.len());

        // Grit: spectral flatness of the middle, from the mean of the log.
        let bins = &scratch[self.grit_bins.start..self.grit_bins.end];
        let n = bins.len() as f32;
        let (log_sum, sum) = bins.iter().fold((0.0, 0.0), |(l, s), x| {
            let p = x.norm_sqr() + 1e-20;
            (l + p.ln(), s + p)
        });
        let flatness = (log_sum / n).exp() / (sum / n);
        self.flatness += (flatness - self.flatness) * follow(GRIT_SECONDS);

        // Fuzz: the same flatness once each bin is divided by the mean of
        // its neighbors, which takes the spectrum's slope away and leaves
        // the valleys between harmonics; and how little the sound peaks.
        // Past the transform's ends, its last bin stands in.
        let last = scratch.len() / 2;
        let low = self.grit_bins.start as isize - self.reach as isize;
        for (i, power) in self.powers.iter_mut().enumerate() {
            let bin = (low + i as isize).clamp(1, last as isize) as usize;
            *power = scratch[bin].norm_sqr();
        }
        let width = 2 * self.reach + 1;
        let mut around: f32 = self.powers[..width].iter().sum();
        let (mut log_sum, mut sum) = (0.0, 0.0);
        for i in self.reach..self.powers.len() - self.reach {
            if i > self.reach {
                around += self.powers[i + self.reach] - self.powers[i - self.reach - 1];
            }
            let q = self.powers[i] / (around.max(0.0) / width as f32 + 1e-20) + 1e-20;
            (log_sum, sum) = (log_sum + q.ln(), sum + q);
        }
        let whitened = (log_sum / n).exp() / (sum / n);
        self.whitened += (whitened - self.whitened) * follow(GRIT_SECONDS);
        self.pressed += (pressed - self.pressed) * follow(GRIT_SECONDS);

        // Hits: a peak of the flux above its last second's median.
        self.recent = [self.recent[1], self.recent[2], flux];
        self.since_hit += dt;
        self.hit *= (-dt / HIT_DECAY).exp();
        self.rate *= (-dt / DENSITY_SECONDS).exp();
        if self.flux_count >= self.flux_past.len() / 2 {
            let past = &mut self.sorted[..self.flux_count];
            past.copy_from_slice(&self.flux_past[..self.flux_count]);
            let median = *past.select_nth_unstable_by(past.len() / 2, f32::total_cmp).1;
            for value in past.iter_mut() {
                *value = (*value - median).abs();
            }
            let spread = *past.select_nth_unstable_by(past.len() / 2, f32::total_cmp).1 * MAD_TO_SIGMA + 1e-3;
            let [before, candidate, after] = self.recent;
            let z = (candidate - median) / spread;
            // The peak was one hop ago.
            if candidate > before && candidate >= after && z > HIT_SPREAD && self.since_hit >= HIT_GAP + dt {
                self.hit = self.hit.max((0.4 + 0.6 * z / 6.0).min(1.0));
                self.hits += 1;
                self.rate += 1.0;
                self.since_hit = dt;
            }
        }
        self.flux_past[self.flux_write] = flux;
        self.flux_write = (self.flux_write + 1) % self.flux_past.len();
        self.flux_count = (self.flux_count + 1).min(self.flux_past.len());
    }

    /// What was heard on the last hop.
    pub fn heavy(&self) -> Heavy {
        let fuzz = ramp(FUZZ_FLATNESS, self.whitened).min(ramp(FUZZ_PRESSED, self.pressed));
        let grit = ramp(GRIT_FLATNESS, self.flatness).max(fuzz);
        let density = ramp(DENSITY_RATE, self.rate / DENSITY_SECONDS);
        Heavy {
            loudness: self.loudness,
            grit,
            density,
            chaos: self.loudness * (CHAOS_GRIT * grit + (1.0 - CHAOS_GRIT) * density),
            impact: self.impact,
            hit: self.hit,
        }
    }

    /// Grit's parts, for tests: the flatness, whitened flatness and RMS
    /// over peak, each averaged.
    #[cfg(test)]
    fn parts(&self) -> [f32; 3] {
        [self.flatness, self.whitened, self.pressed]
    }

    /// Impacts and hits found so far.
    pub fn counts(&self) -> (u64, u64) {
        (self.impacts, self.hits)
    }
}

/// A mean square in decibels.
fn db(power: f32) -> f32 {
    10.0 * power.max(1e-12).log10()
}

/// `x` from `range.0` (0) to `range.1` (1), clamped.
fn ramp(range: (f32, f32), x: f32) -> f32 {
    ((x - range.0) / (range.1 - range.0)).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use crate::onset::Onsets;
    use crate::onset::tests_support::*;

    const RATE: u32 = 48_000;

    /// Runs `signal` through the detector; the heavy readings after every
    /// 10 ms and the counts at the end.
    fn run(signal: &[f32]) -> (Vec<super::Heavy>, (u64, u64)) {
        let mut onsets = Onsets::new(RATE);
        let mut out = Vec::new();
        for chunk in signal.chunks(RATE as usize / 100) {
            onsets.push(chunk);
            out.push(onsets.chaos().heavy());
        }
        (out, onsets.chaos().counts())
    }

    #[test]
    fn a_slam_after_a_break_is_an_impact_and_a_steady_wall_is_not() {
        // 6 s of distorted guitar, a second of near silence, then the
        // guitar again: one impact when it comes back (and one at the
        // start), none while it holds.
        let mut signal = distorted_guitar(6.0, 8.0, 0.5);
        let mut quiet = distorted_guitar(1.0, 8.0, 0.5);
        quiet.iter_mut().for_each(|s| *s *= 0.01);
        signal.extend(quiet);
        signal.extend(distorted_guitar(4.0, 8.0, 0.5));
        let (heard, (impacts, _)) = run(&signal);
        assert_eq!(impacts, 2, "one at the start, one after the break");
        // Within 0.15 s of the guitar's return at 7 s.
        let back = heard[700..715].iter().any(|h| h.impact > 0.9);
        assert!(back, "{:?}", heard[695..720].iter().map(|h| h.impact).collect::<Vec<_>>());
        // In the break, loudness drops; in the wall, it is full.
        assert!(heard[690].loudness < 0.3, "{}", heard[690].loudness);
        assert!(heard[1000].loudness > 0.9, "{}", heard[1000].loudness);
    }

    #[test]
    fn noise_is_gritty_and_a_clean_chord_is_not() {
        // Grit was calibrated on real songs; synthetic distortion is too
        // clean (a few exact harmonics), so the ends are tested here.
        let mut noise = Noise(7);
        let hiss: Vec<f32> = (0..4 * RATE as usize).map(|_| 0.3 * noise.next()).collect();
        let (hiss, _) = run(&hiss);
        let chord: Vec<f32> = (0..4 * RATE as usize)
            .map(|i| [196.0, 247.0, 294.0].iter().map(|hz| 0.2 * (std::f32::consts::TAU * hz * i as f32 / RATE as f32).sin()).sum())
            .collect();
        let (chord, _) = run(&chord);
        assert!(hiss[350].grit > 0.9, "{}", hiss[350].grit);
        assert!(chord[350].grit < 0.1, "{}", chord[350].grit);
        assert!(hiss[350].chaos > chord[350].chaos + 0.5);
    }

    #[test]
    fn a_wall_of_fuzz_is_gritty_and_a_ringing_chord_is_not() {
        // Doom's fuzz: a low power chord clipped hard and darkened, a comb
        // of harmonics with almost no noise, that never decays. Plain grit
        // reads it as clean (its flatness stays near 0.06).
        let low_pass = |signal: &mut Vec<f32>, hz: f32| {
            let k = 1.0 - (-std::f32::consts::TAU * hz / RATE as f32).exp();
            let mut y = 0.0;
            signal.iter_mut().for_each(|x| {
                y += (*x - y) * k;
                *x = y;
            });
        };
        let chord = |i: usize| {
            let t = i as f32 / RATE as f32;
            [55.0, 82.4, 110.0].iter().map(|hz| (hz * t).fract() * 2.0 - 1.0).sum::<f32>()
        };
        let mut fuzz: Vec<f32> = (0..4 * RATE as usize).map(|i| 0.5 * (12.0 * chord(i)).tanh()).collect();
        low_pass(&mut fuzz, 1_500.0);
        // A clean guitar's open chord: eight harmonics falling off as 1/n²,
        // picked twice a second.
        let clean: Vec<f32> = (0..4 * RATE as usize)
            .map(|i| {
                let t = i as f32 / RATE as f32;
                let ring = (-(t % 0.5) / 0.4).exp();
                let notes = [196.0, 247.0, 294.0].iter().map(|hz| {
                    (1..=8).map(|n| (std::f32::consts::TAU * hz * n as f32 * t).sin() / (n * n) as f32).sum::<f32>()
                });
                0.2 * ring * notes.sum::<f32>()
            })
            .collect();
        let parts = |signal: &[f32]| {
            let mut onsets = Onsets::new(RATE);
            onsets.push(signal);
            onsets.chaos().parts()
        };
        // Plain flatness alone would give the fuzz no grit at all.
        assert!(parts(&fuzz[..350 * 480])[0] < 0.1, "{:?}", parts(&fuzz[..350 * 480]));
        let (fuzz, _) = run(&fuzz);
        let (clean, _) = run(&clean);
        assert!(fuzz[350].grit > 0.6, "{}", fuzz[350].grit);
        assert!(clean[350].grit < 0.2, "{}", clean[350].grit);
    }

    #[test]
    fn hits_keep_coming_under_a_wall_of_guitar() {
        // Kicks at 8 a second under a loud guitar: the hits keep up.
        let mut signal = distorted_guitar(6.0, 8.0, 0.5);
        let kicks: Vec<f32> = (0..48).map(|k| 0.5 + k as f32 / 8.0).collect();
        add_kick_drums(&mut signal, &kicks, 0.6, 0.08);
        limit(&mut signal, 4.0);
        let (_, (_, hits)) = run(&signal);
        assert!(hits >= 35, "{hits} hits for 48 kicks");
    }
}
