// The composite step: turns the feedback image's ink into the colors on
// screen, at full size.
//
// 0. Overlay: the scene paints its sharp, present-moment shapes on top.
// 1. Palette: each pixel's palette position becomes a color. Positions are
//    clamped to 0..1, so the palette is a path between two ends, or, for
//    a scene whose palette loops, wrapped around.
// 2. Tone mapping: ink has no upper limit, light on a screen does. A curve
//    with a soft shoulder maps any amount of ink into 0..1, so dense paint
//    glows (and turns toward white) instead of clipping into flat patches.
// 3. Out to the flash limiter (`limiter.wgsl`), in linear light, which
//    checks it, then dithers and encodes it for the screen.
//
// `common.wgsl` and the scene's `overlay` function come before this file.

@group(0) @binding(1) var feedback: texture_2d<f32>;
@group(0) @binding(2) var smooth_sampler: sampler;

// The explorables' background, #09090b, in linear light.
const BACKGROUND: vec3f = vec3f(0.0027, 0.0027, 0.0034);

@fragment
fn fs_composite(@builtin(position) position: vec4f) -> @location(0) vec4f {
    let uv = position.xy / frame.resolution;
    // Smoothly filtered: the image has a quarter of the screen's pixels.
    var stored = textureSampleLevel(feedback, smooth_sampler, uv, 0.0).rg;
    let unit = min(frame.resolution.x, frame.resolution.y);
    let p = (position.xy - 0.5 * frame.resolution) / unit - frame.center;
    let paint = overlay(p, 1.0 / unit);
    stored = stored * (1.0 - paint.x) + paint.yz;
    // A gentle contrast curve: faint haze fades further, dense ink stays.
    let ink = max(stored.x, 0.0) * max(stored.x, 0.0) / (max(stored.x, 0.0) + 0.2);
    let position_on_palette = stored.y / max(stored.x, 1e-4);

    let t = position_on_palette + frame.palette_shift;
    let hue = srgb_to_linear(palette(select(clamp(t, 0.0, 1.0), fract(t), frame.palette_wrap > 0.5)));
    // Each channel saturates on its own: faint ink shows the palette's
    // color, dense ink runs into white, like an overexposed photograph.
    let light = 1.0 - exp(-hue * ink * frame.exposure);
    // A faint vignette keeps the eye in the middle.
    let edge = length(uv - 0.5) * 1.4;
    let c = BACKGROUND * (1.0 - 0.5 * edge * edge) + light;
    return vec4f(clamp(c, vec3f(0.0), vec3f(1.0)), 1.0);
}
