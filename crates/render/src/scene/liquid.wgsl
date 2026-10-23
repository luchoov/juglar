// Liquid: a ribbon of the waveform and three blobs poured into paint that
// never fades, stirred by curl-noise currents. Set in `liquid.rs`.
//
// Knobs: 0 the currents' push this frame, 1 where the currents are, 2
// eddies per screen, 3 the ribbon's and blobs' turn, 4 the ribbon's
// height, 5 paint poured per frame, 6 the blobs' orbit.

// The currents carry the old paint along; nothing fades, so the picture
// is all marble.
fn warp(p: vec2f, q: vec2f) -> vec2f {
    return q - knob(0u) * flow(q, knob(2u), knob(1u));
}

fn sources(p: vec2f, pixel: f32) -> vec3f {
    let poured = knob(5u);
    // The ribbon: the waveform across the screen, turned with the snare.
    let turned = rotate(p, knob(3u));
    let x = clamp(turned.x * 0.6 + 0.5, 0.0, 1.0) * f32(WAVEFORM_POINTS - 1u);
    let i = min(u32(x), WAVEFORM_POINTS - 2u);
    let wave = knob(4u) * mix(waveform(i), waveform(i + 1u), x - f32(i));
    let ribbon = 1.0 - smoothstep(0.004, 0.012 + 2.0 * pixel, abs(turned.y - wave));
    var paint = vec3f(ribbon, ribbon * 1.4, ribbon * 1.4 * fract(turned.x * 0.5 + 0.25));
    // Three blobs on slow orbits, each its own color.
    for (var k = 0u; k < 3u; k++) {
        let a = knob(6u) * (0.4 + 0.13 * f32(k)) + TAU * f32(k) / 3.0;
        let at = vec2f(0.55 * cos(a), 0.32 * sin(1.3 * a));
        let blob = (1.0 - smoothstep(0.03, 0.07, length(p - at))) * poured;
        let color = f32(k) / 3.0 + 0.1;
        paint = paint * (1.0 - blob) + vec3f(blob, blob * 1.2, blob * 1.2 * color);
    }
    return paint;
}

fn overlay(p: vec2f, pixel: f32) -> vec3f {
    return vec3f(0.0);
}
