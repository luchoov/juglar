// Melt: a band of color melting down the picture. Set in `melt.rs`.
//
// Knobs: 0 how far paint runs down this frame, 1 how far the columns sway
// this frame, 2 the columns' pattern, 3 the sway's phase, 4 the band's
// height, 5 wave height.
//
// Lengths are in short sides from the middle; y grows downward.

fn melt_hash(x: f32) -> f32 {
    return fract(sin(x * 127.1) * 43758.5453);
}

// Smooth noise along one direction: random values blended with an S-curve.
fn melt_noise(x: f32) -> f32 {
    let i = floor(x);
    let f = fract(x);
    return mix(melt_hash(i), melt_hash(i + 1.0), f * f * (3.0 - 2.0 * f));
}

// The paint here ran down from above, faster in some columns than others,
// and the columns sway sideways, mirrored about the middle.
fn warp(p: vec2f, q: vec2f) -> vec2f {
    let x = abs(q.x);
    let speed = 0.15 + 0.85 * melt_noise(x * 14.0 + knob(2u)) * melt_noise(x * 5.0 - 0.5 * knob(2u) + 9.0);
    let side = clamp(q.x * 8.0, -1.0, 1.0);
    let sway = knob(1u) * side * sin(q.y * 6.0 - knob(3u)) * melt_noise(x * 3.0 + 4.0);
    return q - vec2f(sway, knob(0u) * speed);
}

const MELT_SPAN: f32 = 0.82;

// The band's paint at `p`: (coverage, ink × coverage, ink × palette
// position × coverage). Each point as bright as its band of the spectrum,
// bass in the middle, treble at the sides.
fn melt_band(p: vec2f) -> vec3f {
    let along = clamp(abs(p.x) / MELT_SPAN, 0.0, 1.0);
    let x = along * f32(SPECTRUM_BANDS - 1u);
    let i = min(u32(x), SPECTRUM_BANDS - 2u);
    let level = mix(spectrum(i), spectrum(i + 1u), x - f32(i));
    let d = (p.y - knob(4u)) / 0.012;
    let ends = 1.0 - smoothstep(0.9, 1.0, abs(p.x) / MELT_SPAN);
    let coverage = 0.25 * exp(-0.5 * d * d) * ends;
    let ink = 0.3 + 1.4 * level;
    return vec3f(coverage, ink * coverage, ink * 0.6 * along * coverage);
}

// How much of the pixel at `p` the waveform's line covers, along the
// band, mirrored.
fn melt_line(p: vec2f, pixel: f32) -> f32 {
    let x = clamp(abs(p.x) / MELT_SPAN, 0.0, 1.0) * f32(WAVEFORM_POINTS - 1u);
    let i = min(u32(x), WAVEFORM_POINTS - 2u);
    let a = waveform(i);
    let b = waveform(i + 1u);
    let height = knob(4u) - 0.02 + knob(5u) * mix(a, b, x - f32(i));
    let slope = (b - a) * knob(5u) * f32(WAVEFORM_POINTS - 1u) / MELT_SPAN;
    let distance = abs(p.y - height) / sqrt(1.0 + slope * slope);
    let half_width = max(0.0015, 0.5 * pixel);
    let ends = 1.0 - smoothstep(0.85, 1.0, abs(p.x) / MELT_SPAN);
    return (1.0 - smoothstep(half_width - pixel, half_width + pixel, distance)) * ends;
}

fn sources(p: vec2f, pixel: f32) -> vec3f {
    let band = melt_band(p);
    let line = melt_line(p, pixel * 2.0) * 0.04;
    return select(band, vec3f(line, line, line * 0.3), line > band.x);
}

fn overlay(p: vec2f, pixel: f32) -> vec3f {
    let coverage = melt_line(p, pixel) * 0.8;
    let ink = coverage * 1.3;
    return vec3f(coverage, ink, ink * 0.3);
}
