// Orbits: nine soft lights on tilted orbits, three per ring. Set in
// `orbits.rs`.
//
// Knobs: 0-2 each ring's angle, 3-5 each ring's level (bass, mid, treble),
// 6 orbit size, 7 tilt.

// Where light `j` of ring `k` is, from the center, in short-side units.
fn light(k: u32, j: u32) -> vec2f {
    let radius = (0.1 + 0.11 * f32(k)) * knob(6u);
    let angle = knob(k) + f32(j) * TAU / 3.0 + f32(k) * 0.7;
    // An ellipse, as a circle seen at an angle, each ring tilted its own way.
    let ellipse = vec2f(cos(angle), 0.55 * sin(angle)) * radius;
    return rotate(ellipse, knob(7u) + f32(k) * 1.05);
}

// The nearest light's paint: (coverage, ink × coverage, ink × palette
// position × coverage), for spots of the given size.
fn lights(p: vec2f, size: f32, ink_scale: f32) -> vec3f {
    var paint = vec3f(0.0);
    for (var k = 0u; k < 3u; k++) {
        let level = knob(3u + k);
        for (var j = 0u; j < 3u; j++) {
            let d = length(p - light(k, j));
            let coverage = exp(-0.5 * d * d / (size * size));
            if coverage > paint.x {
                let ink = ink_scale * (0.5 + 1.5 * level);
                // Each ring its own color, each light a little apart.
                let position = f32(k) * 0.33 + f32(j) * 0.06;
                paint = vec3f(coverage, ink * coverage, ink * position * coverage);
            }
        }
    }
    return paint;
}

fn warp(p: vec2f, q: vec2f) -> vec2f {
    return q;
}

fn sources(p: vec2f, pixel: f32) -> vec3f {
    // Fainter and a little wider into the trails, so the arcs are soft.
    return lights(p, 0.006, 1.0) * 0.5;
}

fn overlay(p: vec2f, pixel: f32) -> vec3f {
    // A small, bright core on top.
    return lights(p, max(0.004, pixel), 1.3) * 0.9;
}
