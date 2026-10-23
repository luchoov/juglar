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

mod barrens;
mod bore;
mod citadel;
mod corona;
mod crystal;
mod datamosh;
mod etch;
mod fracture;
mod furnace;
mod hammer;
mod harmonograph;
mod julia;
mod kaleido;
mod lenses;
mod liquid;
mod maelstrom;
mod melt;
mod oil;
mod orbits;
mod plasma;
pub(crate) mod react;
mod shrapnel;
mod space;
mod static_;
mod tide;
mod vortex;
mod wire;

pub use juglar_director::Character;
use juglar_signals::{SPECTRUM_BANDS, Signals, WAVEFORM_POINTS};

pub(crate) use tide::Tide;

/// A scene: its shader and how it follows the music.
pub(crate) trait Scene {
    /// Shown in the window title.
    fn name(&self) -> &'static str;
    /// What it is like, for the director to match it with music.
    fn character(&self) -> Character;
    /// WGSL defining `sources`, `overlay` and `warp` (see `tide.wgsl`).
    fn shader(&self) -> &'static str;
    /// This frame's look. `motion` is 1 normally and 0 with reduced motion,
    /// which must leave a still picture.
    fn update(&mut self, s: &Signals, motion: f32) -> Look;
}

/// Every scene, in the order the arrow keys go through them.
pub(crate) fn all() -> Vec<Box<dyn Scene>> {
    vec![
        Box::new(etch::Etch::new()),
        Box::new(liquid::Liquid::new()),
        Box::new(shrapnel::Shrapnel::new()),
        Box::new(furnace::Furnace::new()),
        Box::new(static_::Static::new()),
        Box::new(maelstrom::Maelstrom::new()),
        Box::new(hammer::Hammer::new()),
        Box::new(wire::Wire::new()),
        Box::new(datamosh::Datamosh::new()),
        Box::new(fracture::Fracture::new()),
        Box::new(citadel::Citadel::new()),
        Box::new(bore::Bore::new()),
        Box::new(barrens::Barrens::new()),
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
    /// How the old picture works on itself each frame.
    pub material: Material,
    /// Effects on the finished picture.
    pub fx: Fx,
}

/// How the old picture works on itself each frame, in the feedback step,
/// before the new paint: what MilkDrop's warp shaders do to grow texture
/// out of nothing. Each pixel's ink is pushed away from (or toward) the
/// average of its neighborhood, squeezed under a ceiling, and worn down a
/// little. Pushed away hard enough, small differences grow on their own
/// into spots, stripes and contour lines, and the ceiling flattens them
/// into plateaus: the patterns of reaction and diffusion (Geiss' "Reaction
/// Diffusion", flexi's "acid etching"), here from the scene's own paint.
/// All zero leaves the picture as it is.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Material {
    /// How far the ink is pushed from its neighborhood's average each
    /// frame (at 60 fps): above about 0.3 patterns grow; below 0 it blurs.
    pub sharpen: f32,
    /// The ceiling ink is squeezed under, softly (0 = none).
    pub cap: f32,
    /// Ink worn away each frame (at 60 fps), so faint ink dies and
    /// patterns keep their edges.
    pub erode: f32,
    /// How far the neighborhood reaches, in feedback pixels: wider grows
    /// bigger spots.
    pub reach: f32,
}

impl Material {
    /// The picture as it is.
    pub const NONE: Material = Material { sharpen: 0.0, cap: 0.0, erode: 0.0, reach: 0.0 };

    /// As `Frame::material` in `common.wgsl` reads it.
    pub fn packed(&self) -> [f32; 4] {
        [self.sharpen, self.cap, self.erode, self.reach]
    }
}

/// Effects on the finished picture, applied in `composite.wgsl` after the
/// palette: what MilkDrop's composite shaders do to make a hit land. All
/// zero is the picture as it is.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Fx {
    /// The whole picture jolted this far, in units of the window's short
    /// side.
    pub shake: [f32; 2],
    /// Red and blue pulled apart from green, outward from the middle, this
    /// far at the edges (in units of the short side): a lens torn by a hit.
    pub chroma: f32,
    /// How far the colors are turned to their negative, 0..1.
    pub invert: f32,
    /// Light added over everything, 0..1.
    pub flash: f32,
    /// How much of the picture is cut into horizontal slices shifted
    /// sideways, 0..1, and which slices (any number; a new one is a new
    /// pattern of slices).
    pub glitch: f32,
    pub glitch_seed: f32,
    /// How much the ink is lit as a surface, from the side, with a glint:
    /// molten metal instead of light. 0..1.
    pub relief: f32,
    /// A ground of color instead of black: how bright (0..1), and where on
    /// the palette (shifted with it).
    pub ground: f32,
    pub ground_hue: f32,
    /// 0: the ink is light over the ground. 1: the ink is a stencil that
    /// cuts the ground away, dark figures on a field of color.
    pub stencil: f32,
}

impl Fx {
    /// None at all.
    pub const NONE: Fx = Fx {
        shake: [0.0; 2],
        chroma: 0.0,
        invert: 0.0,
        flash: 0.0,
        glitch: 0.0,
        glitch_seed: 0.0,
        relief: 0.0,
        ground: 0.0,
        ground_hue: 0.0,
        stencil: 0.0,
    };

    /// As `Frame::fx` in `common.wgsl` reads it.
    pub fn packed(&self) -> [[f32; 4]; 3] {
        [
            [self.shake[0], self.shake[1], self.chroma, self.invert],
            [self.flash, self.glitch, self.glitch_seed, self.relief],
            [self.ground, self.ground_hue, self.stencil, 0.0],
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn loud() -> Signals {
        Signals { dt: 1.0 / 60.0, bass: 1.0, mid: 1.0, treble: 1.0, energy: 1.0, beat: 1.0, novelty: 1.0, intensity: 1.0, ..Signals::default() }
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
