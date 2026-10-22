// The feedback step, run once per frame on the half-size image.
//
// Every frame starts as a copy of the previous one, slightly warped (zoomed,
// turned, swirled) and faded. Then the scene paints its sources on top. Old
// paint keeps flowing along the warp and fading, so a few simple shapes leave
// trails, tunnels and spirals that nobody drew.
//
// The image doesn't hold colors. Each pixel holds two numbers:
//   r = ink: how much paint is there (0 = none; above 1 is fine),
//   g = ink × palette position: which color the paint is, weighted by ink.
// Weighting by ink means blending and fading treat both alike, and the
// composite step recovers the position as g / r. Storing positions instead
// of colors lets older paint drift along the palette as it ages.
//
// `common.wgsl` and the scene's `sources` function come before this file.

@group(0) @binding(1) var previous: texture_2d<f32>;
@group(0) @binding(2) var smooth_sampler: sampler;

@fragment
fn fs_feedback(@builtin(position) position: vec4f) -> @location(0) vec4f {
    let size = frame.feedback_size;
    let unit = min(size.x, size.y);
    // Centered on the warp's center; the short side spans -0.5..0.5.
    let p = (position.xy - 0.5 * size) / unit - frame.center;

    // Where was this pixel's paint one frame ago? Undo this frame's step:
    // turn back, and shrink toward the center (zooming in on the old image
    // makes its content grow outward). The swirl turns the middle more than
    // the edges, which bends straight trails into spirals.
    let turn = frame.rotation + frame.swirl * (1.0 - smoothstep(0.0, 0.7, length(p)));
    let q = rotate(p, -turn) / frame.zoom;
    let uv = ((q + frame.center) * unit + 0.5 * size) / size;

    // Filtered between the four nearest pixels, so motion of a fraction of
    // a pixel per frame still moves the picture smoothly.
    var ink = textureSampleLevel(previous, smooth_sampler, uv, 0.0).rg;
    // Nothing comes in from outside the old image.
    let inside = all(uv >= vec2f(0.0)) && all(uv <= vec2f(1.0));
    ink = select(vec2f(0.0), ink, inside);
    ink *= frame.decay;
    ink.y += ink.x * frame.palette_drift;

    // New paint goes over the old: `paint` is (coverage, ink × coverage,
    // ink × palette position × coverage).
    let paint = sources(p, 1.0 / unit);
    ink = ink * (1.0 - paint.x) + paint.yz;
    return vec4f(ink, 0.0, 1.0);
}
