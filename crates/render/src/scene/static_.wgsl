// Static: the waveform drawn huge across the screen while the picture
// tears into sliding rows and dripping columns. Set in `static_.rs`.
//
// Knobs: 0 row tear this frame, 1 drip this frame, 2 rows, 3 the tear's
// seed, 4 wave height, 5 blocks' seed, 6 blocks' size, 7 ink.

// Each row of old paint slides sideways by its own amount, a new pattern
// every few frames, and each column drips down at its own speed: the
// picture smears into streaks, like a tape chewed by the machine.
fn warp(p: vec2f, q: vec2f) -> vec2f {
    let row = i32(floor((q.y + 1.0) * knob(2u)));
    let seed = i32(knob(3u));
    let slide = grid_random(vec2i(row, seed)) - 0.5;
    let tear = knob(0u) * slide * step(0.45, abs(slide) + 0.2);
    let column = noise(vec2f(q.x * 9.0, 3.7));
    return q - vec2f(tear, knob(1u) * (0.3 + column));
}

// The waveform across the whole width: how much of the pixel at `p` its
// thick line covers.
fn static_wave(p: vec2f, pixel: f32, width: f32) -> f32 {
    let half_width = 0.5 * frame.resolution.x / min(frame.resolution.x, frame.resolution.y);
    let x = clamp(p.x / half_width * 0.5 + 0.5, 0.0, 1.0) * f32(WAVEFORM_POINTS - 1u);
    let i = min(u32(x), WAVEFORM_POINTS - 2u);
    let a = waveform(i);
    let b = waveform(i + 1u);
    let height = knob(4u) * mix(a, b, x - f32(i));
    let slope = (b - a) * knob(4u) * f32(WAVEFORM_POINTS - 1u) / (2.0 * half_width);
    let distance = abs(p.y - height) / sqrt(1.0 + slope * slope);
    let half = max(width, 0.5 * pixel);
    return 1.0 - smoothstep(half - pixel, half + pixel, distance);
}

// Blocks thrown onto the screen at each hit: four rectangles in new places,
// as big as the spectrum's slices are loud. (coverage, palette position)
fn static_blocks(p: vec2f) -> vec2f {
    let seed = i32(knob(5u));
    var best = vec2f(0.0);
    for (var k = 0; k < 4; k++) {
        let center = vec2f(grid_random(vec2i(seed, k)) * 1.6 - 0.8, grid_random(vec2i(k, seed + 7)) - 0.5);
        var level = 0.0;
        for (var j = 0u; j < 8u; j++) {
            level += spectrum(u32(k) * 8u + j);
        }
        let size = knob(6u) * level / 8.0 * vec2f(1.0 + 2.0 * grid_random(vec2i(seed + 3, k)), 0.3 + grid_random(vec2i(k + 11, seed)));
        let d = abs(p - center) - size;
        if max(d.x, d.y) < 0.0 {
            best = vec2f(1.0, f32(k) * 0.25);
        }
    }
    return best;
}

fn sources(p: vec2f, pixel: f32) -> vec3f {
    let wave = static_wave(p, pixel * 2.0, 0.006) * 0.9;
    let blocks = static_blocks(p) * vec2f(0.5, 1.0);
    if blocks.x > wave {
        let ink = knob(7u) * blocks.x;
        return vec3f(blocks.x, ink, ink * (0.5 + blocks.y));
    }
    let ink = knob(7u) * wave;
    return vec3f(wave, ink, ink * 0.1);
}

fn overlay(p: vec2f, pixel: f32) -> vec3f {
    let wave = static_wave(p, pixel, 0.0015);
    let ink = wave * 2.5;
    return vec3f(wave, ink, ink * 0.05);
}
