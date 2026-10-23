//! Onsets and novelty: when a sound starts, and when the music changes.
//!
//! **Onsets.** Every 10.7 ms (a *hop*), take the newest 21 ms of audio and
//! find the level of about 18 log-spaced bands in decibels. In each band,
//! see how much it got *louder* than two hops ago; bands that got quieter
//! count as zero. Averaged over a group of bands, this is the **spectral
//! flux**: it jumps when a drum is hit or a note starts, because energy
//! appears where there was none a moment ago, and stays low while sounds
//! hold or fade.
//!
//! Before averaging, each band's rise is divided by how much that band jumps
//! around in steady noise: 6 dB for a band one FFT bin wide, less for wider
//! bands (6 dB divided by the square root of its bins). A narrow band is a
//! shaky measurement and a wide one a steady one; in these units, noise
//! looks the same in every band, and one threshold fits them all.
//!
//! The flux is computed separately for three regions, the same split as the
//! bass, mid and treble levels: low (kick drums, bass notes), middle
//! (snares, voices, chords) and high (hi-hats, cymbals). Averaged over the
//! whole spectrum, a kick drum fills only the lowest bands and gets diluted
//! by everything else; in its own region it stands out.
//!
//! A hop is an onset in a region when the region's flux is a local peak
//! (higher than the hop before and no lower than the hop after) and clears
//! an **adaptive threshold**: the mean flux of the last second plus
//! `THRESHOLD_SPREAD` times its typical deviation, and never less than a
//! fixed floor. The threshold rises in busy music and falls in quiet music,
//! so the same detector works on a solo piano and on a wall of drums.
//! Confirming a peak needs the next hop, so onsets are reported 10.7 ms
//! late, and a region can't fire again within 100 ms.
//!
//! **Beat.** `beat` is 1 at every onset in the low or middle region (kicks
//! and snares, what you tap your foot to) and falls back to 0 with a time
//! constant of 0.15 s. High-region onsets (hi-hats, often four per beat) are
//! counted but don't make beats. The tempo is counted from the same flux
//! (`tempo`).
//!
//! **Novelty.** The same band levels, averaged over the last half second and
//! over the last four seconds. While the music keeps its character, the two
//! averages agree; at a drop or a new section the short one moves first, and
//! their difference (in decibels, across all bands) is the novelty, mapped to
//! `0..=1`. It fades as the long average catches up.
//!
//! Everything runs per hop, at a fixed rate set by the audio, so the results
//! don't depend on the frame rate or on how samples are batched.

use crate::analyzer::{Bins, log_bands};
use crate::fft::{Complex, Fft};
use crate::percussion::Percussion;
use crate::tempo::Tempo;
use crate::window::hann;

/// The three regions, in Hz (the same split as `Analyzer`'s bass, mid and
/// treble), and how many log-spaced bands each is divided into before
/// merging.
const REGION_BANDS: [((f32, f32), usize); REGIONS] = [((40.0, 250.0), 4), ((250.0, 4_000.0), 12), ((4_000.0, 16_000.0), 8)];
/// Fewest FFT bins in a band. A band of one or two bins jumps by several
/// decibels from one hop to the next even in steady noise; the lowest bands
/// are merged until they have this many.
const MIN_BINS: usize = 4;
/// Band levels are clamped this far below the loudest band's recent level,
/// so the detector works the same at any volume and sounds far quieter than
/// the music can't produce onsets out of nothing.
const RANGE_DB: f32 = 45.0;
/// And never below this, which is silence.
const SILENCE_DB: f32 = -100.0;
/// How fast the loudest band's level is forgotten, in decibels per second
/// (as the analyzer's automatic gain does).
const PEAK_FALL_DB_PER_S: f32 = 3.0;
/// How much the level of a one-bin band jumps around in steady noise, in
/// decibels (one standard deviation). A band of `n` bins jumps by this
/// divided by `sqrt(n)`: measured, and as the statistics of noise predict.
const NOISE_DB: f32 = 6.0;
/// Seconds of past flux the threshold is computed from.
const THRESHOLD_SECONDS: f32 = 1.0;
/// How many typical deviations above the mean flux an onset must reach.
const THRESHOLD_SPREAD: f32 = 3.0;
/// In steady noise, a band's rise (in deviations of noise, see `NOISE_DB`)
/// averages this, with this deviation. Averaged over `n` bands, the
/// deviation shrinks by `sqrt(n)`.
const NOISE_RISE: (f32, f32) = (0.56, 0.83);
/// A region's threshold never drops below the noise average plus this many
/// of its deviations: in 30 s of loud white noise, no onset at all.
const THRESHOLD_FLOOR: f32 = 4.8;
/// Each band's level is compared with its level this many hops ago. One hop
/// back, the two windows overlap by half and an attack is split between
/// them; two hops back, they don't overlap.
const LAG: usize = 2;
/// The shortest gap between two onsets in one region, in seconds.
const REFRACTORY: f32 = 0.1;
/// How fast `beat` falls back to 0 after an onset, in seconds.
const BEAT_DECAY: f32 = 0.15;
/// Time constants of the short and long averages behind novelty, seconds.
const NOVELTY_SHORT: f32 = 0.5;
const NOVELTY_LONG: f32 = 4.0;
/// Difference between the two averages, in decibels (root mean square over
/// the bands), that maps to novelty 0 and 1. Measured: three 40-second
/// stretches of real songs stayed under 1.5 dB nine tenths of the time and
/// never passed 3.4; a synthetic drop reaches 4.6 dB after one second.
const NOVELTY_DB: (f32, f32) = (2.5, 6.0);
/// Novelty only looks at bands within this many decibels of the loudest
/// (the range the band meters show, see `Analyzer`): a quiet band's level
/// swings by many decibels without anyone hearing a change.
const NOVELTY_RANGE_DB: f32 = 30.0;

/// The three regions onsets are found in: low, middle, high.
pub const REGIONS: usize = 3;
/// Regions whose onsets are beats: low and middle.
const BEAT_REGIONS: [bool; REGIONS] = [true, true, false];
/// How much each region's flux counts toward the tempo: hi-hats help count
/// the beat but shouldn't decide where it falls.
const TEMPO_WEIGHTS: [f32; REGIONS] = [1.0, 1.0, 0.5];

/// One region's flux and its peak picking.
struct Region {
    /// Its bands, as a range of indices into `Onsets::bands`.
    first: usize,
    end: usize,
    /// Past flux values, circular, for the threshold.
    past: Vec<f32>,
    write: usize,
    /// Flux of the last three hops, oldest first.
    recent: [f32; 3],
    threshold: f32,
    /// The lowest the threshold may go, for this region's number of bands.
    floor: f32,
    /// Samples since this region's last onset.
    since: Option<u64>,
    count: u64,
}

impl Region {
    /// Takes this hop's flux; returns whether the previous hop was an onset.
    fn hop(&mut self, flux: f32, refractory_samples: u64) -> bool {
        let n = self.past.len() as f32;
        let mean = self.past.iter().sum::<f32>() / n;
        let deviation = self.past.iter().map(|f| (f - mean).abs()).sum::<f32>() / n;
        self.threshold = (mean + THRESHOLD_SPREAD * deviation).max(self.floor);
        self.past[self.write] = flux;
        self.write = (self.write + 1) % self.past.len();

        self.recent = [self.recent[1], self.recent[2], flux];
        let [before, candidate, after] = self.recent;
        let rested = self.since.is_none_or(|since| since >= refractory_samples);
        candidate > before && candidate >= after && candidate > self.threshold && rested
    }
}

pub struct Onsets {
    sample_rate: f32,
    hop: usize,
    fft: Fft,
    window: Vec<f32>,
    /// Turns a band's power sum into its mean square (as in `Analyzer`).
    power_scale: f32,
    /// The newest `window.len()` samples, circular; `write` is the oldest.
    history: Vec<f32>,
    write: usize,
    /// Samples since the last hop.
    since_hop: usize,
    scratch: Vec<Complex>,
    bands: Vec<Bins>,
    /// Band levels of the last `LAG` hops, newest first.
    previous: [Vec<f32>; LAG],
    /// What a band's rise is multiplied by to express it in deviations of
    /// steady noise: `sqrt(bins) / NOISE_DB`.
    scales: Vec<f32>,
    rises: Vec<f32>,
    short: Vec<f32>,
    long: Vec<f32>,
    regions: [Region; REGIONS],
    /// Samples since the last beat (`None` before the first).
    since_beat: Option<u64>,
    beat_count: u64,
    /// The loudest band's level, following rises at once and falls slowly.
    peak: f32,
    /// How far apart the short and long averages are, in decibels.
    change_db: f32,
    novelty: f32,
    tempo: Tempo,
    percussion: Percussion,
}

impl Onsets {
    pub fn new(sample_rate: u32) -> Self {
        let sample_rate = sample_rate as f32;
        // About 21 ms: 1024 samples at 44.1 or 48 kHz, 2048 at 96 kHz.
        let size = ((sample_rate * 0.0213).round() as usize).max(64).next_power_of_two();
        let window = hann(size);
        let sum_sq: f32 = window.iter().map(|w| w * w).sum();
        let hop = size / 2;
        let past_len = (THRESHOLD_SECONDS * sample_rate / hop as f32).round() as usize;
        // Each region's bands, merged within the region, one after another.
        let mut bands = Vec::new();
        let regions = std::array::from_fn(|r| {
            let (hz, count) = REGION_BANDS[r];
            let first = bands.len();
            bands.extend(merged(log_bands(hz, count, sample_rate, size)));
            let end = bands.len();
            Region {
                first,
                end,
                past: vec![0.0; past_len],
                write: 0,
                recent: [0.0; 3],
                threshold: 0.0,
                floor: NOISE_RISE.0 + THRESHOLD_FLOOR * NOISE_RISE.1 / ((end - first) as f32).sqrt(),
                since: None,
                count: 0,
            }
        });
        let n = bands.len();
        let scales = bands.iter().map(|b| ((b.end - b.start) as f32).sqrt() / NOISE_DB).collect();
        Self {
            sample_rate,
            hop,
            fft: Fft::new(size),
            window,
            power_scale: 2.0 / (size as f32 * sum_sq),
            history: vec![0.0; size],
            write: 0,
            since_hop: 0,
            scratch: vec![Complex::ZERO; size],
            bands,
            previous: std::array::from_fn(|_| vec![SILENCE_DB; n]),
            scales,
            rises: vec![0.0; n],
            short: vec![SILENCE_DB; n],
            long: vec![SILENCE_DB; n],
            regions,
            since_beat: None,
            beat_count: 0,
            peak: SILENCE_DB,
            change_db: 0.0,
            novelty: 0.0,
            tempo: Tempo::new(hop as f32 / sample_rate),
            percussion: Percussion::new(sample_rate, size, hop),
        }
    }

    /// Feeds mono samples, analyzing every complete hop.
    pub fn push(&mut self, samples: &[f32]) {
        for &sample in samples {
            self.history[self.write] = sample;
            self.write = (self.write + 1) % self.history.len();
            for since in self.regions.iter_mut().filter_map(|r| r.since.as_mut()).chain(self.since_beat.as_mut()) {
                *since += 1;
            }
            self.percussion.advance(1);
            self.since_hop += 1;
            if self.since_hop == self.hop {
                self.since_hop = 0;
                self.analyze_hop();
            }
        }
    }

    /// Feeds `count` samples of silence, without needing a buffer.
    pub fn push_silence(&mut self, count: usize) {
        for _ in 0..count {
            self.push(&[0.0]);
        }
    }

    /// 1 at a beat, falling back to 0.
    pub fn beat(&self) -> f32 {
        self.since_beat.map_or(0.0, |since| (-(since as f32) / self.sample_rate / BEAT_DECAY).exp())
    }

    /// How much the music is changing character, `0..=1`.
    pub fn novelty(&self) -> f32 {
        self.novelty
    }

    /// How far apart the short and long averages of the band levels are, in
    /// decibels: what novelty is made of.
    pub fn change_db(&self) -> f32 {
        self.change_db
    }

    /// Onsets found so far in each region: low, middle, high.
    pub fn counts(&self) -> [u64; REGIONS] {
        std::array::from_fn(|r| self.regions[r].count)
    }

    /// Beats found so far (onsets in the low and middle regions, at most
    /// one per refractory time).
    pub fn beats(&self) -> u64 {
        self.beat_count
    }

    /// Each region's newest flux and the threshold it was compared with, in
    /// deviations of steady noise: for drawing what the detector sees.
    pub fn flux(&self) -> [(f32, f32); REGIONS] {
        std::array::from_fn(|r| (self.regions[r].recent[2], self.regions[r].threshold))
    }

    /// The tempo and where the beat is (see `tempo`).
    pub fn tempo(&self) -> &Tempo {
        &self.tempo
    }

    /// The drums and the harmonic/percussive split (see `percussion`).
    pub fn percussion(&self) -> &Percussion {
        &self.percussion
    }

    /// Seconds between hops.
    pub fn hop_seconds(&self) -> f32 {
        self.hop as f32 / self.sample_rate
    }

    fn analyze_hop(&mut self) {
        // Unroll the circular history oldest-first, windowed, and transform.
        let (newest, oldest) = self.history.split_at(self.write);
        for ((slot, &sample), &w) in self.scratch.iter_mut().zip(oldest.iter().chain(newest)).zip(&self.window) {
            *slot = Complex::new(sample * w, 0.0);
        }
        self.fft.process(&mut self.scratch);

        let hop_seconds = self.hop_seconds();
        let short = 1.0 - (-hop_seconds / NOVELTY_SHORT).exp();
        let long = 1.0 - (-hop_seconds / NOVELTY_LONG).exp();
        // Band levels first (kept in `rises` for a moment), then the peak.
        for (i, bins) in self.bands.iter().enumerate() {
            let power: f32 = self.scratch[bins.start..bins.end].iter().map(|x| x.norm_sqr()).sum();
            self.rises[i] = 10.0 * (power * self.power_scale).max(1e-12).log10();
        }
        let loudest = self.rises.iter().copied().fold(SILENCE_DB, f32::max);
        // Sound after real silence: the averages start from it, so music
        // starting doesn't look like the music changing.
        let waking = self.peak <= SILENCE_DB && loudest > SILENCE_DB;
        self.peak = loudest.max(self.peak - PEAK_FALL_DB_PER_S * hop_seconds);
        let floor = (self.peak - RANGE_DB).max(SILENCE_DB);
        self.percussion.hop(&self.scratch, self.power_scale, floor, waking);
        let audible = (self.peak - NOVELTY_RANGE_DB).max(SILENCE_DB);
        let mut difference = 0.0;
        for i in 0..self.bands.len() {
            let level = self.rises[i].max(floor);
            self.rises[i] = (level - self.previous[LAG - 1][i]).max(0.0) * self.scales[i];
            for k in (1..LAG).rev() {
                self.previous[k][i] = self.previous[k - 1][i];
            }
            self.previous[0][i] = level;
            if waking {
                self.short[i] = level;
                self.long[i] = level;
            }
            self.short[i] += (level - self.short[i]) * short;
            self.long[i] += (level - self.long[i]) * long;
            // Novelty compares only what can be heard: averages below the
            // audible range count as its edge.
            difference += (self.short[i].max(audible) - self.long[i].max(audible)).powi(2);
        }
        self.change_db = (difference / self.bands.len() as f32).sqrt();
        self.novelty = smoothstep(NOVELTY_DB.0, NOVELTY_DB.1, self.change_db);

        // The refractory time, less the hop the onset is reported late by.
        let refractory = ((REFRACTORY - hop_seconds) * self.sample_rate) as u64;
        let mut beat = false;
        let mut strength = 0.0;
        for ((region, is_beat), weight) in self.regions.iter_mut().zip(BEAT_REGIONS).zip(TEMPO_WEIGHTS) {
            let rises = &self.rises[region.first..region.end];
            let flux = if rises.is_empty() { 0.0 } else { rises.iter().sum::<f32>() / rises.len() as f32 };
            strength += weight * flux;
            if region.hop(flux, refractory) {
                // The onset happened one hop ago.
                region.since = Some(self.hop as u64);
                region.count += 1;
                beat |= is_beat;
            }
        }
        self.tempo.hop(strength);
        // A kick that also reaches the middle region is one beat, not two.
        if beat && self.since_beat.is_none_or(|since| since >= refractory) {
            self.since_beat = Some(self.hop as u64);
            self.beat_count += 1;
        }
    }
}

/// Merges neighboring bands, from the lowest up, until each has at least
/// `MIN_BINS` bins. Bands that ended up with the same bins merge too, so no
/// bin counts twice.
pub(crate) fn merged(bands: Vec<Bins>) -> Vec<Bins> {
    let mut out: Vec<Bins> = Vec::new();
    for band in bands {
        match out.last_mut() {
            Some(last) if last.end - last.start < MIN_BINS => last.end = last.end.max(band.end),
            _ => out.push(band),
        }
    }
    // A short band left at the top joins the one below it.
    if out.len() > 1 && out[out.len() - 1].end - out[out.len() - 1].start < MIN_BINS {
        let last = out.pop().unwrap();
        out.last_mut().unwrap().end = last.end;
    }
    out
}

fn smoothstep(low: f32, high: f32, x: f32) -> f32 {
    let t = ((x - low) / (high - low)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

#[cfg(test)]
pub(crate) mod tests_support {
    use super::*;
    use std::f32::consts::TAU;

    pub const RATE: u32 = 48_000;

    /// White noise in `-1..1`, from a seeded xorshift.
    pub struct Noise(pub u64);

    impl Noise {
        pub fn next(&mut self) -> f32 {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 7;
            self.0 ^= self.0 << 17;
            (self.0 >> 40) as f32 / (1u64 << 23) as f32 - 1.0
        }
    }

    pub fn seconds(s: f32) -> usize {
        (s * RATE as f32) as usize
    }

    /// `length` seconds of noise of amplitude `floor`, plus a click (a 5 ms
    /// burst of decaying noise) at every time in `clicks`.
    pub fn clicks(length: f32, clicks: &[f32], floor: f32) -> Vec<f32> {
        let mut noise = Noise(7);
        let mut out: Vec<f32> = (0..seconds(length)).map(|_| floor * noise.next()).collect();
        for &t in clicks {
            for k in 0..seconds(0.005) {
                if let Some(s) = out.get_mut(seconds(t) + k) {
                    *s += 0.8 * noise.next() * (-(k as f32) / seconds(0.001) as f32).exp();
                }
            }
        }
        out
    }

    /// Adds kick drums (a 60 Hz tone fading over 0.1 s) of amplitude
    /// `level` at every time in `kicks`.
    pub fn add_kicks(signal: &mut [f32], kicks: &[f32], level: f32) {
        for &t in kicks {
            for k in 0..seconds(0.4) {
                let time = k as f32 / RATE as f32;
                if let Some(s) = signal.get_mut(seconds(t) + k) {
                    *s += level * (TAU * 60.0 * time).sin() * (-time / 0.1).exp();
                }
            }
        }
    }

    /// Feeds `signal` in blocks of `block` samples, returning the time of
    /// every beat (when it was reported, minus the hop it is late by).
    pub fn beats(signal: &[f32], block: usize) -> Vec<f32> {
        let mut detector = Onsets::new(RATE);
        let mut times = Vec::new();
        let mut done = 0;
        for chunk in signal.chunks(block) {
            for (i, &s) in chunk.iter().enumerate() {
                let before = detector.beats();
                detector.push(&[s]);
                if detector.beats() > before {
                    times.push((done + i) as f32 / RATE as f32 - detector.hop_seconds());
                }
            }
            done += chunk.len();
        }
        times
    }

    /// Adds hi-hats (short bursts of noise with the lows taken out: three
    /// first differences in a row) of amplitude `level` at every time in
    /// `hats`.
    pub fn add_hats(signal: &mut [f32], hats: &[f32], level: f32) {
        let mut noise = Noise(3);
        for &t in hats {
            let mut previous = [0.0; 3];
            for k in 0..seconds(0.03) {
                let mut x = noise.next();
                for p in &mut previous {
                    (x, *p) = (x - *p, x);
                }
                if let Some(s) = signal.get_mut(seconds(t) + k) {
                    *s += level * x * (-(k as f32) / seconds(0.008) as f32).exp();
                }
            }
        }
    }

    pub fn tempo(bpm: f32, length: f32, offset: f32) -> Vec<f32> {
        let period = 60.0 / bpm;
        (0..).map(|i| offset + i as f32 * period).take_while(|&t| t < length - 0.1).collect()
    }

    /// A two-pole resonant filter (a biquad band-pass) around `hz`, for
    /// shaping noise into the sound of a drum.
    pub struct Resonance {
        coefficients: [f32; 5],
        state: [f32; 4],
    }

    impl Resonance {
        pub fn new(hz: f32, q: f32) -> Self {
            let w = TAU * hz / RATE as f32;
            let alpha = w.sin() / (2.0 * q);
            let a0 = 1.0 + alpha;
            Self { coefficients: [alpha / a0, 0.0, -alpha / a0, -2.0 * w.cos() / a0, (1.0 - alpha) / a0], state: [0.0; 4] }
        }

        pub fn next(&mut self, x: f32) -> f32 {
            let [b0, b1, b2, a1, a2] = self.coefficients;
            let [x1, x2, y1, y2] = self.state;
            let y = b0 * x + b1 * x1 + b2 * x2 - a1 * y1 - a2 * y2;
            self.state = [x, x1, y, y1];
            y
        }
    }

    /// Adds kick drums as metal records them: a tone falling from 100 to
    /// 50 Hz within a few hundredths of a second and fading over `decay`
    /// seconds, with the beater's click on top (a few milliseconds of noise
    /// ringing around 4 kHz).
    pub fn add_kick_drums(signal: &mut [f32], kicks: &[f32], level: f32, decay: f32) {
        let mut noise = Noise(11);
        for &t in kicks {
            let mut phase = 0.0;
            let mut click = Resonance::new(4_000.0, 1.5);
            for k in 0..seconds(6.0 * decay) {
                let time = k as f32 / RATE as f32;
                phase += TAU * (50.0 + 50.0 * (-time / 0.01).exp()) / RATE as f32;
                let tick = click.next(noise.next() * (-time / 0.003).exp());
                if let Some(s) = signal.get_mut(seconds(t) + k) {
                    *s += level * (phase.sin() * (-time / decay).exp() + 1.5 * tick);
                }
            }
        }
    }

    /// Adds snares as they sit in a loud metal mix: a 185 Hz body fading
    /// over 0.04 s, the crack (noise around 1.2 kHz) over 0.05 s, and a
    /// little rattle of the wires around 5 kHz over 0.08 s. In real mixes a
    /// snare hit rises most below 2.5 kHz; the guitars bury the rest.
    pub fn add_snares(signal: &mut [f32], snares: &[f32], level: f32) {
        let mut noise = Noise(13);
        for &t in snares {
            let mut crack = Resonance::new(1_200.0, 0.8);
            let mut wires = Resonance::new(5_000.0, 0.7);
            for k in 0..seconds(0.4) {
                let time = k as f32 / RATE as f32;
                let body = 0.6 * (TAU * 185.0 * time).sin() * (-time / 0.04).exp();
                let snap = 0.6 * crack.next(noise.next()) * (-time / 0.05).exp();
                let rattle = 0.15 * wires.next(noise.next()) * (-time / 0.08).exp();
                if let Some(s) = signal.get_mut(seconds(t) + k) {
                    *s += level * (body + snap + rattle);
                }
            }
        }
    }

    /// A distorted guitar playing power chords (root, fifth and octave as
    /// sawtooth waves, through hard saturation) with a bass an octave
    /// below, tremolo picked `picks` times a second. Each pick strikes the
    /// strings harder, then they ring down; the saturation flattens most of
    /// that, as high gain does, leaving a small dip between picks and a
    /// scrape of pick noise on each. The bass, barely distorted, keeps
    /// more of its picking. The riff moves through four roots, one every
    /// half second. `length` seconds of amplitude `level`.
    pub fn distorted_guitar(length: f32, picks: f32, level: f32) -> Vec<f32> {
        let roots = [61.7, 73.4, 65.4, 82.4];
        let mut noise = Noise(17);
        let mut scrape = Resonance::new(3_000.0, 1.0);
        let mut phases = [0.0f32; 4];
        (0..seconds(length))
            .map(|i| {
                let time = i as f32 / RATE as f32;
                let root = roots[(time / 0.5) as usize % roots.len()];
                for (phase, ratio) in phases.iter_mut().zip([1.0, 1.5, 2.0, 0.5]) {
                    *phase = (*phase + root * ratio / RATE as f32).fract();
                }
                let saw = |p: f32| 2.0 * p - 1.0;
                let chord = saw(phases[0]) + saw(phases[1]) + saw(phases[2]);
                let since_pick = (time * picks).fract() / picks;
                let pick = (-since_pick / 0.02).exp();
                let guitar = (6.0 * (0.6 + 0.4 * pick) * chord + 0.3 * noise.next()).tanh() * (0.85 + 0.15 * pick);
                let bass = 0.6 * (3.0 * saw(phases[3])).tanh() * (0.6 + 0.4 * pick);
                let attack = scrape.next(noise.next()) * (-since_pick / 0.004).exp();
                level * (0.7 * guitar + 0.3 * bass + 0.3 * attack)
            })
            .collect()
    }

    /// A mastering limiter: the whole mix pushed `drive` times louder, and
    /// turned down at once wherever it would pass full scale, coming back
    /// up over 0.05 s. Loud metal is mastered like this, and every drum
    /// hit ducks the guitars a little.
    pub fn limit(signal: &mut [f32], drive: f32) {
        let release = (-1.0 / (0.05 * RATE as f32)).exp();
        let mut gain = 1.0f32;
        for s in signal {
            let x = *s * drive;
            let needed = if x.abs() > 0.95 { 0.95 / x.abs() } else { 1.0 };
            gain = needed.min(1.0 - (1.0 - gain) * release);
            *s = x * gain;
        }
    }

    /// How many expected times are matched by exactly one beat within
    /// `tolerance` seconds.
    pub fn matched(found: &[f32], expected: &[f32], tolerance: f32) -> usize {
        expected.iter().filter(|&&t| found.iter().filter(|&&f| (f - t).abs() <= tolerance).count() == 1).count()
    }

}

#[cfg(test)]
mod tests {
    use super::*;
    use super::tests_support::*;
    use std::f32::consts::TAU;

    #[test]
    fn clicks_at_120_bpm_over_silence_are_all_found_on_time() {
        let expected = tempo(120.0, 8.0, 0.25);
        let found = beats(&clicks(8.0, &expected, 0.0), 480);
        assert_eq!(found.len(), expected.len(), "{found:?}");
        // Within two hops of where the click starts.
        assert_eq!(matched(&found, &expected, 0.022), expected.len(), "{found:?}");
    }

    #[test]
    fn clicks_at_100_bpm_over_noise_are_found() {
        // Background noise 30 dB under the clicks' peak.
        let expected = tempo(100.0, 10.0, 0.3);
        let found = beats(&clicks(10.0, &expected, 0.025), 512);
        let hits = matched(&found, &expected, 0.022);
        assert_eq!(hits, expected.len(), "{found:?}");
        // The noise starting at 0 s is an onset too; nothing else.
        assert!(found.len() - hits <= 1, "{found:?}");
    }

    #[test]
    fn kicks_are_found_under_loud_noise() {
        // Loud broadband hiss (-10 dBFS) with kicks at 128 BPM: the kicks
        // only fill the lowest bands, and their own region finds them.
        let mut signal = clicks(10.0, &[], 0.3);
        let expected = tempo(128.0, 10.0, 0.5);
        add_kicks(&mut signal, &expected, 0.3);
        let found = beats(&signal, 512);
        let hits = matched(&found, &expected, 0.03);
        assert!(hits as f32 >= 0.9 * expected.len() as f32, "{hits} of {}: {found:?}", expected.len());
        assert!(found.len() - hits <= 2, "{found:?}");
    }

    #[test]
    fn silence_has_no_onsets_no_beat_and_no_novelty() {
        let mut detector = Onsets::new(RATE);
        detector.push(&vec![0.0; seconds(10.0)]);
        assert_eq!(detector.counts(), [0; REGIONS]);
        assert_eq!(detector.beat(), 0.0);
        assert_eq!(detector.novelty(), 0.0);
    }

    #[test]
    fn steady_noise_gives_no_beats_after_it_starts() {
        let found = beats(&clicks(30.0, &[], 0.3), 512);
        let late: Vec<_> = found.iter().filter(|&&t| t > 0.5).collect();
        assert!(late.len() <= 1, "{found:?}");
    }

    #[test]
    fn hi_hats_alone_are_onsets_but_not_beats() {
        // Hi-hats on eighths, over a quiet bed of hiss.
        let mut hats = clicks(6.0, &[], 0.01);
        add_hats(&mut hats, &tempo(240.0, 6.0, 0.2), 0.15);
        // The hiss starting at 0 s is an onset; count from just after it.
        let mut detector = Onsets::new(RATE);
        detector.push(&hats[..seconds(0.15)]);
        let start = detector.counts();
        let start_beats = detector.beats();
        detector.push(&hats[seconds(0.15)..]);
        let [low, _, high] = std::array::from_fn(|r| detector.counts()[r] - start[r]);
        assert!(high >= 20, "high-region onsets {high}");
        assert_eq!(low, 0);
        assert_eq!(detector.beats(), start_beats);
    }

    #[test]
    fn blocks_of_any_size_find_the_same_beats() {
        let signal = clicks(4.0, &tempo(132.0, 4.0, 0.1), 0.02);
        assert_eq!(beats(&signal, 37), beats(&signal, 2048));
    }

    #[test]
    fn beat_jumps_on_a_click_and_fades() {
        let mut detector = Onsets::new(RATE);
        detector.push(&clicks(0.55, &[0.5], 0.0));
        assert!(detector.beat() > 0.6, "{}", detector.beat());
        detector.push(&vec![0.0; seconds(0.5)]);
        assert!(detector.beat() < 0.05, "{}", detector.beat());
    }

    #[test]
    fn novelty_rises_when_the_music_changes_and_settles_after() {
        // Quiet hiss for 6 s, then everything louder, a bass note and kicks:
        // a drop.
        let mut detector = Onsets::new(RATE);
        detector.push(&clicks(6.0, &[], 0.02));
        let before = detector.novelty();
        let mut drop = clicks(8.0, &[], 0.1);
        add_kicks(&mut drop, &tempo(120.0, 8.0, 0.0), 0.5);
        for (i, s) in drop.iter_mut().enumerate() {
            *s += 0.3 * (TAU * 110.0 * i as f32 / RATE as f32).sin();
        }
        detector.push(&drop[..seconds(1.0)]);
        let at_drop = detector.novelty();
        detector.push(&drop[seconds(1.0)..]);
        let later = detector.novelty();
        assert!(before < 0.1, "before {before}");
        assert!(at_drop > 0.5, "at the drop {at_drop}");
        assert!(later < 0.3, "7 s later {later}");
    }
}
