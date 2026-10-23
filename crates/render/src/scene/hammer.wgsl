// Hammer: frames thrown down a tunnel, one at each kick. Set in
// `hammer.rs`.
//
// The spectrum's 32 slots hold up to eight frames in flight, four numbers
// each: how far out it is (in the frame's own shape), how bright, its
// angle, and its shape (0 square, 1 diamond, 2 circle, 3 a cross).
//
// Knobs: 0 the tunnel's pull this frame, 1 wave height on the newest
// frame, 2 line width at the middle, 3 frames in flight.

const HAMMER_FRAMES: u32 = 8u;

// The tunnel's walls: old paint flows outward, faster along the axes than
// the diagonals, so the trails bend into cushions.
fn warp(p: vec2f, q: vec2f) -> vec2f {
    let a = atan2(q.y, q.x);
    return q * (1.0 - knob(0u) * (0.6 + 0.4 * cos(4.0 * a)));
}

// Distance from the middle in a frame's shape.
fn hammer_norm(p: vec2f, shape: u32) -> f32 {
    let a = abs(p);
    switch shape {
        case 1u: { return (a.x + a.y) * 0.7071; }
        case 2u: { return length(p); }
        case 3u: { return min(max(a.x * 0.35, a.y), max(a.x, a.y * 0.35)) * 1.6; }
        default: { return max(a.x, a.y); }
    }
}

// The brightest frame's outline at `p`: (coverage × brightness, its
// position on the palette).
fn hammer_frames(p: vec2f, pixel: f32, thin: f32) -> vec2f {
    var best = vec2f(0.0);
    for (var k = 0u; k < u32(knob(3u)); k++) {
        let size = spectrum(4u * k);
        let ink = spectrum(4u * k + 1u);
        let turned = rotate(p, spectrum(4u * k + 2u));
        let shape = u32(spectrum(4u * k + 3u));
        var edge = size;
        // The newest frame's edge carries the waveform.
        if k == 0u {
            let around = fract(atan2(turned.y, turned.x) / TAU + 0.5);
            let x = abs(2.0 * around - 1.0) * f32(WAVEFORM_POINTS - 1u);
            let i = min(u32(x), WAVEFORM_POINTS - 2u);
            edge += knob(1u) * mix(waveform(i), waveform(i + 1u), x - f32(i));
        }
        let d = abs(hammer_norm(turned, shape) - edge);
        // Nearer frames (further out) are thicker, as in perspective.
        let half = max(thin * (knob(2u) + 0.05 * size), 0.5 * pixel);
        let cover = (1.0 - smoothstep(half - pixel, half + pixel, d)) * ink;
        if cover > best.x {
            best = vec2f(cover, f32(k) * 0.11);
        }
    }
    return best;
}

fn sources(p: vec2f, pixel: f32) -> vec3f {
    let frames = hammer_frames(p, pixel * 2.0, 1.0);
    let coverage = min(frames.x, 1.0) * 0.5;
    let ink = frames.x * 0.6;
    return vec3f(coverage, ink, ink * frames.y);
}

fn overlay(p: vec2f, pixel: f32) -> vec3f {
    let frames = hammer_frames(p, pixel, 0.6);
    let coverage = min(frames.x, 1.0);
    let ink = frames.x * 1.8;
    return vec3f(coverage, ink, ink * frames.y);
}
