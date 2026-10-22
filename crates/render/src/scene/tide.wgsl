// Tide: what the scene paints. Its motion and colors are set in `tide.rs`.
//
// A ring whose edge is the waveform, mirrored left to right so its two ends
// meet at the top and bottom. Each frame it is painted twice:
//
// - faintly into the feedback image (`sources`), where the warp carries it
//   outward and fades it, leaving a tunnel of its own past shapes;
// - sharply on top of the final picture (`overlay`), at full resolution,
//   so the present moment stays crisp in front of its blurred history.
//
// Knobs (set in `tide.rs`):
//   0 ring radius            1 wave height on the ring
//   2 line half-width        3 how much of the line goes into the trails
//   4 line palette position  5 line ink on top
//   6 how much cooler the trails start than the line
// Lengths are in units of the window's short side.

// How much of the pixel at `p` the ring's line covers, 0..1. `p` is the
// position from the warp center and `pixel` one pixel, both in short-side
// units.
fn ring(p: vec2f, pixel: f32) -> f32 {
    let r = length(p);
    // The angle from the top, 0..1 on either side, picks the waveform point.
    let t = abs(atan2(p.x, -p.y)) / PI;
    let x = t * f32(WAVEFORM_POINTS - 1u);
    let i = min(u32(x), WAVEFORM_POINTS - 2u);
    let a = waveform(i);
    let b = waveform(i + 1u);
    let radius = knob(0u) + knob(1u) * mix(a, b, x - f32(i));
    // Distance across the curve, not just along the radius, so steep parts
    // of the wave keep the same thickness (as in the analysis view).
    let radius_per_radian = (b - a) * f32(WAVEFORM_POINTS - 1u) * knob(1u) / PI;
    // Measured at the curve's own radius: near the center, the pixel's
    // radius is tiny and would make every slope look vertical.
    let slope = radius_per_radian / max(radius, pixel);
    let distance = abs(r - radius) / sqrt(1.0 + slope * slope);
    let half_width = max(knob(2u), 0.5 * pixel);
    return 1.0 - smoothstep(half_width - pixel, half_width + pixel, distance);
}

// Paint for the feedback image: (coverage, ink × coverage, ink × palette
// position × coverage), see `feedback.wgsl`.
fn sources(p: vec2f, pixel: f32) -> vec3f {
    let coverage = ring(p, pixel) * knob(3u);
    return vec3f(coverage, coverage, coverage * (knob(4u) + knob(6u)));
}

// Paint on top of the final picture, in the same form.
fn overlay(p: vec2f, pixel: f32) -> vec3f {
    let coverage = ring(p, pixel);
    let ink = coverage * knob(5u);
    return vec3f(coverage, ink, ink * knob(4u));
}
