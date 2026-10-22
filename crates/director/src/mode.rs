//! Modes: how much the visuals do overall.
//!
//! *Lively*, the default, is everything this crate does elsewhere. *Focus*
//! and *relax* are style options for watching less, from the research note
//! (`docs/research/focus-and-relaxation.md`, section 8): slow, muted, low in
//! contrast, with no camera flow, changing rarely and gently, and following
//! the music only faintly. They claim nothing about attention or stress;
//! the note found no evidence for such claims, only for what to avoid.

use juglar_signals::{SPECTRUM_BANDS, Signals};

use crate::schedule::Pacing;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Lively,
    Focus,
    Relax,
}

/// How a calm mode changes the picture, for the renderer.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Calm {
    /// Every scene's motion is multiplied by this.
    pub motion: f32,
    /// How much color is kept: 0 is gray, 1 the scene's own colors.
    pub saturation: f32,
    /// The picture's luminance is squeezed between these (linear light,
    /// 0..1): the darkest point is lifted to `floor`, the brightest held to
    /// `ceiling`. Their gap is the largest contrast left anywhere. A floor
    /// of gray (tried first, at 0.10) left a dull gray panel; a dark one
    /// keeps the look of the other modes, only dimmer.
    pub floor: f32,
    pub ceiling: f32,
}

impl Mode {
    /// The next mode, in the order the `M` key goes through them.
    pub fn next(self) -> Mode {
        match self {
            Mode::Lively => Mode::Focus,
            Mode::Focus => Mode::Relax,
            Mode::Relax => Mode::Lively,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Mode::Lively => "lively",
            Mode::Focus => "focus",
            Mode::Relax => "relax",
        }
    }

    /// Within the research table's limits: focus at most 0.20 of luminance
    /// with at most 0.10 of contrast, relax 0.25 with 0.15 (here 0.10 and
    /// 0.15 at most, from black); HSV saturation of 0.10–0.30 and 0.10–0.35,
    /// here approached by keeping a share of each color's difference from
    /// gray.
    pub fn calm(self) -> Option<Calm> {
        match self {
            Mode::Lively => None,
            Mode::Focus => Some(Calm { motion: 0.15, saturation: 0.3, floor: 0.0, ceiling: 0.10 }),
            Mode::Relax => Some(Calm { motion: 0.3, saturation: 0.35, floor: 0.0, ceiling: 0.15 }),
        }
    }

    /// How scenes change: hold a scene at least 120 s (focus) or 60 s
    /// (relax), only scenes with a still camera, and seep from one to the
    /// next over 15 or 20 s.
    pub fn pacing(self) -> Pacing {
        match self {
            Mode::Lively => Pacing::LIVELY,
            Mode::Focus => Pacing { shortest: 120.0, longest: 300.0, still_camera: true, seep_seconds: Some(15.0) },
            Mode::Relax => Pacing { shortest: 60.0, longest: 240.0, still_camera: true, seep_seconds: Some(20.0) },
        }
    }

    /// The intensity in this mode, if the mode sets it.
    pub fn intensity(self) -> Option<f32> {
        match self {
            Mode::Lively => None,
            Mode::Focus => Some(0.0),
            Mode::Relax => Some(0.1),
        }
    }

    /// The music's envelope in this mode: seconds to rise and to fall, and
    /// how much of its swing reaches the scenes.
    fn response(self) -> Option<(f32, f32, f32)> {
        match self {
            Mode::Lively => None,
            Mode::Focus => Some((2.0, 5.0, 0.25)),
            Mode::Relax => Some((1.0, 4.0, 0.4)),
        }
    }
}

/// Makes the signals the scenes see as faint as a calm mode asks: levels
/// follow the music only slowly (seconds, not tenths) and swing only a
/// fraction of their range around a middle value; beats and novelty are
/// gone. The director's own listening is unaffected.
#[derive(Clone, Debug)]
pub struct Quieter {
    levels: [f32; 4],
    spectrum: [f32; SPECTRUM_BANDS],
}

/// Levels swing around this value in the calm modes.
const MIDDLE: f32 = 0.4;

impl Quieter {
    pub fn new() -> Self {
        Self { levels: [MIDDLE; 4], spectrum: [MIDDLE; SPECTRUM_BANDS] }
    }

    pub fn apply(&mut self, mode: Mode, s: &mut Signals) {
        let Some((rise, fall, depth)) = mode.response() else { return };
        let dt = s.dt.clamp(0.0, 0.25);
        let follow = |value: &mut f32, target: f32| {
            let seconds = if target > *value { rise } else { fall };
            *value += (target - *value) * (1.0 - (-dt / seconds).exp());
            MIDDLE + depth * (*value - MIDDLE)
        };
        let [bass, mid, treble, energy] = &mut self.levels;
        s.bass = follow(bass, s.bass);
        s.mid = follow(mid, s.mid);
        s.treble = follow(treble, s.treble);
        s.energy = follow(energy, s.energy);
        for (out, level) in s.spectrum.iter_mut().zip(&mut self.spectrum) {
            *out = follow(level, *out);
        }
        s.waveform.iter_mut().for_each(|v| *v *= depth);
        s.beat = 0.0;
        s.novelty = 0.0;
    }
}

impl Default for Quieter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use juglar_signals::Synthetic;

    #[test]
    fn calm_modes_hear_the_music_faintly() {
        // Through the synthetic drop, the scenes' bass swings by at most a
        // tenth in focus, and never beats.
        let mut song = Synthetic::starting_at(1, 32.0);
        let mut quieter = Quieter::new();
        let mut s = Signals::default();
        let (mut low, mut high) = (f32::MAX, f32::MIN);
        for frame in 0..(16 * 60) {
            song.next(1.0 / 60.0, &mut s);
            quieter.apply(Mode::Focus, &mut s);
            assert_eq!(s.beat, 0.0);
            // After the first seconds of settling.
            if frame > 10 * 60 {
                low = low.min(s.bass);
                high = high.max(s.bass);
            }
        }
        assert!(high - low < 0.1, "{low}..{high}");
    }

    #[test]
    fn lively_changes_nothing() {
        let mut song = Synthetic::starting_at(1, 40.0);
        let mut s = Signals::default();
        song.next(1.0 / 60.0, &mut s);
        let before = s.clone();
        Quieter::new().apply(Mode::Lively, &mut s);
        assert_eq!(s.beat, before.beat);
        assert_eq!(s.bass, before.bass);
    }
}
