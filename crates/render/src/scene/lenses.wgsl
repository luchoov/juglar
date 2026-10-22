// Lenses: four wandering lenses bending a marbled coat of color. Set in
// `lenses.rs`.
//
// Knobs: 0 where the lenses wander, 1 how the marble has drifted, 2-5 each
// lens's bend this frame (bass, mid, treble, energy).

// How far each lens bends: its bend falls to a third at this distance.
const LENS_REACH: f32 = 0.16;

fn lens_center(i: u32) -> vec2f {
    let w = knob(0u);
    let f = f32(i);
    return vec2f(0.45 * sin(w * (0.41 + 0.07 * f) + f * 1.7), 0.3 * cos(w * (0.33 + 0.05 * f) + f * 2.3));
}

// Each lens bends what lies under it: magnifying lenses (even) read from
// nearer their middle, so the picture under them grows; shrinking ones
// (odd) read from further out. Each also turns what it bends, the two
// kinds opposite ways. The bend fades smoothly away from the lens.
fn warp(p: vec2f, q: vec2f) -> vec2f {
    var r = q;
    for (var i = 0u; i < 4u; i++) {
        let c = lens_center(i);
        let d = r - c;
        let weight = exp(-dot(d, d) / (LENS_REACH * LENS_REACH));
        let sign = select(1.0, -1.0, (i & 1u) == 1u);
        let bend = knob(2u + i) * weight;
        r = c + rotate(d, sign * 1.5 * bend) * (1.0 - sign * bend);
    }
    return r;
}

// A thin coat of color everywhere: the marble, two layers of noise, the
// second one bent by the first.
fn sources(p: vec2f, pixel: f32) -> vec3f {
    let t = knob(1u);
    let bent = p * 4.0 + vec2f(2.0 * noise(p * 3.0 + vec2f(t, 5.0)), 2.0 * noise(p * 3.0 + vec2f(9.0, -t)));
    let position = 1.2 * noise(bent + vec2f(0.5 * t, 0.0));
    let coverage = 0.05;
    let ink = 1.1;
    return vec3f(coverage, coverage * ink, coverage * ink * position);
}

// Each lens's glass edge: a soft, faint ring where its bend fades, so the
// lenses can be told apart from what they bend.
fn overlay(p: vec2f, pixel: f32) -> vec3f {
    var coverage = 0.0;
    for (var i = 0u; i < 4u; i++) {
        let d = (length(p - lens_center(i)) - LENS_REACH) / 0.006;
        coverage = max(coverage, 0.35 * exp(-0.5 * d * d));
    }
    let ink = coverage * 1.4;
    return vec3f(coverage, ink, ink * 0.5);
}
