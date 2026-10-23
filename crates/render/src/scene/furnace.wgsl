// Furnace: flames of the spectrum rising through a churning, molten mass,
// lit as a surface. Set in `furnace.rs`.
//
// Knobs: 0 churn this frame, 1 rise this frame, 2 bulge this frame, 3 flame
// height, 4 crack ink, 5 the churn's clock, 6 crack height, 7 flame ink.

// The flow that churns the mass: two layers of noise, the coarse one
// moving the most, both drifting with the clock.
fn furnace_flow(q: vec2f) -> vec2f {
    let t = knob(5u);
    let coarse = vec2f(noise(q * 2.3 + vec2f(t, 0.0)), noise(q * 2.3 + vec2f(5.2, -t))) - 0.5;
    let fine = vec2f(noise(q * 7.0 - vec2f(0.0, 2.0 * t)), noise(q * 7.0 + vec2f(3.1 * t, 9.7))) - 0.5;
    return coarse + 0.45 * fine;
}

// Old paint rises, churns, and bulges out from the middle on a kick.
fn warp(p: vec2f, q: vec2f) -> vec2f {
    let bulge = 1.0 - knob(2u) * (1.0 - smoothstep(0.0, 0.7, length(q)));
    return q * bulge + knob(0u) * furnace_flow(q) + vec2f(0.0, knob(1u));
}

// The flames along the bottom: the spectrum mirrored, bass in the middle,
// each column as tall as its band is loud, flickering. (coverage, palette
// position).
fn furnace_flames(p: vec2f) -> vec2f {
    let half_width = 0.5 * frame.resolution.x / min(frame.resolution.x, frame.resolution.y);
    let across = clamp(abs(p.x) / half_width, 0.0, 1.0);
    let x = across * f32(SPECTRUM_BANDS - 1u);
    let i = min(u32(x), SPECTRUM_BANDS - 2u);
    let level = mix(spectrum(i), spectrum(i + 1u), x - f32(i));
    let flicker = 0.6 + 0.8 * noise(vec2f(p.x * 30.0, knob(5u) * 9.0));
    let top = 0.5 - knob(3u) * level * flicker;
    let inside = smoothstep(top - 0.01, top + 0.03, p.y);
    // Yellow at the bottom, orange at the tips.
    return vec2f(inside, 0.2 + 0.45 * clamp((0.5 - p.y) / max(knob(3u), 0.01), 0.0, 1.0));
}

// The crack an impact tears across the middle: the waveform, jagged.
fn furnace_crack(p: vec2f, pixel: f32) -> f32 {
    let half_width = 0.5 * frame.resolution.x / min(frame.resolution.x, frame.resolution.y);
    let x = clamp(p.x / half_width * 0.5 + 0.5, 0.0, 1.0) * f32(WAVEFORM_POINTS - 1u);
    let i = min(u32(x), WAVEFORM_POINTS - 2u);
    let height = knob(6u) * mix(waveform(i), waveform(i + 1u), x - f32(i));
    let distance = abs(p.y - height);
    let half = max(0.004, pixel);
    return 1.0 - smoothstep(half - pixel, half + pixel, distance);
}

fn sources(p: vec2f, pixel: f32) -> vec3f {
    let flames = furnace_flames(p);
    let coverage = flames.x * 0.2;
    var paint = vec3f(coverage, coverage * knob(7u), coverage * knob(7u) * flames.y);
    let crack = furnace_crack(p, pixel * 2.0) * knob(4u);
    if crack > paint.x {
        paint = vec3f(crack, crack * 2.0, crack * 0.2);
    }
    return paint;
}

fn overlay(p: vec2f, pixel: f32) -> vec3f {
    let crack = furnace_crack(p, pixel) * knob(4u);
    return vec3f(crack, crack * 2.0, crack * 0.1);
}
