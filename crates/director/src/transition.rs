//! Transitions: how one scene gives way to the next.
//!
//! The renderer draws both scenes while a transition lasts, each in its own
//! feedback loop, the new one starting from a copy of the old picture, so
//! the new scene's motion takes hold of the old shapes before its own paint
//! replaces them. The old scene's picture is then withdrawn through a shape
//! (`Style`), at a pace partly set by the clock and partly by the beats.

/// The shape through which the new scene appears.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Style {
    /// Opens from the middle outward, the edge wobbling a little: the new
    /// scene bursts out of the old one. For drops.
    Bloom,
    /// Closes from the edges inward, the old scene shrinking into the
    /// middle until it is gone. For breakdowns and endings.
    Iris,
    /// Seeps in through a soft pattern of patches, like ink through paper.
    /// For quiet music, and for the calm modes.
    Seep,
    /// Sweeps around in two spiral arms. For a change of section that is
    /// neither a rise nor a fall.
    Spiral,
}

/// How much of a beat's signal there is: it jumps to 1 and falls with a
/// time constant of 0.15 s, in the analysis (`onset.rs`) and in the
/// synthetic song alike, so it adds up to 0.15 over each beat.
const BEAT_AREA: f32 = 0.15;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Transition {
    pub style: Style,
    /// How long it takes with no beats.
    pub seconds: f32,
    /// How much further each beat takes it, as a share of the whole: with
    /// beats it moves in surges, and ends sooner.
    pub per_beat: f32,
}

impl Transition {
    /// A change the listener asked for, with the arrow keys: quick, from
    /// the middle.
    pub fn asked() -> Self {
        Self { style: Style::Bloom, seconds: 1.5, per_beat: 0.0 }
    }

    /// How far it advances in `dt` seconds with this `beat` signal.
    pub fn step(&self, dt: f32, beat: f32) -> f32 {
        dt / self.seconds.max(1e-3) + self.per_beat * beat * dt / BEAT_AREA
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use juglar_signals::{Signals, Synthetic};

    #[test]
    fn each_beat_adds_its_share() {
        // A second of the synthetic drop holds two beats.
        let mut song = Synthetic::starting_at(1, 40.0);
        let mut s = Signals::default();
        let transition = Transition { style: Style::Bloom, seconds: 1e6, per_beat: 0.1 };
        let mut progress = 0.0;
        for _ in 0..600 {
            song.next(0.01, &mut s);
            progress += transition.step(0.01, s.beat);
        }
        // Six seconds, twelve beats; a little under 1.2, since each beat's
        // tail is cut short by the next and the signal is sampled in steps.
        assert!((progress - 1.2).abs() < 0.1, "{progress}");
    }
}
