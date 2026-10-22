// Vortex: rings turning against each other, a ripple, five circling
// lights and a line. Set in `vortex.rs`.
//
// Knobs: 0 the rings' turn this frame, 1 the ripple's push this frame,
// 2 how far the ripple has traveled, 3 the rings' pattern, 4 the lights'
// angle (the line turns the other way), 5 wave height on the line, 6 the
// lights' circle.

const VORTEX_LIGHTS: u32 = 5u;

// Each ring turns by its own amount, one way or the other, and the ripple
// moves it in or out: a warp that changes with the distance from the middle.
fn warp(p: vec2f, q: vec2f) -> vec2f {
    let r = length(q);
    let pattern = knob(3u);
    // Two slow waves across the radius, drifting: which rings turn which way.
    let turn = sin(r * 9.0 + pattern) * 0.7 + sin(r * 4.0 - 1.3 * pattern) * 0.3;
    // Both fade out toward the corners, which would otherwise turn in
    // paint from beyond the image's edges (black).
    let edge = 1.0 - smoothstep(0.4, 0.9, r);
    let turned = rotate(q, knob(0u) * turn * edge);
    // A ripple of zoom traveling outward.
    let push = 1.0 - knob(1u) * edge * sin(r * 14.0 - knob(2u));
    return turned * push;
}

// The five lights' paint: (coverage, ink × coverage, ink × palette
// position × coverage). Each light is as big as its slice of the spectrum
// is loud, and has its own color.
fn vortex_lights(p: vec2f) -> vec3f {
    var paint = vec3f(0.0);
    for (var i = 0u; i < VORTEX_LIGHTS; i++) {
        let angle = knob(4u) + f32(i) * TAU / f32(VORTEX_LIGHTS);
        let center = knob(6u) * vec2f(cos(angle), sin(angle));
        var level = 0.0;
        for (var j = 0u; j < 6u; j++) {
            level += spectrum(min(i * 6u + j, SPECTRUM_BANDS - 1u));
        }
        let size = 0.005 + 0.008 * level / 6.0;
        let d = length(p - center);
        let coverage = 0.3 * exp(-0.5 * d * d / (size * size));
        if coverage > paint.x {
            let ink = 1.3;
            paint = vec3f(coverage, ink * coverage, ink * f32(i) * 0.2 * coverage);
        }
    }
    return paint;
}

// How much of the pixel at `p` the line covers: the waveform drawn across
// the middle, turned against the lights.
fn vortex_line(p: vec2f, pixel: f32) -> f32 {
    let along = rotate(p, 0.6 * knob(4u));
    let half_length = 0.07;
    let x = clamp(along.x / half_length * 0.5 + 0.5, 0.0, 1.0) * f32(WAVEFORM_POINTS - 1u);
    let i = min(u32(x), WAVEFORM_POINTS - 2u);
    let a = waveform(i);
    let b = waveform(i + 1u);
    let height = knob(5u) * 0.4 * mix(a, b, x - f32(i));
    // Distance across the curve, so steep parts keep their width; and the
    // line's ends fade, so it has no hard tips.
    let slope = (b - a) * knob(5u) * 0.4 * f32(WAVEFORM_POINTS - 1u) / (2.0 * half_length);
    let distance = abs(along.y - height) / sqrt(1.0 + slope * slope);
    let half_width = max(0.0018, 0.5 * pixel);
    let ends = 1.0 - smoothstep(0.7, 1.0, abs(along.x) / half_length);
    return (1.0 - smoothstep(half_width - pixel, half_width + pixel, distance)) * ends;
}

fn sources(p: vec2f, pixel: f32) -> vec3f {
    let lights = vortex_lights(p);
    let line = vortex_line(p, pixel * 2.0) * 0.05;
    return select(lights, vec3f(line, line, line * 0.5), line > lights.x);
}

fn overlay(p: vec2f, pixel: f32) -> vec3f {
    let coverage = vortex_line(p, pixel);
    let ink = coverage * 1.4;
    return vec3f(coverage, ink, ink * 0.5);
}
