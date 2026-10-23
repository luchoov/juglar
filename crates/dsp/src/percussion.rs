//! The drums apart from everything else: kick, snare and cymbals, and how
//! much of the sound is percussive and how much harmonic.
//!
//! **Percussive and harmonic.** A guitar chord holds its level in each band
//! for a while; a drum hit jumps above it for a few hundredths of a second.
//! So every hop (10.7 ms, on `onset`'s transform), each band's *usual level*
//! is the median of its last 0.2 s. A hit is too short to move a median,
//! and a held chord is its own median. Whatever a band has above its usual
//! level is percussive; the rest is harmonic. Medians over time are the
//! classic way to split the two; this one only looks back, so the
//! percussive part is known at once and only the harmonic part trails.
//!
//! **Where each drum is.** Each drum is looked for in its own region: the
//! kick's thump at 40–120 Hz, the snare's body and crack at 165–2500 Hz,
//! the cymbals at 2.5–16 kHz. In each region the flux is computed as in
//! `onset` (how much each band got louder than two hops ago, in deviations
//! of steady noise), with one change: a rise counts only from the band's
//! usual level up. Tremolo picking dips a chord sixteen times a second, and
//! climbing back out of each dip is not a hit.
//!
//! **Thresholds for fast drums.** `onset`'s threshold (the last second's
//! mean plus three deviations) fails on a double kick: sixteen hits a second
//! lift the mean and the deviation above every hit. The kick and cymbals use
//! the median plus four *median* deviations instead: at least half the hops
//! of the fastest double kick fall between hits, so the median stays there.
//! The snare must stay quiet through tremolo picking, which fills its
//! region with sixteen small attacks a second; its threshold is 1.2 times
//! the typical peak of the last second (the median of the local peaks),
//! which is the picking itself when that is all there is. Every threshold
//! has a floor, high enough that 30 s of white noise fires nothing.
//!
//! **Telling the drums apart.** Each drum spills into the others' regions,
//! so a hit only counts when it isn't the spill of a louder one:
//!
//! - A kick doesn't count when the snare's region rose, at the same
//!   moment, more than two thirds as much (in decibels), and a snare
//!   doesn't count when the kick's region rose much more. In a loud mix the bass guitar fills the kick's region
//!   too, so a kick also needs its *click*, the beater's tick at 2.5–6 kHz,
//!   unless its thump is unmistakable on its own. And a sharp click with no
//!   snare and no cymbal in it is a kick even without a thump: in dense
//!   metal the click is often all of the kick that gets through.
//! - A cymbal is bright: its sound reaches 16 kHz in at least the
//!   proportions of the music around it, while a kick's click or a snare's
//!   crack lands mostly below 6 kHz. So a cymbal counts only when its
//!   percussive power above 6 kHz, against its power at 2.5–6 kHz, is at
//!   least 1.2 times what the harmonic part has in the same two places.
//!   Measuring against the harmonic part makes the test the same for a
//!   bright master and a dull one.
//!
//! **Events.** Each drum's event is 1 at its hit and falls back to 0 at a
//! constant rate, with a time constant of 0.04 s (kick) or 0.05 s: between
//! two kicks of a double kick at 16 per second, 62 ms apart, it falls to a
//! fifth. A drum can't fire again within 40 ms. As with onsets, a hit is
//! confirmed one hop later, so it arrives 10.7 ms late.
//!
//! **What it can't do.** In a loud master each drum is masked by the
//! guitars and by the other drums, and nothing here finds them all (see
//! `docs/theory/12-drums.md` for the measurements). A drum alone over
//! digital silence is hard too: the faintest spill of a kick into the
//! snare's region is a huge rise in decibels over nothing.

use crate::analyzer::{Bins, log_bands};
use crate::fft::Complex;
use crate::onset::merged;

/// The three drums: kick, snare, cymbals.
pub const DRUMS: usize = 3;
const KICK: usize = 0;
const SNARE: usize = 1;
const CYMBALS: usize = 2;
/// The beater's click (and the guitars' pick noise): not a drum, but what
/// tells a kick from the bass.
const CLICK: usize = 3;
/// Above the click: how bright a hit is.
const BRIGHT: usize = 4;
const REGIONS: usize = 5;

/// Each region in Hz, how many log-spaced bands it is divided into before
/// merging, and how its threshold is set (see `Threshold`) with its floor
/// in deviations of steady noise. Kick, snare, cymbals, click, bright; the
/// bright region picks no peaks of its own.
const REGION_SETUP: [((f32, f32), usize, Picking); REGIONS] = [
    ((40.0, 120.0), 1, Some((Threshold::Spread(4.0), 0.4))),
    ((165.0, 2_500.0), 8, Some((Threshold::Typical(1.2), 1.15))),
    ((2_500.0, 16_000.0), 9, Some((Threshold::Spread(4.0), 1.5))),
    ((2_500.0, 6_000.0), 3, Some((Threshold::Spread(0.0), 1.45))),
    ((6_000.0, 16_000.0), 6, None),
];
/// The rest of the spectrum, which counts toward the percussive and
/// harmonic energies but holds no region of its own.
const OTHER_BANDS: [((f32, f32), usize); 1] = [((120.0, 165.0), 1)];
/// Silence, in decibels, as in `onset`.
const SILENCE_DB: f32 = -100.0;
/// How much the level of a one-bin band jumps around in steady noise, in
/// decibels, as in `onset`.
const NOISE_DB: f32 = 6.0;
/// Seconds of past levels each band's usual level is the median of: longer
/// than a drum hit, shorter than a chord.
const USUAL_SECONDS: f32 = 0.2;
/// Each band's level is compared with its level this many hops ago, as in
/// `onset`.
const LAG: usize = 2;
/// Seconds of past flux each region's threshold is computed from.
const THRESHOLD_SECONDS: f32 = 1.0;
/// The shortest gap between two hits of one drum, in seconds: a blast beat
/// or a double kick can pass 20 hits a second.
const REFRACTORY: f32 = 0.04;
/// How fast each drum's event falls back to 0 after a hit, in seconds.
const DECAY: [f32; DRUMS] = [0.04, 0.05, 0.05];
/// A kick must rise at least this many times as much as the snare's region
/// (in decibels, over the same hops), and a snare at least this many times
/// as much as the kick's.
const KICK_OVER_SNARE: f32 = 1.5;
const SNARE_OVER_KICK: f32 = 0.6;
/// A kick needs its click: the click region this many times over its own
/// threshold in the hops around the thump, unless the thump alone reaches
/// a flux of `KICK_ALONE`.
const KICK_CLICK: f32 = 1.2;
const KICK_ALONE: f32 = 4.0;
/// A click alone is a kick when it rises this many decibels (averaged over
/// its bands), the snare's region rises less than this fraction of it, and
/// the cymbals' less than this many times it.
const CLICK_KICK: (f32, f32, f32) = (6.0, 0.18, 1.5);
/// How much brighter than the harmonic part a cymbal must be (see the
/// module notes).
const BRIGHTNESS: f32 = 1.0;
/// Seconds the harmonic power is averaged over for the brightness test.
const TILT_SECONDS: f32 = 1.0;
/// The percussive power is held after a hit with this time constant, in
/// seconds: about one frame, so a display reading it once a frame doesn't
/// miss a hit that lasted one hop.
const PERCUSSIVE_HOLD: f32 = 0.02;

/// How a region's threshold follows its last second of flux.
#[derive(Clone, Copy)]
enum Threshold {
    /// The median plus this many median absolute deviations.
    Spread(f32),
    /// This many times the median of the local peaks.
    Typical(f32),
}

/// A region's threshold and its floor, if it picks peaks.
type Picking = Option<(Threshold, f32)>;

/// One region's flux and its peak picking.
struct Region {
    /// Its bands, as a range of indices into `Percussion::bands`.
    first: usize,
    end: usize,
    threshold_rule: Picking,
    /// Past flux values, circular, for the threshold.
    past: Vec<f32>,
    write: usize,
    /// A copy of `past` to find medians in.
    sorted: Vec<f32>,
    /// Flux of the last three hops, oldest first, and of the hop before.
    recent: [f32; 3],
    earlier: f32,
    /// The same three hops' mean rise in decibels, to compare regions with,
    /// and their percussive power; and this hop's harmonic power.
    rise_db: [f32; 3],
    percussive: [f32; 3],
    harmonic: f32,
    threshold: f32,
    /// Samples since this region's last hit.
    since: Option<u64>,
    count: u64,
}

impl Region {
    /// Takes this hop's flux; returns whether the previous hop is a peak
    /// above the threshold, rested.
    fn hop(&mut self, flux: f32, refractory_samples: u64) -> bool {
        let Some((rule, floor)) = self.threshold_rule else {
            self.recent = [self.recent[1], self.recent[2], flux];
            return false;
        };
        let raw = match rule {
            Threshold::Spread(spread) => {
                self.sorted.copy_from_slice(&self.past);
                let middle = self.sorted.len() / 2;
                let median = *self.sorted.select_nth_unstable_by(middle, f32::total_cmp).1;
                for v in &mut self.sorted {
                    *v = (*v - median).abs();
                }
                median + spread * *self.sorted.select_nth_unstable_by(middle, f32::total_cmp).1
            }
            Threshold::Typical(factor) => factor * typical_peak(&self.past, self.write, &mut self.sorted),
        };
        self.threshold = raw.max(floor);
        self.past[self.write] = flux;
        self.write = (self.write + 1) % self.past.len();

        self.earlier = self.recent[0];
        self.recent = [self.recent[1], self.recent[2], flux];
        let [before, candidate, after] = self.recent;
        let rested = self.since.is_none_or(|since| since >= refractory_samples);
        candidate > before && candidate >= after && candidate > self.threshold && rested
    }

    /// The largest rise in decibels over the last three hops.
    fn largest_rise_db(&self) -> f32 {
        self.rise_db.iter().copied().fold(0.0, f32::max)
    }

    /// The percussive power at the candidate hop or the one after,
    /// whichever is larger.
    fn percussive_peak(&self) -> f32 {
        self.percussive[1].max(self.percussive[2])
    }

    fn hit(&mut self, late_samples: u64) {
        self.since = Some(late_samples);
        self.count += 1;
    }
}

/// The median of the local peaks in `past` (circular, `write` the oldest),
/// using `scratch` to sort them.
fn typical_peak(past: &[f32], write: usize, scratch: &mut [f32]) -> f32 {
    let n = past.len();
    let mut peaks = 0;
    for k in 1..n - 1 {
        let [a, b, c] = [k - 1, k, k + 1].map(|i| past[(write + i) % n]);
        if b > a && b >= c {
            scratch[peaks] = b;
            peaks += 1;
        }
    }
    if peaks == 0 { 0.0 } else { *scratch[..peaks].select_nth_unstable_by(peaks / 2, f32::total_cmp).1 }
}

pub struct Percussion {
    sample_rate: f32,
    hop: usize,
    bands: Vec<Bins>,
    /// What a band's rise is multiplied by to express it in deviations of
    /// steady noise: `sqrt(bins) / NOISE_DB`.
    scales: Vec<f32>,
    /// Each band's last levels, `usual_hops` per band, circular.
    past: Vec<f32>,
    usual_hops: usize,
    write: usize,
    /// A copy of one band's past levels to find the median in.
    sorted: Vec<f32>,
    /// Band levels of the last `LAG` hops, newest first.
    previous: [Vec<f32>; LAG],
    /// This hop's level, usual level, rise (in decibels) and percussive and
    /// harmonic power, per band.
    levels: Vec<f32>,
    usual: Vec<f32>,
    rises: Vec<f32>,
    percussive_bands: Vec<f32>,
    harmonic_bands: Vec<f32>,
    regions: [Region; REGIONS],
    /// Hops left during which the band histories are refilled with the
    /// current level: sound after silence takes a full window to arrive.
    settling: usize,
    settle_hops: usize,
    /// The percussive power, held for a moment after each hit, and the
    /// harmonic power, as mean squares.
    percussive: f32,
    harmonic: f32,
}

impl Percussion {
    /// For `onset`'s transform: `size` samples, `hop` apart.
    pub(crate) fn new(sample_rate: f32, size: usize, hop: usize) -> Self {
        let hop_seconds = hop as f32 / sample_rate;
        let past_len = (THRESHOLD_SECONDS / hop_seconds).round() as usize;
        let usual_hops = ((USUAL_SECONDS / hop_seconds).round() as usize) | 1;
        let mut bands = Vec::new();
        let regions = std::array::from_fn(|r| {
            let (hz, count, threshold_rule) = REGION_SETUP[r];
            let first = bands.len();
            bands.extend(merged(log_bands(hz, count, sample_rate, size)));
            Region {
                first,
                end: bands.len(),
                threshold_rule,
                past: vec![0.0; past_len],
                write: 0,
                sorted: vec![0.0; past_len],
                recent: [0.0; 3],
                earlier: 0.0,
                rise_db: [0.0; 3],
                percussive: [0.0; 3],
                harmonic: 0.0,
                threshold: 0.0,
                since: None,
                count: 0,
            }
        });
        for (hz, count) in OTHER_BANDS {
            bands.extend(merged(log_bands(hz, count, sample_rate, size)));
        }
        let n = bands.len();
        Self {
            sample_rate,
            hop,
            scales: bands.iter().map(|b| ((b.end - b.start) as f32).sqrt() / NOISE_DB).collect(),
            bands,
            past: vec![SILENCE_DB; n * usual_hops],
            usual_hops,
            write: 0,
            sorted: vec![0.0; usual_hops],
            previous: std::array::from_fn(|_| vec![SILENCE_DB; n]),
            levels: vec![0.0; n],
            usual: vec![0.0; n],
            rises: vec![0.0; n],
            percussive_bands: vec![0.0; n],
            harmonic_bands: vec![0.0; n],
            regions,
            settling: 0,
            settle_hops: size.div_ceil(hop),
            percussive: 0.0,
            harmonic: 0.0,
        }
    }

    /// Counts `samples` more since the last hits.
    pub(crate) fn advance(&mut self, samples: u64) {
        for since in self.regions.iter_mut().filter_map(|r| r.since.as_mut()) {
            *since += samples;
        }
    }

    /// Analyzes one hop: `spectrum` is its transform, `power_scale` turns a
    /// band's power sum into its mean square, band levels are clamped at
    /// `floor_db` (see `onset`), and `waking` is set on the first hop of
    /// sound after silence.
    pub(crate) fn hop(&mut self, spectrum: &[Complex], power_scale: f32, floor_db: f32, waking: bool) {
        if waking {
            self.settling = self.settle_hops;
        }
        let hops = self.usual_hops;
        for (i, bins) in self.bands.iter().enumerate() {
            let power = spectrum[bins.start..bins.end].iter().map(|x| x.norm_sqr()).sum::<f32>() * power_scale;
            let level = (10.0 * power.max(1e-12).log10()).max(floor_db);
            let past = &mut self.past[i * hops..(i + 1) * hops];
            // Sound after silence is the new usual, not a hit.
            if self.settling > 0 {
                past.fill(level);
                self.previous.iter_mut().for_each(|p| p[i] = level);
            }
            self.sorted.copy_from_slice(past);
            let usual = *self.sorted.select_nth_unstable_by(hops / 2, f32::total_cmp).1;
            past[self.write] = level;
            // The part above the usual level is percussive.
            let excess = (level - usual).max(0.0);
            let percussive = power * (1.0 - 10f32.powf(-excess / 10.0));
            self.percussive_bands[i] = percussive;
            self.harmonic_bands[i] = power - percussive;
            // A rise counts from the usual level up.
            self.rises[i] = (level - self.previous[LAG - 1][i].max(usual)).max(0.0);
            self.levels[i] = level;
            self.usual[i] = usual;
        }
        self.write = (self.write + 1) % hops;
        self.settling = self.settling.saturating_sub(1);
        for i in 0..self.bands.len() {
            for k in (1..LAG).rev() {
                self.previous[k][i] = self.previous[k - 1][i];
            }
            self.previous[0][i] = self.levels[i];
        }

        let hop_seconds = self.hop as f32 / self.sample_rate;
        let refractory = ((REFRACTORY - hop_seconds).max(0.0) * self.sample_rate) as u64;
        let mut candidates = [false; REGIONS];
        for (r, region) in self.regions.iter_mut().enumerate() {
            let bands = region.first..region.end;
            let n = bands.len() as f32;
            let flux = bands.clone().map(|i| self.rises[i] * self.scales[i]).sum::<f32>() / n;
            let rise_db = bands.clone().map(|i| self.rises[i]).sum::<f32>() / n;
            let percussive = self.percussive_bands[bands.clone()].iter().sum::<f32>();
            region.rise_db = [region.rise_db[1], region.rise_db[2], rise_db];
            region.percussive = [region.percussive[1], region.percussive[2], percussive];
            let harmonic = self.harmonic_bands[bands].iter().sum::<f32>();
            region.harmonic += (harmonic - region.harmonic) * (1.0 - (-hop_seconds / TILT_SECONDS).exp());
            candidates[r] = region.hop(flux, refractory);
        }

        // The hits happened one hop ago.
        let late = self.hop as u64;
        let accepted = [self.kick(candidates[KICK], refractory), self.snare(candidates[SNARE]), self.cymbal(candidates[CYMBALS])];
        for (d, accepted) in accepted.into_iter().enumerate() {
            if accepted {
                self.regions[d].hit(late);
            }
        }

        let held = self.percussive * (-hop_seconds / PERCUSSIVE_HOLD).exp();
        self.percussive = self.percussive_bands.iter().sum::<f32>().max(held);
        self.harmonic = self.harmonic_bands.iter().sum::<f32>();
    }

    /// Whether the previous hop is a kick, given whether its region peaked.
    fn kick(&self, candidate: bool, refractory: u64) -> bool {
        let [kick, snare, cymbals, click] = [KICK, SNARE, CYMBALS, CLICK].map(|r| &self.regions[r]);
        if candidate {
            let spill = snare.largest_rise_db() * KICK_OVER_SNARE > kick.rise_db[1];
            let flux = kick.recent[1];
            let heard = click.recent.iter().chain([&click.earlier]).copied().fold(0.0, f32::max);
            let clicked = heard >= KICK_CLICK * click.threshold;
            return !spill && (clicked || flux >= KICK_ALONE);
        }
        // A click on its own.
        let (rise, snare_share, cymbal_share) = CLICK_KICK;
        let [before, at, after] = click.rise_db;
        let rested = kick.since.is_none_or(|since| since >= refractory);
        at > before && at >= after && at >= rise && snare.largest_rise_db() < snare_share * at && cymbals.rise_db[1] < cymbal_share * at && rested
    }

    /// Whether the previous hop is a snare, given whether its region peaked.
    fn snare(&self, candidate: bool) -> bool {
        let [kick, snare] = [KICK, SNARE].map(|r| &self.regions[r]);
        candidate && kick.largest_rise_db() * SNARE_OVER_KICK <= snare.rise_db[1]
    }

    /// Whether the previous hop is a cymbal, given whether its region peaked.
    fn cymbal(&self, candidate: bool) -> bool {
        let [click, bright] = [CLICK, BRIGHT].map(|r| &self.regions[r]);
        candidate && bright.percussive_peak() * click.harmonic >= BRIGHTNESS * click.percussive_peak() * bright.harmonic
    }

    /// Each drum's event: 1 at a hit, falling back to 0. Kick, snare,
    /// cymbals.
    pub fn events(&self) -> [f32; DRUMS] {
        std::array::from_fn(|d| {
            self.regions[d].since.map_or(0.0, |since| (-(since as f32) / self.sample_rate / DECAY[d]).exp())
        })
    }

    /// Hits found so far by each drum.
    pub fn counts(&self) -> [u64; DRUMS] {
        std::array::from_fn(|d| self.regions[d].count)
    }

    /// The percussive power (held for a moment after each hit) and the
    /// harmonic power, as mean squares.
    pub fn powers(&self) -> (f32, f32) {
        (self.percussive, self.harmonic)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::onset::Onsets;
    use crate::onset::tests_support::*;

    /// What a signal made of the detector, hop by hop.
    struct Heard {
        /// Each drum's hit times (when reported, minus the hops they are
        /// late by).
        hits: [Vec<f32>; DRUMS],
        /// Each drum's event after every hop.
        events: [Vec<f32>; DRUMS],
        /// The percussive and harmonic powers after every hop.
        powers: Vec<(f32, f32)>,
    }

    fn listen(signal: &[f32]) -> Heard {
        let mut detector = Onsets::new(RATE);
        let hop = (detector.hop_seconds() * RATE as f32).round() as usize;
        let mut heard = Heard { hits: Default::default(), events: Default::default(), powers: Vec::new() };
        for (n, chunk) in signal.chunks(hop).enumerate() {
            let before = detector.percussion().counts();
            detector.push(chunk);
            let percussion = detector.percussion();
            let (after, events) = (percussion.counts(), percussion.events());
            for d in 0..DRUMS {
                if after[d] > before[d] {
                    heard.hits[d].push(((n + 1) * hop) as f32 / RATE as f32 - 2.0 * detector.hop_seconds());
                }
                heard.events[d].push(events[d]);
            }
            heard.powers.push(percussion.powers());
        }
        heard
    }

    /// How many of `expected` have a hit within 30 ms, and how many hits
    /// match nothing.
    fn score(found: &[f32], expected: &[f32]) -> (usize, usize) {
        let near = |a: f32, b: f32| (a - b).abs() <= 0.03;
        let hit = expected.iter().filter(|&&t| found.iter().any(|&f| near(f, t))).count();
        let extra = found.iter().filter(|&&f| !expected.iter().any(|&t| near(f, t))).count();
        (hit, extra)
    }

    fn mix(a: &mut [f32], b: &[f32]) {
        for (x, y) in a.iter_mut().zip(b) {
            *x += y;
        }
    }

    /// 120 per minute: kick on 1 and 3, snare on 2 and 4, hi-hats on the
    /// eighths between, every hit a quarter second from the next. Kicks,
    /// snares, hi-hats and the 8 s signal over faint noise.
    fn pattern() -> [Vec<f32>; 4] {
        let kicks = tempo(60.0, 8.0, 0.5);
        let snares: Vec<f32> = kicks.iter().map(|t| t + 0.5).filter(|&t| t < 7.9).collect();
        let hats = tempo(120.0, 8.0, 0.75);
        let mut signal = clicks(8.0, &[], 0.002);
        add_kick_drums(&mut signal, &kicks, 0.5, 0.06);
        add_snares(&mut signal, &snares, 0.8);
        add_hats(&mut signal, &hats, 0.2);
        [kicks, snares, hats, signal]
    }

    #[test]
    fn each_drum_fires_only_its_own_event() {
        let [kicks, snares, hats, signal] = pattern();
        let heard = listen(&signal);
        for (d, expected) in [kicks, snares, hats].iter().enumerate() {
            let (hit, extra) = score(&heard.hits[d], expected);
            assert_eq!((hit, extra), (expected.len(), 0), "drum {d}: {:?}", heard.hits[d]);
        }
    }

    #[test]
    fn guitar_attacks_are_not_snares_and_raise_the_harmonic_energy() {
        // Sixteen picks a second on a distorted chord, alone: 128 attacks.
        let guitar = distorted_guitar(8.0, 16.0, 0.3);
        let alone = listen(&guitar);
        assert!(alone.hits[SNARE].len() <= 2, "snares {:?}", alone.hits[SNARE]);

        // Under the drum pattern, mastered loud.
        let [kicks, snares, hats, drums] = pattern();
        let mut heavy = guitar.clone();
        mix(&mut heavy, &drums);
        limit(&mut heavy, 1.5);
        let heard = listen(&heavy);
        let (hit, extra) = score(&heard.hits[SNARE], &snares);
        assert!(hit >= snares.len() - 1 && extra <= 1, "snares {hit}/{} +{extra}", snares.len());
        let (hit, _) = score(&heard.hits[KICK], &kicks);
        assert_eq!(hit, kicks.len());
        let (hit, extra) = score(&heard.hits[CYMBALS], &hats);
        assert!(hit == hats.len() && extra <= 6, "cymbals {hit}/{} +{extra}", hats.len());

        // Drums alone are percussive. The guitar is mostly harmonic, though
        // not all: each pick is a small attack. Adding it to the drums
        // raises the harmonic energy and its share.
        let mean = |heard: &Heard| {
            let (p, h) = heard.powers[100..].iter().fold((0.0, 0.0), |(p, h), &(a, b)| (p + a, h + b));
            (p, h, h / (p + h))
        };
        let [drums_only, with_guitar, guitar_only] = [&listen(&drums), &heard, &alone].map(mean);
        assert!(drums_only.2 < 0.1, "harmonic share of the drums {}", drums_only.2);
        assert!(guitar_only.2 > 0.55, "harmonic share of the guitar {}", guitar_only.2);
        assert!(with_guitar.1 > 10.0 * drums_only.1 && with_guitar.2 > drums_only.2 + 0.4, "{drums_only:?} {with_guitar:?}");
    }

    #[test]
    fn follows_a_double_kick_at_17_per_second() {
        let kicks = tempo(60.0 * 17.0, 6.0, 0.5);
        let mut signal = distorted_guitar(6.0, 16.0, 0.3);
        add_kick_drums(&mut signal, &kicks, 0.5, 0.04);
        limit(&mut signal, 1.5);
        let heard = listen(&signal);
        let (hit, extra) = score(&heard.hits[KICK], &kicks);
        assert!(hit >= kicks.len() * 9 / 10 && extra == 0, "kicks {hit}/{} +{extra}", kicks.len());
        // Between two kicks found as far apart as they were played (59 ms),
        // the event falls below a third.
        let hop = 512.0 / RATE as f32;
        let found = &heard.hits[KICK];
        let pairs: Vec<&[f32]> = found.windows(2).filter(|p| (0.05..0.1).contains(&(p[1] - p[0]))).collect();
        assert!(pairs.len() >= kicks.len() * 2 / 3, "{} pairs", pairs.len());
        for pair in pairs {
            let [a, b] = [pair[0], pair[1]].map(|t| ((t + 2.0 * hop) / hop).round() as usize);
            let lowest = heard.events[KICK][a..b].iter().copied().fold(1.0, f32::min);
            assert!(lowest < 0.33, "kick event stayed at {lowest} between {} and {}", pair[0], pair[1]);
        }
        assert!(heard.hits[SNARE].len() <= 2, "snares {:?}", heard.hits[SNARE]);
    }

    #[test]
    fn hears_a_blast_beat_under_guitars() {
        // 220 per minute: kick and snare alternating on the sixteenths, a
        // hi-hat with every kick.
        let sixteenth = 60.0 / 220.0 / 4.0;
        let kicks: Vec<f32> = (0..).map(|i| 0.5 + 2.0 * i as f32 * sixteenth).take_while(|&t| t < 5.9).collect();
        let snares: Vec<f32> = kicks.iter().map(|t| t + sixteenth).collect();
        let mut signal = distorted_guitar(6.0, 16.0, 0.3);
        add_kick_drums(&mut signal, &kicks, 0.5, 0.04);
        add_snares(&mut signal, &snares, 0.8);
        add_hats(&mut signal, &kicks, 0.2);
        limit(&mut signal, 1.5);
        let heard = listen(&signal);
        let (hit, extra) = score(&heard.hits[SNARE], &snares);
        assert!(hit >= snares.len() * 9 / 10 && extra <= 1, "snares {hit}/{} +{extra}", snares.len());
        let (hit, extra) = score(&heard.hits[CYMBALS], &kicks);
        assert!(hit >= kicks.len() * 9 / 10 && extra <= 3, "cymbals {hit}/{} +{extra}", kicks.len());
        // The kicks, between snares, under hi-hats and the bass, are the
        // hardest part: only some are found, and some snares pass for kicks.
        let (hit, _) = score(&heard.hits[KICK], &kicks);
        let on_snares = score(&heard.hits[KICK], &snares).0;
        assert!(hit >= kicks.len() / 4, "kicks {hit}/{}", kicks.len());
        assert!(on_snares <= snares.len() / 4, "{on_snares} snares taken for kicks");
    }

    #[test]
    fn silence_and_white_noise_make_no_hits() {
        let mut signal = vec![0.0; seconds(2.0)];
        signal.extend(clicks(30.0, &[], 0.3));
        let heard = listen(&signal);
        assert_eq!(heard.hits, <[Vec<f32>; DRUMS]>::default());
    }
}
