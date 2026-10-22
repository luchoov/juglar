//! Scenes: what is drawn, how it moves and which colors it uses.
//!
//! The renderer runs the same feedback and composite steps for any scene. A
//! scene supplies two things: a WGSL `sources` function that paints into the
//! feedback image, and a `Look` each frame that sets the warp, the knobs its
//! sources read, and the palette. Milestone 6's generator will compose these
//! from building blocks; for now there is one hand-made scene, `Tide`.

mod tide;

pub(crate) use tide::Tide;

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
    /// How bright a given amount of ink looks.
    pub exposure: f32,
}
