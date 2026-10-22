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
// 3. Bloom: the glow computed by the chain in `bloom.rs` is added.
// 4. Out to the flash limiter (`limiter.wgsl`), in linear light, which
//    checks it, then dithers and encodes it for the screen.
//
// `common.wgsl` and the scene's `overlay` function come before this file.

@group(0) @binding(1) var feedback: texture_2d<f32>;
@group(0) @binding(2) var smooth_sampler: sampler;

// The explorables' background, #09090b, in linear light.
const BACKGROUND: vec3f = vec3f(0.0027, 0.0027, 0.0034);

// The scene's light at `position` on an image of `size` pixels, without the
// background: overlay, palette, tone mapping.
fn shade(position: vec2f, size: vec2f) -> vec3f {
    let uv = position / size;
    // Smoothly filtered: the image has a quarter of the screen's pixels.
    var stored = textureSampleLevel(feedback, smooth_sampler, uv, 0.0).rg;
    let unit = min(size.x, size.y);
    let p = (position - 0.5 * size) / unit - frame.center;
    let paint = overlay(p, 1.0 / unit);
    stored = stored * (1.0 - paint.x) + paint.yz;
    // A gentle contrast curve: faint haze fades further, dense ink stays.
    let ink = max(stored.x, 0.0) * max(stored.x, 0.0) / (max(stored.x, 0.0) + 0.2);
    let position_on_palette = stored.y / max(stored.x, 1e-4);

    let t = position_on_palette + frame.palette_shift;
    let hue = srgb_to_linear(palette(select(clamp(t, 0.0, 1.0), fract(t), frame.palette_wrap > 0.5)));
    // Each channel saturates on its own: faint ink shows the palette's
    // color, dense ink runs into white, like an overexposed photograph.
    return 1.0 - exp(-hue * ink * frame.exposure);
}

// The start of the bloom chain (`bloom.rs`), at a quarter of the window's
// size: only what is bright, fading in from a luminance of 0.2 to full at 0.8.
@fragment
fn fs_bloom_source(@builtin(position) position: vec4f) -> @location(0) vec4f {
    let light = shade(position.xy, ceil(frame.resolution / 4.0));
    let bright = smoothstep(0.2, 0.8, dot(light, vec3f(0.2126, 0.7152, 0.0722)));
    return vec4f(light * bright, 1.0);
}

// The bloom chain's three blurs, from a quarter of the window's size to a
// sixteenth.
@group(1) @binding(0) var glow_quarter: texture_2d<f32>;
@group(1) @binding(1) var glow_eighth: texture_2d<f32>;
@group(1) @binding(2) var glow_sixteenth: texture_2d<f32>;

// A small image enlarged smoothly: four filtered samples half a pixel
// apart, a tent over 3×3 pixels, without the diamond-shaped steps one
// filtered sample shows when an image is enlarged sixteen times.
fn enlarge(image: texture_2d<f32>, uv: vec2f) -> vec3f {
    let half_texel = 0.5 / vec2f(textureDimensions(image));
    var c = textureSampleLevel(image, smooth_sampler, uv + vec2f(-half_texel.x, -half_texel.y), 0.0).rgb;
    c += textureSampleLevel(image, smooth_sampler, uv + vec2f(half_texel.x, -half_texel.y), 0.0).rgb;
    c += textureSampleLevel(image, smooth_sampler, uv + vec2f(-half_texel.x, half_texel.y), 0.0).rgb;
    c += textureSampleLevel(image, smooth_sampler, uv + vec2f(half_texel.x, half_texel.y), 0.0).rgb;
    return 0.25 * c;
}

@fragment
fn fs_composite(@builtin(position) position: vec4f) -> @location(0) vec4f {
    let uv = position.xy / frame.resolution;
    let light = shade(position.xy, frame.resolution);
    // The three blurs' average, as much as the scene asks for. Combined
    // like light through two slides ("screen"), so the glow brightens
    // without ever passing white.
    let blurs = enlarge(glow_quarter, uv) + enlarge(glow_eighth, uv) + enlarge(glow_sixteenth, uv);
    let glow = min(frame.bloom * blurs / 3.0, vec3f(1.0));
    let lit = 1.0 - (1.0 - light) * (1.0 - glow);
    // A faint vignette keeps the eye in the middle.
    let edge = length(uv - 0.5) * 1.4;
    let c = BACKGROUND * (1.0 - 0.5 * edge * edge) + lit;
    return vec4f(clamp(c, vec3f(0.0), vec3f(1.0)), 1.0);
}
