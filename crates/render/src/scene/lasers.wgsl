// Lasers: fans of straight beams from a few emitters. Set in `lasers.rs`.
//
// The waveform's slots hold the emitters, eight numbers each: x, y (in
// units of the short side), the fan's direction and width (radians), and
// its number of beams.
//
// Knobs: 0 emitters, 1 beam half-width at the emitter, 2 beam ink, 3 which
// beams drop out, 4 how many drop out.

// The afterimages only move with the frame's zoom.
fn warp(p: vec2f, q: vec2f) -> vec2f {
    return q;
}

// The nearest beam of every fan at `p`: (coverage × ink, palette position).
fn lasers_beams(p: vec2f, pixel: f32, thin: f32) -> vec2f {
    var best = vec2f(0.0);
    for (var e = 0u; e < u32(knob(0u)); e++) {
        let at = vec2f(waveform(8u * e), waveform(8u * e + 1u));
        let direction = waveform(8u * e + 2u);
        let spread = waveform(8u * e + 3u);
        let beams = max(waveform(8u * e + 4u), 2.0);
        let d = p - at;
        let r = length(d);
        // The angle from the fan's first beam, `-PI..PI` around its middle.
        var off = atan2(d.y, d.x) - direction;
        off = off - TAU * floor(off / TAU + 0.5);
        // A full circle has no first and last beam: they would overlap.
        let full = spread > 6.2;
        let step = select(spread / (beams - 1.0), TAU / beams, full);
        let start = select(-0.5 * spread, 0.0, full);
        var index: f32;
        var along: f32;
        if full {
            // Around the circle, the nearest beam may be on either side of
            // the first; its number wraps for its color and flicker.
            let nearest = round(off / step);
            along = off - nearest * step;
            index = nearest - beams * floor(nearest / beams);
        } else {
            index = clamp(round((off - start) / step), 0.0, beams - 1.0);
            along = off - (start + index * step);
        }
        // Distance to the beam's line, on the emitter's side only.
        let dist = select(1e6, r * abs(sin(along)), abs(along) < 1.5);
        // Beams widen as they travel, like light through haze.
        let half = max(thin * knob(1u) * (1.0 + 1.5 * r), 0.5 * pixel);
        let core = 1.0 - smoothstep(half - pixel, half + pixel, dist);
        let out = grid_random(vec2i(i32(index) + 17 * i32(e), i32(knob(3u)))) < knob(4u);
        let ink = select(knob(2u), 0.0, out) * (1.0 - 0.3 * min(r, 1.0));
        if core * ink > best.x {
            // The beams' colors run from pale gold to warm white along the
            // fan, where the palette is brightest in all three channels.
            best = vec2f(core * ink, 0.76 + 0.06 * index / beams);
        }
    }
    return best;
}

fn sources(p: vec2f, pixel: f32) -> vec3f {
    let beam = lasers_beams(p, pixel * 2.0, 1.6);
    let coverage = min(beam.x, 1.0) * 0.6;
    let ink = beam.x * 0.5;
    return vec3f(coverage, ink, ink * beam.y);
}

fn overlay(p: vec2f, pixel: f32) -> vec3f {
    let beam = lasers_beams(p, pixel, 1.0);
    let coverage = min(beam.x, 1.0);
    return vec3f(coverage, beam.x, beam.x * beam.y);
}
