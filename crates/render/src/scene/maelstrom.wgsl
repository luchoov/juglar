// Maelstrom: a whirlpool whose rings twist against each other, three blades
// and a ring of the spectrum sinking into it. Set in `maelstrom.rs`.
//
// Knobs: 0 twist this frame, 1 the twist's phase, 2 the blades' angle,
// 3 blade length, 4 spectrum ring radius, 5 bar length, 6 blade ink,
// 7 rings' spacing.

// Each ring of old paint turns by its own amount, one way at some
// distances and the other way at others, a wave traveling inward: the
// whirlpool shears into bands that wind and unwind with the kicks.
fn warp(p: vec2f, q: vec2f) -> vec2f {
    let r = length(q);
    let shear = sin(knob(7u) * r - knob(1u)) * (1.0 - smoothstep(0.3, 0.9, r));
    return rotate(q, knob(0u) * shear);
}

// How much of the pixel at `p` a blade covers: three thick spokes, each as
// bright as its third of the spectrum. (coverage, palette position)
fn maelstrom_blades(p: vec2f, pixel: f32) -> vec2f {
    var best = vec2f(0.0);
    for (var k = 0u; k < 3u; k++) {
        let angle = knob(2u) + f32(k) * TAU / 3.0;
        let along = rotate(p, -angle);
        var level = 0.0;
        for (var j = 0u; j < 10u; j++) {
            level += spectrum(k * 10u + j);
        }
        level /= 10.0;
        let reach = knob(3u) * (0.4 + level);
        let width = 0.004 + 0.012 * level;
        let inside = along.x > 0.02 && along.x < reach;
        let cover = select(0.0, 1.0 - smoothstep(width - pixel, width + pixel, abs(along.y)), inside);
        if cover > best.x {
            best = vec2f(cover, f32(k) / 3.0);
        }
    }
    return best;
}

// The spectrum as bars around a ring, mirrored so it closes.
fn maelstrom_ring(p: vec2f, pixel: f32) -> vec2f {
    let a = atan2(p.y, p.x) - 0.5 * knob(2u);
    let around = fract(a / TAU);
    let x = abs(2.0 * around - 1.0) * f32(SPECTRUM_BANDS - 1u);
    let level = spectrum(min(u32(round(x)), SPECTRUM_BANDS - 1u));
    let r = length(p);
    let inner = knob(4u);
    let outer = inner + knob(5u) * level;
    let cover = smoothstep(inner - pixel, inner + pixel, r) * (1.0 - smoothstep(outer - pixel, outer + pixel, r));
    // Gaps between bars.
    let bar = fract(x + 0.5);
    let gap = smoothstep(0.1, 0.2, bar) * (1.0 - smoothstep(0.8, 0.9, bar));
    return vec2f(cover * gap, 0.6 + 0.4 * x / f32(SPECTRUM_BANDS));
}

fn sources(p: vec2f, pixel: f32) -> vec3f {
    let blades = maelstrom_blades(p, pixel * 2.0);
    let ring = maelstrom_ring(p, pixel * 2.0);
    let pick = select(ring, blades, blades.x > ring.x);
    let coverage = pick.x * 0.8;
    let ink = coverage * knob(6u);
    return vec3f(coverage, ink, ink * pick.y);
}

fn overlay(p: vec2f, pixel: f32) -> vec3f {
    let ring = maelstrom_ring(p, pixel);
    let ink = ring.x * 1.4;
    return vec3f(ring.x * 0.8, ink, ink * ring.y);
}
