// The composite step: turns the feedback image's ink into the colors on
// screen, at full size.
//
// 0. Overlay: the scene paints its sharp, present-moment shapes on top.
// 1. Palette: each pixel's palette position becomes a color. Positions are
//    clamped to 0..1, so the palette is a path between two ends.
// 2. Tone mapping: ink has no upper limit, light on a screen does. A curve
//    with a soft shoulder maps any amount of ink into 0..1, so dense paint
//    glows (and turns toward white) instead of clipping into flat patches.
// 3. Dithering: a screen shows 256 levels per channel. A slow dark gradient
//    crosses only a few of them and shows bands. Adding noise of one level
//    before rounding turns the bands into an even grain too fine to see.
//
// `common.wgsl` and the scene's `overlay` function come before this file.

@group(0) @binding(1) var feedback: texture_2d<f32>;
@group(0) @binding(2) var smooth_sampler: sampler;

// The explorables' background, #09090b, in linear light.
const BACKGROUND: vec3f = vec3f(0.0027, 0.0027, 0.0034);

// A well-mixed 32-bit hash (PCG), for noise without textures.
fn hash(n: u32) -> u32 {
    let state = n * 747796405u + 2891336453u;
    let word = ((state >> ((state >> 28u) + 4u)) ^ state) * 277803737u;
    return (word >> 22u) ^ word;
}

fn random(n: u32) -> f32 {
    return f32(hash(n)) / 4294967296.0;
}

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

    let hue = srgb_to_linear(palette(clamp(position_on_palette + frame.palette_shift, 0.0, 1.0)));
    // Each channel saturates on its own: faint ink shows the palette's
    // color, dense ink runs into white, like an overexposed photograph.
    let light = 1.0 - exp(-hue * ink * frame.exposure);
    // A faint vignette keeps the eye in the middle.
    let edge = length(uv - 0.5) * 1.4;
    let c = BACKGROUND * (1.0 - 0.5 * edge * edge) + light;

    // Dither in the screen's own encoding, where its 256 levels are evenly
    // spaced: two random numbers summed give triangular noise of ±1 level,
    // which hides banding without adding visible grain.
    let pixel = vec2u(position.xy);
    let n = (pixel.y * 8192u + pixel.x) * 2u + u32(frame.seed) * 33554432u;
    var encoded = linear_to_srgb(c) + (random(n) + random(n + 1u) - 1.0) / 255.0;
    encoded = clamp(encoded, vec3f(0.0), vec3f(1.0));
    if frame.encode_srgb > 0.5 {
        return vec4f(encoded, 1.0);
    }
    // The screen format encodes by itself: hand it linear light that
    // encodes back to exactly these dithered values.
    return vec4f(srgb_to_linear(encoded), 1.0);
}
