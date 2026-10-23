// Furnace: a wall of fire fed from the bottom edge, its tongues the
// spectrum, rising through turbulence. Set in `furnace.rs`.
//
// Knobs: 0 turbulence this frame, 1 rise this frame, 2 bulge this frame,
// 3 flame height, 4 crack ink, 5 the turbulence's clock, 6 crack height,
// 7 flame ink.

// Old paint rises (more slowly as it climbs, so the fire piles into
// billows), tossed by curl turbulence, and bulges out on a kick.
fn warp(p: vec2f, q: vec2f) -> vec2f {
    let bulge = 1.0 - knob(2u) * (1.0 - smoothstep(0.0, 0.7, length(q)));
    let climb = knob(1u) * (1.2 - 0.6 * smoothstep(-0.5, 0.5, -q.y));
    return q * bulge - knob(0u) * flow(q, 3.0, knob(5u)) + vec2f(0.0, climb);
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
    // A bed of fire all along the bottom, the spectrum's tongues above it.
    let top = 0.5 - 0.06 - knob(3u) * level * flicker;
    // Tongues apart: noise along the bottom, moving, leaves dark gaps.
    let tongues = smoothstep(0.35, 0.65, noise(vec2f(p.x * 9.0, knob(5u) * 4.0)));
    let inside = smoothstep(top - 0.01, top + 0.03, p.y) * (0.25 + 0.75 * tongues);
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
    let coverage = flames.x * 0.18;
    var paint = vec3f(coverage, coverage * knob(7u), coverage * knob(7u) * flames.y);
    // Embers: sparks in the lower half, new ones every frame, that the
    // rise carries up.
    let cell = vec2i(floor(p / (3.0 * pixel)));
    let seed = i32(knob(5u) * 40.0);
    let ember = step(0.9985, grid_random(cell + vec2i(seed * 7919, seed))) * smoothstep(-0.1, 0.4, p.y);
    paint = paint * (1.0 - ember) + vec3f(ember, ember * 1.5, ember * 1.5 * 0.1);
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
