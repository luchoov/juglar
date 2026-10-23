// Fracture: territories of flat color around seeds, split by hard seams.
// Set in `fracture.rs`.
//
// The waveform's slots hold the seeds, four numbers each: x, y (in units
// of the short side), color on the palette, and glow (1 when the seed was
// just flung in, falling to 0).
//
// Knobs: 0 seeds, 1 slide this frame, 2 the slide's seed, 3 fill per
// frame, 4 seam half-width, 5 the territories' ink, 6 bright seams.

fn fracture_seed(k: u32) -> vec2f {
    return vec2f(waveform(4u * k), waveform(4u * k + 1u));
}

// The nearest seed and the second nearest, and how far `p` is from the
// seam between them (the line halfway, square to the two): (nearest,
// second, distance to the seam).
fn fracture_cells(p: vec2f) -> vec3f {
    var first = 1e6;
    var second = 1e6;
    var a = 0u;
    var b = 0u;
    for (var k = 0u; k < u32(knob(0u)); k++) {
        let d = distance(p, fracture_seed(k));
        if d < first {
            second = first;
            b = a;
            first = d;
            a = k;
        } else if d < second {
            second = d;
            b = k;
        }
    }
    let sa = fracture_seed(a);
    let sb = fracture_seed(b);
    let across = sb - sa;
    let seam = dot(0.5 * (sa + sb) - p, across / max(length(across), 1e-5));
    return vec3f(f32(a), f32(b), select(1e6, abs(seam), second < 1e5));
}

// Each territory's old paint slides its own way at a kick; most only a
// little, a few far, so the picture shears along the seams.
fn warp(p: vec2f, q: vec2f) -> vec2f {
    let cell = i32(fracture_cells(q).x);
    let seed = i32(knob(2u));
    let angle = TAU * grid_random(vec2i(cell, seed));
    let far = grid_random(vec2i(seed + 5, cell));
    return q - vec2f(cos(angle), sin(angle)) * knob(1u) * far * far;
}

// A territory's paint at `p`, and its seam: (coverage, ink × coverage,
// ink × position × coverage).
fn fracture_paint(p: vec2f, pixel: f32, fill: f32) -> vec3f {
    let cells = fracture_cells(p);
    let k = u32(cells.x);
    let color = waveform(4u * k + 2u);
    let glow = waveform(4u * k + 3u);
    // Each territory a shade of its own, so neighbors of one color part.
    let shade = 0.7 + 0.3 * grid_random(vec2i(i32(k), 91));
    let lit_seams = knob(6u) > 0.5;
    let ink = knob(5u) * shade * select(1.0, 0.35, lit_seams) + 2.5 * glow;
    let cover = min(fill + glow, 1.0);
    let half = max(knob(4u), 0.5 * pixel);
    let seam = 1.0 - smoothstep(half - pixel, half + pixel, cells.z);
    // Seams are dark lead, or light between the darkened territories.
    let seam_ink = select(0.0, 3.0, lit_seams);
    let seam_color = color + 0.5;
    let coverage = seam + cover * (1.0 - seam);
    let paint = seam_ink * seam + ink * cover * (1.0 - seam);
    let colored = seam_ink * seam_color * seam + ink * color * cover * (1.0 - seam);
    return vec3f(coverage, paint, colored);
}

fn sources(p: vec2f, pixel: f32) -> vec3f {
    return fracture_paint(p, pixel * 2.0, knob(3u));
}

// Crisp seams over the smeared territories.
fn overlay(p: vec2f, pixel: f32) -> vec3f {
    let cells = fracture_cells(p);
    let half = max(knob(4u), 0.5 * pixel);
    let seam = 1.0 - smoothstep(half - pixel, half + pixel, cells.z);
    let lit_seams = knob(6u) > 0.5;
    let ink = select(0.0, 3.0, lit_seams);
    let color = waveform(4u * u32(cells.x) + 2u) + 0.5;
    return vec3f(seam, ink * seam, ink * color * seam);
}
