// Oil: blobs of color dragged by a current. Set in `oil.rs`.
//
// Knobs: 0 where the blobs wander, 1 how the current's pattern has
// changed, 2 how far the current moves color this frame, 3-6 each blob's
// level (bass, mid, treble, energy), 7 the current's scale.

// Two layers of noise, drifting in different directions.
fn oil_field(p: vec2f) -> f32 {
    let t = knob(1u);
    let s = knob(7u);
    return noise(p * s + vec2f(t, -0.7 * t)) + 0.5 * noise(p * s * 2.1 + vec2f(17.0 - 1.3 * t, t));
}

// The current: the old paint here came from upstream. Following the field's
// level lines (its gradient turned a quarter turn) gives a flow that neither
// gathers nor spreads.
fn warp(p: vec2f, q: vec2f) -> vec2f {
    let e = 0.02;
    let dx = oil_field(q + vec2f(e, 0.0)) - oil_field(q - vec2f(e, 0.0));
    let dy = oil_field(q + vec2f(0.0, e)) - oil_field(q - vec2f(0.0, e));
    let curl = vec2f(dy, -dx) / (2.0 * e * knob(7u));
    return q - curl * knob(2u);
}

fn blob_center(i: u32) -> vec2f {
    let w = knob(0u);
    let f = f32(i);
    return vec2f(0.3 * sin(w * 0.37 + f * 1.9), 0.2 * cos(w * 0.29 + f * 2.7));
}

fn sources(p: vec2f, pixel: f32) -> vec3f {
    var paint = vec3f(0.0);
    for (var i = 0u; i < 4u; i++) {
        let level = knob(3u + i);
        let size = 0.03 + 0.03 * level;
        let d = length(p - blob_center(i));
        let coverage = 0.12 * exp(-0.5 * d * d / (size * size));
        if coverage > paint.x {
            let ink = 0.6 + 1.4 * level;
            paint = vec3f(coverage, ink * coverage, ink * 0.25 * f32(i) * coverage);
        }
    }
    return paint;
}

// Nothing sharp on top: oil has no edges.
fn overlay(p: vec2f, pixel: f32) -> vec3f {
    return vec3f(0.0);
}
