// Plasma: four slow waves summed into a color. Set in `plasma.rs`.
//
// Knobs: 0 the waves' phase, 1 a clock for the circling centers, 2 how much
// ink each frame lays down.

// The sum of four waves at `p`, between -1 and 1.
fn plasma(p: vec2f) -> f32 {
    let phase = knob(0u);
    let clock = knob(1u);
    // Two circling centers, wandering slowly.
    let a = vec2f(0.3 * sin(clock * 0.13), 0.25 * cos(clock * 0.17));
    let b = vec2f(0.35 * cos(clock * 0.11 + 2.0), 0.2 * sin(clock * 0.07 + 1.0));
    var v = sin(p.x * 5.0 + phase);
    v += sin((p.x + p.y) * 3.5 - 0.8 * phase);
    v += sin(length(p - a) * 7.0 - 1.3 * phase);
    v += sin(length(p - b) * 6.0 + 0.9 * phase);
    return 0.25 * v;
}

fn warp(p: vec2f, q: vec2f) -> vec2f {
    return q;
}

// A thin coat of color everywhere: (coverage, ink, ink × palette position).
fn sources(p: vec2f, pixel: f32) -> vec3f {
    let coverage = 0.06;
    let ink = knob(2u);
    // Most of the palette across the whole range of the waves.
    let position = 0.8 * plasma(p);
    return vec3f(coverage, coverage * ink, coverage * ink * position);
}

fn overlay(p: vec2f, pixel: f32) -> vec3f {
    return vec3f(0.0);
}
