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
// 3b. Effects (`Fx` in `scene/mod.rs`): the picture jolted, cut into
//    shifted slices, its colors split, lit as a surface, flashed, inverted.
// 4. Out to the present step (`present.wgsl`), in linear light, which
//    dithers and encodes it for the screen.
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
    let light = 1.0 - exp(-hue * ink * frame.exposure);
    let relief = frame.fx[1].w;
    if relief <= 0.0 {
        return light;
    }
    // Relief: the ink's slope, from its neighbors in the feedback image,
    // is a surface lit from the upper left. Thick ink becomes ridges with
    // a dark side and a glint, like metal cooling in a mold.
    let texel = 1.5 / vec2f(textureDimensions(feedback));
    let left = textureSampleLevel(feedback, smooth_sampler, uv - vec2f(texel.x, 0.0), 0.0).r;
    let right = textureSampleLevel(feedback, smooth_sampler, uv + vec2f(texel.x, 0.0), 0.0).r;
    let up = textureSampleLevel(feedback, smooth_sampler, uv - vec2f(0.0, texel.y), 0.0).r;
    let down = textureSampleLevel(feedback, smooth_sampler, uv + vec2f(0.0, texel.y), 0.0).r;
    let normal = normalize(vec3f(left - right, up - down, 0.35));
    let sun = normalize(vec3f(-0.55, -0.6, 0.6));
    let diffuse = max(dot(normal, sun), 0.0);
    let glint = pow(max(reflect(-sun, normal).z, 0.0), 18.0);
    let lit = light * (0.25 + 1.3 * diffuse) + glint * (0.4 + 0.6 * light) * min(stored.x, 1.0);
    return mix(light, lit, relief);
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

// Where the picture at `position` comes from once it is jolted and cut
// into slices.
fn displaced(position: vec2f) -> vec2f {
    let unit = min(frame.resolution.x, frame.resolution.y);
    var at = position - frame.fx[0].xy * unit;
    let glitch = frame.fx[1].y;
    if glitch > 0.0 {
        // Slices of a few heights, each shifted sideways or not, a new
        // pattern with every seed.
        let seed = i32(frame.fx[1].z);
        let rows = 7.0 + 23.0 * grid_random(vec2i(seed, 3));
        let slice = i32(floor(at.y / frame.resolution.y * rows));
        let chosen = grid_random(vec2i(slice, seed));
        if chosen < glitch {
            let shift = grid_random(vec2i(seed, slice + 101)) - 0.5;
            at.x += shift * glitch * 0.6 * frame.resolution.x;
        }
    }
    return at;
}

// The scene's light at a pixel with its colors split: red pulled outward
// from the middle and blue inward, more toward the edges.
fn split(position: vec2f) -> vec3f {
    let at = displaced(position);
    let chroma = frame.fx[0].z;
    if chroma <= 0.0 {
        return shade(at, frame.resolution);
    }
    let unit = min(frame.resolution.x, frame.resolution.y);
    let outward = (at - 0.5 * frame.resolution) / unit;
    let d = outward * chroma * unit * 2.0;
    return vec3f(shade(at + d, frame.resolution).r, shade(at, frame.resolution).g, shade(at - d, frame.resolution).b);
}

// The finished picture at a pixel, before it is dithered for the screen.
fn composite(position: vec2f) -> vec3f {
    let uv = displaced(position) / frame.resolution;
    let light = split(position);
    // The three blurs' average, as much as the scene asks for. Combined
    // like light through two slides ("screen"), so the glow brightens
    // without ever passing white.
    let blurs = enlarge(glow_quarter, uv) + enlarge(glow_eighth, uv) + enlarge(glow_sixteenth, uv);
    let glow = min(frame.bloom * blurs / 3.0, vec3f(1.0));
    let lit = 1.0 - (1.0 - light) * (1.0 - glow);
    // A faint vignette keeps the eye in the middle.
    let edge = length(uv - 0.5) * 1.4;
    var c = clamp(BACKGROUND * (1.0 - 0.5 * edge * edge) + lit, vec3f(0.0), vec3f(1.0));
    // A ground of color: the ink as light over it, or as a stencil that
    // cuts it away.
    let ground_strength = frame.fx[2].x;
    if ground_strength > 0.0 {
        let t = frame.fx[2].y + frame.palette_shift;
        let field = srgb_to_linear(palette(select(clamp(t, 0.0, 1.0), fract(t), frame.palette_wrap > 0.5)));
        let ground = field * ground_strength * (1.0 - 0.35 * edge * edge);
        let over = 1.0 - (1.0 - ground) * (1.0 - lit);
        let cut = ground * (1.0 - min(2.0 * dot(lit, vec3f(0.2126, 0.7152, 0.0722)), 1.0)) + lit * lit;
        c = clamp(mix(over, cut, frame.fx[2].z), vec3f(0.0), vec3f(1.0));
    }
    // A flash: light over everything, as through a screen.
    c = 1.0 - (1.0 - c) * (1.0 - frame.fx[1].x);
    // The negative, as much as asked.
    c = mix(c, 1.0 - c, frame.fx[0].w);
    return calm_look(c);
}

// The calm modes: colors pulled toward gray, and luminance squeezed between
// a floor and a ceiling, so nothing is dark against bright anywhere by more
// than their gap. A floor above black is a cool gray. Mixed in by how far
// the mode has faded in.
fn calm_look(c: vec3f) -> vec3f {
    let luma = dot(c, vec3f(0.2126, 0.7152, 0.0722));
    let muted = mix(vec3f(luma), c, frame.calm.x);
    // Same luminance as gray of `floor`, a little cooler.
    let floor = frame.calm.y * vec3f(0.94, 1.0, 1.1) / dot(vec3f(0.94, 1.0, 1.1), vec3f(0.2126, 0.7152, 0.0722));
    let squeezed = floor + (frame.calm.z - frame.calm.y) * muted;
    return mix(c, squeezed, frame.calm.w);
}

@fragment
fn fs_composite(@builtin(position) position: vec4f) -> @location(0) vec4f {
    return vec4f(composite(position.xy), 1.0);
}

// Transitions. The incoming scene is drawn first, whole; the outgoing one
// is drawn over it with `fs_fade`, whose alpha says how much of it is left
// at each pixel: what is gone shows the new scene underneath.
//
// Each style is a map giving every point a number from 0 to 1: when it
// turns over. As the transition's progress passes a point's number, that
// point turns from the old scene to the new, over a soft band. The maps
// never change during a transition, so every point turns exactly once and
// nothing flickers back and forth.
const STYLE_BLOOM: u32 = 0u;
const STYLE_IRIS: u32 = 1u;
const STYLE_SEEP: u32 = 2u;

// When `p` (centered on the window, its short side spanning -0.5..0.5)
// turns over.
fn turn_over(p: vec2f) -> f32 {
    let style = u32(frame.reveal.y);
    let seed = frame.reveal.w;
    // From the middle to the farthest corner.
    let reach = 0.5 * length(frame.resolution / min(frame.resolution.x, frame.resolution.y));
    let wobble = noise(p * 3.0 + vec2f(seed, 2.0 * seed)) - 0.5;
    switch style {
        case STYLE_BLOOM: {
            return clamp(length(p) / reach * 0.85 + 0.15 + 0.25 * wobble, 0.0, 1.0);
        }
        case STYLE_IRIS: {
            return clamp(1.0 - length(p) / reach * 0.85 + 0.25 * wobble, 0.0, 1.0);
        }
        case STYLE_SEEP: {
            let patches = 0.65 * noise(p * 3.5 + vec2f(seed, 0.0)) + 0.35 * noise(p * 8.0 - vec2f(0.0, seed));
            return smoothstep(0.2, 0.8, patches);
        }
        default: {
            // Two spiral arms sweeping around the middle.
            let around = atan2(p.y, p.x) / TAU + 0.5;
            return fract(2.0 * around + 1.6 * length(p) / reach + seed);
        }
    }
}

@fragment
fn fs_fade(@builtin(position) position: vec4f) -> @location(0) vec4f {
    let unit = min(frame.resolution.x, frame.resolution.y);
    let p = (position.xy - 0.5 * frame.resolution) / unit;
    let soft = frame.reveal.z;
    // Runs from just before every point's turn to just after, so the first
    // and last points also turn over smoothly.
    let edge = frame.reveal.x * (1.0 + 2.0 * soft) - soft;
    let at = turn_over(p);
    let gone = smoothstep(at - soft, at + soft, edge);
    return vec4f(composite(position.xy), 1.0 - gone);
}
