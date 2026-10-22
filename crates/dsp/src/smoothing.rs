//! Turning jittery per-frame measurements into values that move like music.
//!
//! Everything here is driven by elapsed time (`dt`), never by frame counts, so
//! it behaves the same at 30, 60 or 144 frames per second.

/// Attack/release envelope follower.
///
/// Rises toward a louder input with the `attack` time constant and falls
/// toward a quieter one with `release`. A fast attack and slow release is what
/// makes visuals jump on a hit and then breathe out, instead of trembling.
#[derive(Clone, Copy, Debug)]
pub struct Envelope {
    attack: f32,
    release: f32,
    value: f32,
}

impl Envelope {
    /// Time constants in seconds: after one of them, the output has covered
    /// about 63% of the distance to a new steady input.
    pub fn new(attack: f32, release: f32) -> Self {
        Self { attack, release, value: 0.0 }
    }

    pub fn update(&mut self, input: f32, dt: f32) -> f32 {
        let tau = if input > self.value { self.attack } else { self.release };
        // Exact exponential smoothing for this `dt`, not the frame-rate
        // dependent `value += (input - value) * k`.
        let k = if tau > 0.0 { 1.0 - (-dt / tau).exp() } else { 1.0 };
        self.value += (input - self.value) * k;
        self.value
    }

    pub fn value(&self) -> f32 {
        self.value
    }
}

/// Automatic gain: maps a level in dBFS to `0..=1` relative to how loud the
/// signal has been recently, so quiet and loud songs both use the full range.
///
/// It follows the recent peak instantly upward and lets it sink slowly, then
/// shows the last `range_db` decibels below that peak. The peak never sinks
/// below `floor_db`, so silence and background hiss stay near zero instead of
/// being amplified into noise.
#[derive(Clone, Copy, Debug)]
pub struct AutoGain {
    peak_db: f32,
    range_db: f32,
    floor_db: f32,
    fall_db_per_s: f32,
}

impl AutoGain {
    pub fn new(range_db: f32, floor_db: f32, fall_db_per_s: f32) -> Self {
        Self { peak_db: floor_db, range_db, floor_db, fall_db_per_s }
    }

    /// Tracks `level_db` and returns it normalized.
    pub fn update(&mut self, level_db: f32, dt: f32) -> f32 {
        self.update_with_floor(level_db, dt, self.floor_db)
    }

    /// Like `update`, but the peak is not allowed to sink below `floor_db`
    /// this frame. Lets a band's gain be tied to a louder reference, so an
    /// almost empty band isn't amplified to full scale.
    pub fn update_with_floor(&mut self, level_db: f32, dt: f32, floor_db: f32) -> f32 {
        let floor_db = floor_db.max(self.floor_db);
        self.peak_db = if level_db > self.peak_db {
            level_db
        } else {
            (self.peak_db - self.fall_db_per_s * dt).max(floor_db)
        };
        self.peak_db = self.peak_db.max(floor_db);
        self.relative(level_db)
    }

    pub fn peak_db(&self) -> f32 {
        self.peak_db
    }

    /// Normalizes `level_db` against the current peak without tracking it,
    /// for several levels that share one gain.
    pub fn relative(&self, level_db: f32) -> f32 {
        ((level_db - (self.peak_db - self.range_db)) / self.range_db).clamp(0.0, 1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn envelope_covers_63_percent_after_one_time_constant() {
        let mut env = Envelope::new(0.1, 1.0);
        // Same total time in different frame sizes gives the same result.
        for _ in 0..100 {
            env.update(1.0, 0.001);
        }
        assert!((env.value() - 0.632).abs() < 0.01);

        let mut coarse = Envelope::new(0.1, 1.0);
        coarse.update(1.0, 0.1);
        assert!((coarse.value() - env.value()).abs() < 1e-3);
    }

    #[test]
    fn envelope_attacks_fast_and_releases_slow() {
        let mut env = Envelope::new(0.01, 0.5);
        env.update(1.0, 0.05);
        assert!(env.value() > 0.99);
        env.update(0.0, 0.05);
        assert!(env.value() > 0.85);
    }

    #[test]
    fn auto_gain_normalizes_quiet_and_loud_songs_alike() {
        let mut loud = AutoGain::new(30.0, -60.0, 3.0);
        let mut quiet = AutoGain::new(30.0, -60.0, 3.0);
        for _ in 0..100 {
            loud.update(-6.0, 0.01);
            quiet.update(-26.0, 0.01);
        }
        assert_eq!(loud.update(-6.0, 0.01), 1.0);
        assert_eq!(quiet.update(-26.0, 0.01), 1.0);
        assert!((loud.update(-21.0, 0.01) - 0.5).abs() < 0.01);
    }

    #[test]
    fn auto_gain_keeps_silence_at_zero() {
        let mut gain = AutoGain::new(30.0, -60.0, 3.0);
        for _ in 0..1000 {
            assert_eq!(gain.update(-95.0, 0.01), 0.0);
        }
    }
}
