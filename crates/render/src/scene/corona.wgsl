// Corona: flames rising from a ring around a dark sun. Set in `corona.rs`.
//
// Knobs: 0 the turbulence's pattern, 1 how far it moves paint this frame,
// 2 the sun's radius, 3 the ring's soft width, 4 how much the ring gives off.

fn corona_hash(p: vec2f) -> f32 {
    return fract(sin(dot(p, vec2f(127.1, 311.7))) * 43758.5453);
}

// Smooth noise: random values on a grid, blended with an S-curve.
fn corona_noise(p: vec2f) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    let bottom = mix(corona_hash(i), corona_hash(i + vec2f(1.0, 0.0)), u.x);
    let top = mix(corona_hash(i + vec2f(0.0, 1.0)), corona_hash(i + vec2f(1.0, 1.0)), u.x);
    return mix(bottom, top, u.y);
}

// Two layers of noise, churning in different directions.
fn corona_field(p: vec2f) -> f32 {
    let t = knob(0u);
    return corona_noise(p * 5.0 + vec2f(t, -0.6 * t)) + 0.5 * corona_noise(p * 11.0 + vec2f(-1.2 * t, 0.8 * t + 7.0));
}

// The tunnel's outward flow, plus turbulence: the curl of the noise field
// (its level lines, followed sideways), as in Oil, which stirs without
// piling paint up anywhere.
fn warp(p: vec2f, q: vec2f) -> vec2f {
    let e = 0.01;
    let dx = corona_field(q + vec2f(e, 0.0)) - corona_field(q - vec2f(e, 0.0));
    let dy = corona_field(q + vec2f(0.0, e)) - corona_field(q - vec2f(0.0, e));
    let curl = vec2f(dy, -dx) / (2.0 * e * 5.0);
    return q - curl * knob(1u);
}

// The level of the spectrum around the ring: bass at the top, treble at
// the bottom, mirrored left to right.
fn corona_level(p: vec2f) -> f32 {
    let t = abs(atan2(p.x, -p.y)) / PI;
    let x = t * f32(SPECTRUM_BANDS - 1u);
    let i = min(u32(x), SPECTRUM_BANDS - 2u);
    return mix(spectrum(i), spectrum(i + 1u), x - f32(i));
}

fn sources(p: vec2f, pixel: f32) -> vec3f {
    let r = length(p);
    let radius = knob(2u);
    // Paint given off just outside the sun's edge, as much as the band.
    let d = (r - radius - knob(3u)) / knob(3u);
    let level = corona_level(p);
    // Tongues: the ring gives off more at some angles than others, a
    // pattern that churns with the turbulence and has no regular spacing.
    let angle = atan2(p.y, p.x);
    let tongues = corona_noise(vec2f(cos(angle), sin(angle)) * 4.0 + vec2f(knob(0u) * 1.7, 3.0));
    let glow = exp(-0.5 * d * d) * knob(4u) * (0.1 + level) * (0.2 + 1.6 * tongues * tongues);
    // Inside the sun, nothing: a dark disc that covers the flames' past.
    let disc = 1.0 - smoothstep(radius - 2.0 * pixel, radius, r);
    let coverage = max(glow, disc);
    let ink = glow * (0.8 + 1.2 * level);
    // Louder bands start hotter on the palette.
    return vec3f(coverage, ink, ink * -0.15 * level);
}

// The sun's rim, thin and steady, on top.
fn overlay(p: vec2f, pixel: f32) -> vec3f {
    let d = abs(length(p) - knob(2u));
    let coverage = (1.0 - smoothstep(0.0015, 0.0015 + 1.5 * pixel, d)) * 0.8;
    let ink = coverage * 1.2;
    return vec3f(coverage, ink, 0.0);
}
