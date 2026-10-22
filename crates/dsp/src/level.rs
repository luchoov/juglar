//! Loudness of a block of samples.

/// Root-mean-square and peak amplitude of a block, both in `0..=1` for
/// normalized audio.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Level {
    pub rms: f32,
    pub peak: f32,
}

impl Level {
    pub fn measure(samples: &[f32]) -> Self {
        if samples.is_empty() {
            return Self::default();
        }
        let (sum_sq, peak) = samples
            .iter()
            .fold((0.0f32, 0.0f32), |(sum, peak), &s| (sum + s * s, peak.max(s.abs())));
        Self { rms: (sum_sq / samples.len() as f32).sqrt(), peak }
    }
}

/// Converts an amplitude to decibels relative to full scale, floored at
/// `floor_db` so silence doesn't become negative infinity.
pub fn to_dbfs(amplitude: f32, floor_db: f32) -> f32 {
    if amplitude <= 0.0 { floor_db } else { (20.0 * amplitude.log10()).max(floor_db) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_scale_square_wave_is_zero_db() {
        let level = Level::measure(&[1.0, -1.0, 1.0, -1.0]);
        assert_eq!(level, Level { rms: 1.0, peak: 1.0 });
        assert_eq!(to_dbfs(level.rms, -90.0), 0.0);
    }

    #[test]
    fn sine_rms_is_peak_over_root_two() {
        let sine: Vec<f32> = (0..48_000).map(|i| (i as f32 * 0.01).sin()).collect();
        let level = Level::measure(&sine);
        assert!((level.rms - std::f32::consts::FRAC_1_SQRT_2).abs() < 1e-3);
    }

    #[test]
    fn silence_hits_the_floor() {
        assert_eq!(Level::measure(&[]), Level::default());
        assert_eq!(to_dbfs(0.0, -90.0), -90.0);
    }
}
