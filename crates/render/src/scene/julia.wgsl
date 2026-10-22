// Julia: one step of z → z² + c per frame. Set in `julia.rs`.
//
// Knobs: 0-1 c, 2 fractal units per short side, 3 ring radius (fractal
// units), 4 wave height on the ring, 5 line half-width (short sides),
// 6 ink where points escape, 7 ring ink.

fn julia_step(z: vec2f) -> vec2f {
    return vec2f(z.x * z.x - z.y * z.y, 2.0 * z.x * z.y) + vec2f(knob(0u), knob(1u));
}

// The old paint comes from the next point of the orbit.
fn warp(p: vec2f, q: vec2f) -> vec2f {
    let scale = knob(2u);
    return julia_step(q * scale) / scale;
}

// How much of the pixel at `p` the waveform ring covers, in fractal
// units, mirrored left to right like Tide's.
fn julia_ring(z: vec2f, pixel: f32) -> f32 {
    let t = abs(atan2(z.x, -z.y)) / PI;
    let x = t * f32(WAVEFORM_POINTS - 1u);
    let i = min(u32(x), WAVEFORM_POINTS - 2u);
    let radius = knob(3u) + knob(4u) * mix(waveform(i), waveform(i + 1u), x - f32(i));
    let half_width = max(knob(5u) * knob(2u), 0.5 * pixel);
    return 1.0 - smoothstep(half_width - pixel, half_width + pixel, abs(length(z) - radius));
}

fn sources(p: vec2f, pixel: f32) -> vec3f {
    let scale = knob(2u);
    let z = p * scale;
    let next = julia_step(z);
    // Points whose next step leaves the circle that fits in the image never
    // come back: paint them. (The circle is 1.5 across in fractal units,
    // just past where any orbit of these sets can return from.) Their color
    // turns with the direction they leave in, so the outside isn't flat.
    let escape = smoothstep(-0.1, 0.0, length(next) - 0.49 * scale);
    let leaving = 0.12 * sin(2.0 * atan2(next.y, next.x));
    let ring = julia_ring(z, pixel * scale);
    // The ring covers the escape paint; each starts at its own color.
    let coverage = max(escape, ring);
    let share = ring / max(coverage, 1e-4);
    let ink = mix(knob(6u), knob(7u), share) * coverage;
    return vec3f(coverage, ink, ink * mix(leaving, 0.5, share));
}

// Nothing sharp on top: the ring lives inside the fractal.
fn overlay(p: vec2f, pixel: f32) -> vec3f {
    return vec3f(0.0);
}
