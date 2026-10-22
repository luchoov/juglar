// Kaleido: a rose drawn by the spectrum, in a kaleidoscope. Set in
// `kaleido.rs`.
//
// Knobs: 0 rose radius, 1 petal reach, 2 line half-width, 3 share of the
// line the echoes keep, 4 petals (and mirror wedges), 5 line ink on top,
// 6 the rose's turn.

// `p` folded into one mirrored wedge: the angle is taken modulo a wedge and
// reflected about its middle, the distance kept.
fn fold(p: vec2f, wedges: f32) -> vec2f {
    let wedge = TAU / wedges;
    var angle = atan2(p.y, p.x);
    angle = angle - wedge * floor(angle / wedge);
    angle = abs(angle - 0.5 * wedge);
    return vec2f(cos(angle), sin(angle)) * length(p);
}

// The old paint comes from the folded position: every echo is mirrored.
// Folding keeps the distance from the center and lands near the horizontal
// axis, so the screen's corners, further out than its half-width, would fold
// to points beyond its sides and stay black; they reach no further than the
// side instead.
fn warp(p: vec2f, q: vec2f) -> vec2f {
    let folded = fold(q, knob(4u));
    let half_width = 0.49 * frame.resolution.x / min(frame.resolution.x, frame.resolution.y);
    return folded * min(1.0, half_width / max(length(folded), 1e-4));
}

// How much of the pixel at `p` the rose's line covers (x), and where along
// its petal it is, 0 at the middle to 1 at the edges (y).
fn rose(p: vec2f, pixel: f32) -> vec2f {
    let turned = rotate(p, -knob(6u));
    let wedge = TAU / knob(4u);
    var a = atan2(turned.x, -turned.y) / wedge;
    a = a - floor(a);
    // 0 at the wedge's edges, 1 in its middle, mirrored.
    let u = 1.0 - abs(2.0 * a - 1.0);
    // Bass in the middle of each petal, treble at its edges.
    let x = (1.0 - u) * f32(SPECTRUM_BANDS - 1u);
    let i = min(u32(x), SPECTRUM_BANDS - 2u);
    let s0 = spectrum(i);
    let s1 = spectrum(i + 1u);
    let radius = knob(0u) + knob(1u) * mix(s0, s1, x - f32(i));
    // Distance across the curve, as in Tide: steep parts keep their width.
    let per_radian = (s1 - s0) * f32(SPECTRUM_BANDS - 1u) * knob(1u) * 2.0 / wedge;
    let slope = per_radian / max(radius, pixel);
    let distance = abs(length(p) - radius) / sqrt(1.0 + slope * slope);
    let half_width = max(knob(2u), 0.5 * pixel);
    return vec2f(1.0 - smoothstep(half_width - pixel, half_width + pixel, distance), 1.0 - u);
}

fn sources(p: vec2f, pixel: f32) -> vec3f {
    let r = rose(p, pixel * 2.0);
    let coverage = r.x * knob(3u);
    // Colors run along each petal.
    return vec3f(coverage, coverage, coverage * 0.35 * r.y);
}

fn overlay(p: vec2f, pixel: f32) -> vec3f {
    let r = rose(p, pixel);
    let ink = r.x * knob(5u);
    return vec3f(r.x, ink, ink * 0.35 * r.y);
}
