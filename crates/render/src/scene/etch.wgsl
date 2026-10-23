// Etch: grain sown over the whole screen and a ring thrown out on kicks;
// the feedback's material (`Material`) grows them into a pattern. Set in
// `etch.rs`.
//
// Knobs: 0 stirring this frame, 1 where the stirring is, 2 grain sown per
// frame, 3 the grain's seed, 4 the ring's radius, 5 the ring's ink, 6 the
// waveform's height on the ring, 7 the colors' place on the palette.

// Stirred like a liquid, slowly.
fn warp(p: vec2f, q: vec2f) -> vec2f {
    return q - knob(0u) * flow(q, 2.2, knob(1u));
}

fn sources(p: vec2f, pixel: f32) -> vec3f {
    // Grain: a few cells in every hundred get a drop of ink, new ones every
    // frame; the material does the rest.
    let cell = vec2i(floor(p / (2.0 * pixel)));
    let seed = i32(knob(3u));
    let sown = step(1.0 - knob(2u), grid_random(cell + vec2i(seed * 7919, seed)));
    let grain_color = knob(7u) + 0.25 * grid_random(cell + vec2i(seed, 31));

    // The ring, its edge carrying the waveform.
    let r = length(p);
    let around = fract(atan2(p.y, p.x + 1e-6) / TAU + 0.5);
    let x = abs(2.0 * around - 1.0) * f32(WAVEFORM_POINTS - 1u);
    let i = min(u32(x), WAVEFORM_POINTS - 2u);
    let edge = knob(4u) + knob(6u) * mix(waveform(i), waveform(i + 1u), x - f32(i));
    let ring = (1.0 - smoothstep(0.0, 0.012 + 2.0 * pixel, abs(r - edge))) * knob(5u);

    let coverage = max(sown * 0.8, ring);
    let ink = sown * 0.9 * 0.8 + ring * 1.6;
    let colored = sown * 0.9 * 0.8 * grain_color + ring * 1.6 * (knob(7u) + 0.5);
    return vec3f(coverage, ink, colored);
}

fn overlay(p: vec2f, pixel: f32) -> vec3f {
    return vec3f(0.0);
}
