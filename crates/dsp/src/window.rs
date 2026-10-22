//! Window functions applied to a block before its FFT.

use std::f64::consts::TAU;

/// Hann window of length `n`: `0.5 - 0.5·cos(2πt/n)`. Periodic form, the one
/// meant for spectral analysis (it tiles seamlessly at 50% overlap).
pub fn hann(n: usize) -> Vec<f32> {
    (0..n).map(|t| (0.5 - 0.5 * (TAU * t as f64 / n as f64).cos()) as f32).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starts_at_zero_and_peaks_in_the_middle() {
        let w = hann(8);
        assert_eq!(w[0], 0.0);
        assert!((w[4] - 1.0).abs() < 1e-6);
        assert!((w[2] - w[6]).abs() < 1e-6);
    }

    #[test]
    fn overlapping_halves_sum_to_one() {
        let w = hann(16);
        for t in 0..8 {
            assert!((w[t] + w[t + 8] - 1.0).abs() < 1e-6);
        }
    }
}
