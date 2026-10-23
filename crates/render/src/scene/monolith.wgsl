// Monolith: polygon slabs over clay, leaving contours. Set in
// `monolith.rs`. The ground is drawn as a stencil (`Fx::stencil`): ink
// cuts the clay away to dark, and only dense ink shows as light.
//
// The waveform's slots hold the slabs, eight numbers each: x, y (in units
// of the short side), size (the polygon's inner radius), sides, angle.
//
// Knobs: 0 slabs, 1 contour half-width, 2 contour ink, 3 chambers.

// The contours spread with the frame's zoom.
fn warp(p: vec2f, q: vec2f) -> vec2f {
    return q;
}

// Signed distance from a regular polygon of `sides` sides whose inner
// radius is `size`: negative inside.
fn monolith_polygon(p: vec2f, sides: f32, size: f32) -> f32 {
    let wedge = TAU / sides;
    let a = atan2(p.y, p.x);
    return length(p) * cos(wedge * floor(0.5 + a / wedge) - a) - size;
}

// The slab under or nearest to `p`: (distance, its inner radius).
fn monolith_slab(p: vec2f) -> vec2f {
    var best = vec2f(1e6, 1.0);
    for (var k = 0u; k < u32(knob(0u)); k++) {
        let at = vec2f(waveform(8u * k), waveform(8u * k + 1u));
        let size = waveform(8u * k + 2u);
        let local = rotate(p - at, waveform(8u * k + 4u));
        let d = monolith_polygon(local, waveform(8u * k + 3u), size);
        if d < best.x {
            best = vec2f(d, size);
        }
    }
    return best;
}

// Each slab's outline pressed into the clay; the zoom spreads it out.
fn sources(p: vec2f, pixel: f32) -> vec3f {
    let slab = monolith_slab(p);
    let half = max(knob(1u), pixel);
    let rim = 1.0 - smoothstep(half - pixel, half + pixel, abs(slab.x));
    let ink = rim * knob(2u);
    return vec3f(rim * 0.8, ink, ink * 0.55);
}

// The slabs themselves, crisp: dark stone with a pale rim, and chambers
// carved inside, outline within outline.
fn overlay(p: vec2f, pixel: f32) -> vec3f {
    let slab = monolith_slab(p);
    let inside = 1.0 - smoothstep(-pixel, pixel, slab.x);
    let rim = 1.0 - smoothstep(0.0, 2.5 * pixel, abs(slab.x + 2.0 * pixel));
    // Chambers: rings at even depths inside the slab, as many as asked.
    let chambers = knob(3u);
    let depth = -slab.x / slab.y * (floor(chambers) + 1.0);
    let ring = 1.0 - smoothstep(0.0, 0.08, abs(fract(depth + 0.5) - 0.5));
    let carved = ring * step(0.5, depth) * step(depth, floor(chambers) + 0.5) * min(chambers, 1.0);
    let ink = mix(1.3, 2.4, carved) * inside + 3.0 * rim;
    let coverage = max(inside, rim);
    return vec3f(coverage, ink * coverage, ink * coverage * mix(0.75, 0.3, carved));
}
