// Datamosh: bars of the spectrum smeared by blocks that each move their own
// way, as a broken video stream does. Set in `datamosh.rs`.
//
// Knobs: 0 block drift this frame, 1 blocks across the short side, 2 the
// blocks' seed, 3 bar ink, 4 the bars' scroll, 5 drip this frame, 6 bar
// coverage per frame.

// Each block of old paint moves by its own vector, chosen anew at every
// hit: the picture slides apart along the block edges, the way a video
// that lost its key frames keeps moving old pixels with new motion.
fn warp(p: vec2f, q: vec2f) -> vec2f {
    let block = vec2i(floor(q * knob(1u)));
    let seed = i32(knob(2u));
    let angle = TAU * grid_random(block + vec2i(seed, 3 * seed));
    let strength = grid_random(block + vec2i(7 * seed, seed + 11));
    // Most blocks hold still; a few slide hard.
    let moving = select(0.0, 1.0, strength > 0.55);
    let drift = vec2f(cos(angle), sin(angle)) * knob(0u) * moving * (strength - 0.4);
    // Bright columns drip down, like sorted pixels.
    let column = grid_random(vec2i(i32(floor(q.x * 60.0)), seed));
    let drip = select(0.0, knob(5u), column > 0.8);
    return q - drift - vec2f(0.0, drip);
}

// The spectrum as a barcode across the whole screen, scrolling sideways:
// each band a column of the full height, its bar as wide as the band is
// loud, its color the band's. (coverage, palette position)
fn datamosh_bars(p: vec2f) -> vec2f {
    let half_width = 0.5 * frame.resolution.x / min(frame.resolution.x, frame.resolution.y);
    let x = fract((p.x / half_width * 0.5 + 0.5) + knob(4u)) * f32(SPECTRUM_BANDS);
    let band = min(u32(x), SPECTRUM_BANDS - 1u);
    let level = spectrum(band);
    let inside = step(fract(x), level * level);
    // Colors go around the palette three times across the screen.
    return vec2f(inside, fract(3.0 * f32(band) / f32(SPECTRUM_BANDS)));
}

fn sources(p: vec2f, pixel: f32) -> vec3f {
    let bars = datamosh_bars(p);
    let coverage = bars.x * knob(6u);
    let ink = coverage * knob(3u);
    return vec3f(coverage, ink, ink * bars.y);
}

fn overlay(p: vec2f, pixel: f32) -> vec3f {
    return vec3f(0.0);
}
