// Crystal: a fold, a shift and a shrink per frame. Set in `crystal.rs`.
//
// Knobs: 0 the fold's turn, 1 mirrored wedges, 2 how far out the copies
// sit, 3 how much smaller each generation is, 4 ring radius, 5 wave height
// on the ring, 6 ring half-width, 7 ring ink.

// `p` folded into one mirrored wedge, as in Kaleido: the angle taken modulo
// a wedge and reflected about its middle, the distance kept.
fn crystal_fold(p: vec2f, wedges: f32) -> vec2f {
    let wedge = TAU / wedges;
    var angle = atan2(p.y, p.x);
    angle = angle - wedge * floor(angle / wedge);
    angle = abs(angle - 0.5 * wedge);
    return vec2f(cos(angle), sin(angle)) * length(p);
}

// Where the old paint comes from: turn, fold, step out, enlarge. Read
// backwards, that puts a shrunken copy of the old picture in every wedge.
fn warp(p: vec2f, q: vec2f) -> vec2f {
    let folded = crystal_fold(rotate(q, knob(0u)), knob(1u));
    return (folded - vec2f(knob(2u), 0.0)) * knob(3u);
}

fn sources(p: vec2f, pixel: f32) -> vec3f {
    // The waveform around a ring, mirrored left to right like Tide's.
    let t = abs(atan2(p.x, -p.y)) / PI;
    let x = t * f32(WAVEFORM_POINTS - 1u);
    let i = min(u32(x), WAVEFORM_POINTS - 2u);
    let radius = knob(4u) + knob(5u) * mix(waveform(i), waveform(i + 1u), x - f32(i));
    let d = (length(p) - radius) / max(knob(6u), pixel);
    // A soft ring: wide and blurred, so its small copies stay smooth.
    let coverage = exp(-0.5 * d * d);
    let ink = coverage * knob(7u);
    return vec3f(coverage, ink, 0.0);
}

// Nothing sharp on top: every part of the crystal is a copy.
fn overlay(p: vec2f, pixel: f32) -> vec3f {
    return vec3f(0.0);
}
