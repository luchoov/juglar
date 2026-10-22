//! Scenes: what is drawn, how it moves and which colors it uses.
//!
//! The renderer runs the same feedback and composite steps for any scene. A
//! scene supplies two things: WGSL functions (`sources`, painted faintly into
//! the feedback image; `overlay`, painted sharply on top; `warp`, which can
//! bend where each point's old paint comes from), and a `Look` each frame
//! that sets the warp, the knobs its shader reads, and the palette.
//! Milestone 6's generator will compose these from building blocks; for now
//! the scenes are hand-made, each trying a different idea, and the arrow
//! keys switch between them.

mod corona;
mod crystal;
mod harmonograph;
mod julia;
mod kaleido;
mod lenses;
mod melt;
mod oil;
mod orbits;
mod plasma;
mod tide;
mod vortex;

use juglar_signals::{SPECTRUM_BANDS, Signals, WAVEFORM_POINTS};

pub(crate) use tide::Tide;

/// A scene: its shader and how it follows the music.
pub(crate) trait Scene {
    /// Shown in the window title.
    fn name(&self) -> &'static str;
    /// WGSL defining `sources`, `overlay` and `warp` (see `tide.wgsl`).
    fn shader(&self) -> &'static str;
    /// This frame's look. `motion` is 1 normally and 0 with reduced motion,
    /// which must leave a still picture.
    fn update(&mut self, s: &Signals, motion: f32) -> Look;
}

/// Every scene, in the order the arrow keys go through them.
pub(crate) fn all() -> Vec<Box<dyn Scene>> {
    vec![
        Box::new(Tide::new()),
        Box::new(orbits::Orbits::new()),
        Box::new(kaleido::Kaleido::new()),
        Box::new(oil::Oil::new()),
        Box::new(plasma::Plasma::new()),
        Box::new(harmonograph::Harmonograph::new()),
        Box::new(julia::Julia::new()),
        Box::new(crystal::Crystal::new()),
        Box::new(vortex::Vortex::new()),
        Box::new(corona::Corona::new()),
        Box::new(melt::Melt::new()),
        Box::new(lenses::Lenses::new()),
    ]
}

/// Follows `target` over time in three stages, each a running average
/// that takes `seconds` to catch up. One stage alone would still take a
/// small random step every frame when the target is noisy; three leave a
/// smooth path. A thin bright line drawn from a noisy signal must be
/// smoothed like this, or the pixels along it blink (see `tide.rs`).
pub(crate) fn smooth3<const N: usize>(stages: &mut [[f32; N]; 3], target: &[f32; N], dt: f32, seconds: f32) -> [f32; N] {
    let follow = 1.0 - (-dt / seconds).exp();
    let mut input = *target;
    for stage in stages {
        for (smooth, &raw) in stage.iter_mut().zip(&input) {
            *smooth += (raw - *smooth) * follow;
        }
        input = *stage;
    }
    input
}

/// A cosine palette: `color(t) = a + b·cos(2π(c·t + d))`, per channel, in
/// sRGB. Four triples describe a smooth loop of colors.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Palette {
    pub a: [f32; 3],
    pub b: [f32; 3],
    pub c: [f32; 3],
    pub d: [f32; 3],
}

impl Palette {
    /// The color at `t`, as the shader computes it. Only tests need it on
    /// the CPU.
    #[cfg(test)]
    pub fn at(&self, t: f32) -> [f32; 3] {
        std::array::from_fn(|i| {
            (self.a[i] + self.b[i] * (std::f32::consts::TAU * (self.c[i] * t + self.d[i])).cos()).clamp(0.0, 1.0)
        })
    }
}

/// What a scene decides for one frame. Warp values are this frame's step,
/// already scaled by its duration, so motion keeps the same speed at any
/// frame rate.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Look {
    /// Where the warp is centered, from the middle of the window, in units of
    /// its short side.
    pub center: [f32; 2],
    /// How much the old picture grows this frame (1 = not at all).
    pub zoom: f32,
    /// How far it turns this frame, in radians.
    pub rotation: f32,
    /// Extra turn at the center, fading to none at the edges, in radians.
    pub swirl: f32,
    /// What the old ink is multiplied by this frame (1 = it never fades).
    pub decay: f32,
    /// How far old ink moves along the palette this frame.
    pub palette_drift: f32,
    /// Numbers the scene's own shader reads; their meaning is the scene's.
    pub knobs: [f32; 8],
    pub palette: Palette,
    /// Added to every palette position: turns the whole picture's colors.
    pub palette_shift: f32,
    /// Whether the palette loops (positions wrap) or runs between two ends.
    pub palette_wrap: bool,
    /// How bright a given amount of ink looks.
    pub exposure: f32,
    /// How much the bright parts glow (0 = none, 1 = strong; see `bloom.rs`).
    pub bloom: f32,
    /// The waveform and spectrum the scene draws, which may be calmer than
    /// the ones heard. A scene that doesn't draw the waveform may fill its
    /// slots with numbers of its own (Harmonograph: its figure).
    pub waveform: [f32; WAVEFORM_POINTS],
    pub spectrum: [f32; SPECTRUM_BANDS],
}

#[cfg(test)]
mod tests {
    use super::*;

    fn loud() -> Signals {
        Signals { dt: 1.0 / 60.0, bass: 1.0, mid: 1.0, treble: 1.0, energy: 1.0, beat: 1.0, novelty: 1.0, intensity: 1.0, ..Signals::default() }
    }

    #[test]
    fn no_palette_turns_saturated_red() {
        // Research policy: animated colors keep red under 75% of the light.
        for mut scene in all() {
            let look = scene.update(&loud(), 1.0);
            for i in 0..=1000 {
                let c = look.palette.at(i as f32 / 1000.0);
                let linear = c.map(|c| if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) });
                let red_share = linear[0] / linear.iter().sum::<f32>();
                assert!(red_share <= 0.75, "{}: {red_share} at {i}", scene.name());
            }
        }
    }

    #[test]
    fn reduced_motion_stands_still() {
        for mut scene in all() {
            let first = scene.update(&loud(), 0.0);
            let second = scene.update(&loud(), 0.0);
            assert_eq!(second.zoom, 1.0, "{}", scene.name());
            assert_eq!(second.rotation, 0.0, "{}", scene.name());
            assert_eq!(second.swirl, 0.0, "{}", scene.name());
            assert_eq!(first.knobs, second.knobs, "{}", scene.name());
            assert_eq!(first.center, second.center, "{}", scene.name());
            assert_eq!(first.palette_shift, second.palette_shift, "{}", scene.name());
        }
    }
}
