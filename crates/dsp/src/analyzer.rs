//! From mono samples to the handful of numbers the visuals react to.
//!
//! Each frame: take the most recent `FFT_SIZE` samples, apply a Hann window,
//! transform, and sum bin powers into bands. Every band then goes through
//! automatic gain (so any volume uses the full `0..=1` range) and an
//! attack/release envelope (so it moves smoothly).

use crate::fft::{Complex, Fft};
use crate::level::to_dbfs;
use crate::smoothing::{AutoGain, Envelope};
use crate::window::hann;

/// Samples per analysis window. At 48 kHz this is 43 ms of audio and bins
/// 23.4 Hz apart: fine enough to separate a kick from a bass line, short
/// enough to feel instant.
pub const FFT_SIZE: usize = 2048;

/// Bands of the log-spaced spectrum, for bar displays and per-band effects.
pub const SPECTRUM_BANDS: usize = 32;

const BASS_HZ: (f32, f32) = (20.0, 250.0);
const MID_HZ: (f32, f32) = (250.0, 4_000.0);
const TREBLE_HZ: (f32, f32) = (4_000.0, 16_000.0);
const SPECTRUM_HZ: (f32, f32) = (30.0, 16_000.0);

/// Levels below this are treated as silence by the auto-gain.
const SILENCE_DB: f32 = -60.0;
/// How many decibels below the recent peak still register above zero.
const GAIN_RANGE_DB: f32 = 30.0;
/// How fast the auto-gain forgets a loud moment: 3 dB/s re-levels a song that
/// got 20 dB quieter in about seven seconds.
const GAIN_FALL_DB_PER_S: f32 = 3.0;
/// How far below the overall peak a band's own peak may sink. Within this
/// spread each band uses its full range even if the mix favors another; a
/// band further below than this (one that is nearly empty) stays low instead
/// of being amplified into noise.
const BAND_SPREAD_DB: f32 = 18.0;

/// What the analysis produces each frame. Every value is in `0..=1`.
#[derive(Clone, Debug, Default)]
pub struct Analysis {
    pub bass: f32,
    pub mid: f32,
    pub treble: f32,
    /// Overall loudness across the whole spectrum.
    pub energy: f32,
    /// Log-spaced bands from low to high. They share one auto-gain, so their
    /// relative heights keep the shape of the spectrum.
    pub spectrum: [f32; SPECTRUM_BANDS],
}

/// A half-open range of FFT bins.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Bins {
    start: usize,
    end: usize,
}

/// One analyzed band: where it is in the spectrum and how it is smoothed.
struct Band {
    bins: Bins,
    gain: AutoGain,
    envelope: Envelope,
}

impl Band {
    fn new(bins: Bins, attack: f32, release: f32) -> Self {
        Self {
            bins,
            gain: AutoGain::new(GAIN_RANGE_DB, SILENCE_DB, GAIN_FALL_DB_PER_S),
            envelope: Envelope::new(attack, release),
        }
    }
}

pub struct Analyzer {
    sample_rate: f32,
    fft: Fft,
    window: Vec<f32>,
    /// Converts a sum of bin powers into the mean square of the signal in
    /// those bins (see `Analyzer::new`).
    power_scale: f32,
    /// The last `FFT_SIZE` samples, circular; `write` is the oldest.
    history: Vec<f32>,
    write: usize,
    /// Whether samples arrived since the last `analyze`.
    fed: bool,
    scratch: Vec<Complex>,
    bass: Band,
    mid: Band,
    treble: Band,
    energy: Band,
    spectrum: Vec<Band>,
    spectrum_gain: AutoGain,
    output: Analysis,
}

impl Analyzer {
    pub fn new(sample_rate: u32) -> Self {
        let sample_rate = sample_rate as f32;
        let window = hann(FFT_SIZE);
        // Parseval: a sine of amplitude A has mean square A²/2, and after a
        // window w its positive-frequency bins hold N·(A²/4)·Σw² of power.
        // Scaling by 2/(N·Σw²) turns a band's power sum back into the mean
        // square of what is in that band.
        let sum_sq: f32 = window.iter().map(|w| w * w).sum();
        let power_scale = 2.0 / (FFT_SIZE as f32 * sum_sq);

        let bins = |hz: (f32, f32)| hz_to_bins(hz.0, hz.1, sample_rate);
        let spectrum = log_bands(SPECTRUM_HZ, SPECTRUM_BANDS, sample_rate)
            .into_iter()
            .map(|b| Band::new(b, 0.010, 0.150))
            .collect();

        Self {
            sample_rate,
            fft: Fft::new(FFT_SIZE),
            window,
            power_scale,
            history: vec![0.0; FFT_SIZE],
            write: 0,
            fed: false,
            scratch: vec![Complex::ZERO; FFT_SIZE],
            bass: Band::new(bins(BASS_HZ), 0.015, 0.250),
            mid: Band::new(bins(MID_HZ), 0.015, 0.250),
            treble: Band::new(bins(TREBLE_HZ), 0.010, 0.200),
            energy: Band::new(bins((BASS_HZ.0, TREBLE_HZ.1)), 0.030, 0.400),
            spectrum,
            spectrum_gain: AutoGain::new(GAIN_RANGE_DB, SILENCE_DB, GAIN_FALL_DB_PER_S),
            output: Analysis::default(),
        }
    }

    /// Appends mono samples to the analysis history.
    pub fn push(&mut self, samples: &[f32]) {
        if samples.is_empty() {
            return;
        }
        self.fed = true;
        // Only the newest FFT_SIZE samples can matter.
        let samples = &samples[samples.len().saturating_sub(FFT_SIZE)..];
        for &sample in samples {
            self.history[self.write] = sample;
            self.write = (self.write + 1) % FFT_SIZE;
        }
    }

    /// Analyzes the most recent window. `dt` is the time since the previous
    /// call, in seconds.
    ///
    /// WASAPI loopback sends no packets at all while nothing plays, so a call
    /// with nothing pushed since the last one means the system went quiet:
    /// the elapsed time is filled with silence instead of re-analyzing the
    /// last sound forever.
    pub fn analyze(&mut self, dt: f32) -> &Analysis {
        if !self.fed {
            let silent = ((dt * self.sample_rate) as usize).clamp(1, FFT_SIZE);
            for _ in 0..silent {
                self.history[self.write] = 0.0;
                self.write = (self.write + 1) % FFT_SIZE;
            }
        }
        self.fed = false;

        // Unroll the circular history oldest-first, windowed.
        let (newest, oldest) = self.history.split_at(self.write);
        for ((slot, &sample), &w) in self.scratch.iter_mut().zip(oldest.iter().chain(newest)).zip(&self.window) {
            *slot = Complex::new(sample * w, 0.0);
        }
        self.fft.process(&mut self.scratch);

        let scratch = &self.scratch;
        let power_scale = self.power_scale;
        let level_db = |bins: Bins| {
            let power: f32 = scratch[bins.start..bins.end].iter().map(|x| x.norm_sqr()).sum();
            to_dbfs((power * power_scale).sqrt(), -120.0)
        };
        let drive = |band: &mut Band, floor_db: f32| {
            let gained = band.gain.update_with_floor(level_db(band.bins), dt, floor_db);
            band.envelope.update(gained, dt)
        };

        self.output.energy = drive(&mut self.energy, SILENCE_DB);
        let band_floor = self.energy.gain.peak_db() - BAND_SPREAD_DB;
        self.output.bass = drive(&mut self.bass, band_floor);
        self.output.mid = drive(&mut self.mid, band_floor);
        self.output.treble = drive(&mut self.treble, band_floor);

        // The spectrum shares one gain, driven by its loudest band, so a bass
        // heavy mix still looks bass heavy.
        let levels: [f32; SPECTRUM_BANDS] = std::array::from_fn(|i| level_db(self.spectrum[i].bins));
        let loudest = levels.iter().copied().fold(f32::MIN, f32::max);
        self.spectrum_gain.update(loudest, dt);
        for ((out, band), &db) in self.output.spectrum.iter_mut().zip(&mut self.spectrum).zip(&levels) {
            let gained = self.spectrum_gain.relative(db);
            *out = band.envelope.update(gained, dt);
        }

        &self.output
    }

    /// Copies the newest `out.len()` samples (at most `FFT_SIZE`) into `out`,
    /// oldest first.
    pub fn recent(&self, out: &mut [f32]) {
        assert!(out.len() <= FFT_SIZE, "only the newest {FFT_SIZE} samples are kept");
        let (newest, oldest) = self.history.split_at(self.write);
        let samples = oldest.iter().chain(newest).skip(FFT_SIZE - out.len());
        for (slot, &sample) in out.iter_mut().zip(samples) {
            *slot = sample;
        }
    }
}

/// Bins covering `low..high` Hz, at least one, never including DC (bin 0).
fn hz_to_bins(low: f32, high: f32, sample_rate: f32) -> Bins {
    let bin_hz = sample_rate / FFT_SIZE as f32;
    let nyquist_bin = FFT_SIZE / 2;
    let start = ((low / bin_hz).round() as usize).clamp(1, nyquist_bin - 1);
    let end = ((high / bin_hz).round() as usize).clamp(start + 1, nyquist_bin);
    Bins { start, end }
}

/// `count` bands spaced evenly on a log-frequency axis, as hearing perceives
/// pitch: every band spans the same musical interval.
fn log_bands((low, high): (f32, f32), count: usize, sample_rate: f32) -> Vec<Bins> {
    let ratio = high / low;
    (0..count)
        .map(|i| {
            let edge = |i: usize| low * ratio.powf(i as f32 / count as f32);
            hz_to_bins(edge(i), edge(i + 1), sample_rate)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::TAU;

    const RATE: u32 = 48_000;

    fn sine(hz: f32, amplitude: f32, len: usize) -> Vec<f32> {
        (0..len).map(|t| amplitude * (TAU * hz * t as f32 / RATE as f32).sin()).collect()
    }

    /// Feeds `signal` in 10 ms packets, analyzing after each.
    fn run(analyzer: &mut Analyzer, signal: &[f32]) -> Analysis {
        for packet in signal.chunks(RATE as usize / 100) {
            analyzer.push(packet);
            analyzer.analyze(0.01);
        }
        analyzer.output.clone()
    }

    #[test]
    fn full_scale_sine_measures_minus_3_dbfs() {
        let mut analyzer = Analyzer::new(RATE);
        analyzer.push(&sine(1_000.0, 1.0, FFT_SIZE));
        analyzer.fed = true;
        analyzer.analyze(0.01);
        let power: f32 = analyzer.scratch[1..FFT_SIZE / 2].iter().map(|x| x.norm_sqr()).sum();
        let db = to_dbfs((power * analyzer.power_scale).sqrt(), -120.0);
        assert!((db - -3.01).abs() < 0.05, "got {db} dBFS");
    }

    #[test]
    fn bass_tone_drives_bass_only() {
        let mut analyzer = Analyzer::new(RATE);
        let out = run(&mut analyzer, &sine(80.0, 0.5, RATE as usize));
        assert!(out.bass > 0.9, "bass {}", out.bass);
        assert!(out.treble < 0.1, "treble {}", out.treble);
    }

    #[test]
    fn treble_tone_drives_treble_only() {
        let mut analyzer = Analyzer::new(RATE);
        let out = run(&mut analyzer, &sine(8_000.0, 0.5, RATE as usize));
        assert!(out.treble > 0.9, "treble {}", out.treble);
        assert!(out.bass < 0.1, "bass {}", out.bass);
    }

    #[test]
    fn nearly_empty_band_is_not_amplified() {
        // Treble 48 dB under the bass: audible in theory, irrelevant to the mix.
        let mut analyzer = Analyzer::new(RATE);
        let bass = sine(80.0, 0.5, RATE as usize);
        let faint = sine(8_000.0, 0.002, RATE as usize);
        let mix: Vec<f32> = bass.iter().zip(&faint).map(|(a, b)| a + b).collect();
        let out = run(&mut analyzer, &mix);
        assert!(out.bass > 0.9, "bass {}", out.bass);
        assert!(out.treble < 0.1, "treble {}", out.treble);
    }

    #[test]
    fn spectrum_peaks_at_the_tone() {
        let mut analyzer = Analyzer::new(RATE);
        let out = run(&mut analyzer, &sine(1_000.0, 0.5, RATE as usize));
        let peak = (0..SPECTRUM_BANDS).max_by(|&a, &b| out.spectrum[a].total_cmp(&out.spectrum[b])).unwrap();
        let bands = log_bands(SPECTRUM_HZ, SPECTRUM_BANDS, RATE as f32);
        let bin_hz = RATE as f32 / FFT_SIZE as f32;
        let (low, high) = (bands[peak].start as f32 * bin_hz, bands[peak].end as f32 * bin_hz);
        assert!(low <= 1_020.0 && 980.0 <= high, "peak band {peak} covers {low}..{high} Hz");
    }

    #[test]
    fn stops_reacting_when_packets_stop() {
        let mut analyzer = Analyzer::new(RATE);
        run(&mut analyzer, &sine(80.0, 0.5, RATE as usize));
        for _ in 0..200 {
            analyzer.analyze(0.01);
        }
        assert!(analyzer.output.bass < 0.01, "bass {}", analyzer.output.bass);
        assert!(analyzer.output.energy < 0.01, "energy {}", analyzer.output.energy);
    }

    #[test]
    fn every_log_band_has_bins_and_they_climb() {
        let bands = log_bands(SPECTRUM_HZ, SPECTRUM_BANDS, RATE as f32);
        assert!(bands.iter().all(|b| b.end > b.start));
        assert!(bands.windows(2).all(|w| w[1].start >= w[0].start));
    }
}
