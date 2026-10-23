// Shrapnel: a barbed ring drawn by the waveform, blown outward through a
// kaleidoscope. Set in `shrapnel.rs`.
//
// Knobs: 0 ring radius, 1 wave height, 2 barbs around the ring, 3 barb
// length, 4 the ring's turn, 5 mirror wedges (0 = no fold), 6 serration
// this frame, 7 ink.

// `p` folded into one mirrored wedge, as in Kaleido.
fn shrapnel_fold(p: vec2f, wedges: f32) -> vec2f {
    let wedge = TAU / wedges;
    var angle = atan2(p.y, p.x);
    angle = angle - wedge * floor(angle / wedge);
    angle = abs(angle - 0.5 * wedge);
    return vec2f(cos(angle), sin(angle)) * length(p);
}

// The old paint is blown outward unevenly: each barb's direction a little
// faster than the gaps between them, so the echoes tear into teeth. Then
// folded into mirror wedges, if any.
fn warp(p: vec2f, q: vec2f) -> vec2f {
    let angle = atan2(q.y, q.x) - knob(4u);
    let teeth = cos(knob(2u) * angle);
    var r = q * (1.0 - knob(6u) * teeth * smoothstep(0.02, 0.3, length(q)));
    if knob(5u) > 0.5 {
        r = shrapnel_fold(r, knob(5u));
        let half_width = 0.49 * frame.resolution.x / min(frame.resolution.x, frame.resolution.y);
        r = r * min(1.0, half_width / max(length(r), 1e-4));
    }
    return r;
}

// The ring's radius at angle `a` (already turned).
fn shrapnel_radius(a: f32) -> f32 {
    // Around the ring and back, so the waveform's ends meet.
    let around = fract(a / TAU);
    let x = abs(2.0 * around - 1.0) * f32(WAVEFORM_POINTS - 1u);
    let i = min(u32(x), WAVEFORM_POINTS - 2u);
    let wave = mix(waveform(i), waveform(i + 1u), x - f32(i));
    // Barbs: narrow spikes, sharpened by a power.
    let barb = pow(max(cos(knob(2u) * a), 0.0), 12.0);
    return knob(0u) + knob(1u) * wave + knob(3u) * barb;
}

// How much of the pixel at `p` the ring covers, and where around it the
// pixel is (0..1).
fn shrapnel_ring(p: vec2f, pixel: f32) -> vec2f {
    let a = atan2(p.y, p.x) - knob(4u);
    let r = length(p);
    let radius = shrapnel_radius(a);
    // The slope, from the radius a little either side: steep barbs keep
    // their width.
    let step = 0.004;
    let slope = (shrapnel_radius(a + step) - shrapnel_radius(a - step)) / (2.0 * step) / max(radius, 0.01);
    let distance = abs(r - radius) / sqrt(1.0 + slope * slope);
    let half_width = max(0.0022, 0.6 * pixel);
    let coverage = 1.0 - smoothstep(half_width - pixel, half_width + pixel, distance);
    return vec2f(coverage, fract(a / TAU));
}

fn sources(p: vec2f, pixel: f32) -> vec3f {
    let ring = shrapnel_ring(p, pixel * 2.0);
    let coverage = ring.x * 0.9;
    let ink = knob(7u) * coverage;
    // Colors run around the ring, mirrored, so its two halves match.
    return vec3f(coverage, ink, ink * abs(2.0 * ring.y - 1.0));
}

fn overlay(p: vec2f, pixel: f32) -> vec3f {
    let ring = shrapnel_ring(p, pixel);
    let ink = ring.x * (1.2 + knob(7u));
    return vec3f(ring.x, ink, ink * 0.1);
}
