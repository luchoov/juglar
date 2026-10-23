// Wire: solids drawn as wireframes, their edges computed on the CPU and
// passed in the waveform's 256 slots, four numbers per edge (x0, y0, x1,
// y1, in units of the window's short side). Set in `wire.rs`.
//
// Knobs: 0 edges, 1 line half-width, 2 ink.

// The echoes only move with the frame's zoom and turn.
fn warp(p: vec2f, q: vec2f) -> vec2f {
    return q;
}

// The nearest edge to `p`: (coverage, position on the palette).
fn wire_edges(p: vec2f, pixel: f32, width: f32) -> vec2f {
    var nearest = 1e6;
    var which = 0.0;
    let edges = u32(knob(0u));
    for (var k = 0u; k < edges; k++) {
        let a = vec2f(waveform(4u * k), waveform(4u * k + 1u));
        let b = vec2f(waveform(4u * k + 2u), waveform(4u * k + 3u));
        let ab = b - a;
        let t = clamp(dot(p - a, ab) / max(dot(ab, ab), 1e-8), 0.0, 1.0);
        let d = length(p - a - t * ab);
        if d < nearest {
            nearest = d;
            which = f32(k) / max(f32(edges), 1.0);
        }
    }
    let half = max(width, 0.5 * pixel);
    return vec2f(1.0 - smoothstep(half - pixel, half + pixel, nearest), which);
}

fn sources(p: vec2f, pixel: f32) -> vec3f {
    let edge = wire_edges(p, pixel * 2.0, knob(1u) * 1.5);
    let coverage = edge.x * 0.85;
    let ink = coverage * knob(2u);
    return vec3f(coverage, ink, ink * edge.y);
}

fn overlay(p: vec2f, pixel: f32) -> vec3f {
    let edge = wire_edges(p, pixel, knob(1u));
    let ink = edge.x * (0.6 + knob(2u));
    return vec3f(edge.x, ink, ink * edge.y);
}
