// Harmonograph: a figure of 128 points, computed in `harmonograph.rs` and
// passed in the waveform's slots as x, y pairs (short-side units).
//
// Knobs: 0 line half-width, 1 share of the line the trails keep, 3 palette
// positions along the figure.

const FIGURE_POINTS: u32 = 128u;

fn figure_point(i: u32) -> vec2f {
    return vec2f(waveform(2u * i), waveform(2u * i + 1u));
}

// How much of the pixel at `p` the line covers (x), and how far along the
// figure the nearest part is, 0 to 1 (y).
fn figure(p: vec2f, pixel: f32) -> vec2f {
    var nearest = 1e9;
    var along = 0.0;
    var a = figure_point(0u);
    for (var i = 1u; i < FIGURE_POINTS; i++) {
        let b = figure_point(i);
        let ab = b - a;
        let h = clamp(dot(p - a, ab) / max(dot(ab, ab), 1e-9), 0.0, 1.0);
        let d = length(p - a - ab * h);
        if d < nearest {
            nearest = d;
            along = (f32(i - 1u) + h) / f32(FIGURE_POINTS - 1u);
        }
        a = b;
    }
    let half_width = max(knob(0u), 0.5 * pixel);
    return vec2f(1.0 - smoothstep(half_width - pixel, half_width + pixel, nearest), along);
}

fn warp(p: vec2f, q: vec2f) -> vec2f {
    return q;
}

fn sources(p: vec2f, pixel: f32) -> vec3f {
    let f = figure(p, pixel);
    let coverage = f.x * knob(1u);
    return vec3f(coverage, 1.6 * coverage, 1.6 * coverage * knob(3u) * f.y);
}

// Nothing on top: the line lives in the trails only (see `harmonograph.rs`).
fn overlay(p: vec2f, pixel: f32) -> vec3f {
    return vec3f(0.0);
}
