// The analysis view (Tab): what Juglar hears, drawn by a single shader.
// Milestone 3's whole picture; now a debugging view next to the scene.
//
// One triangle covers the screen. For every pixel, the GPU runs `fs_analysis`,
// which works out what that pixel shows (part of the waveform, a spectrum
// bar, a band meter, or background) from the numbers in `frame`. Millions of
// pixels run this in parallel; the CPU only uploads ~5 KB per frame.
// `common.wgsl` comes first and defines `frame` and the helpers used here.

// The explorables' palette, as sRGB hex.
const BACKGROUND: u32 = 0x09090bu;
const LINE: u32 = 0x1c1c21u;
const TRACK: u32 = 0x131317u;
const FOREGROUND: u32 = 0xd4d4d8u;
const BASS: u32 = 0x5b7fa6u;
const MID: u32 = 0x5f9488u;
const TREBLE: u32 = 0xa39a78u;
// Beats are drawn dim on purpose: a bright mark that lights up on every beat
// would itself be a flashing light (see docs/research/focus-and-relaxation.md).
const BEAT: u32 = 0x45454du;
const NOVELTY: u32 = 0x8b8b94u;
// The drums are dim for the same reason: a double kick lights its meter
// sixteen times a second. Each takes its region's hue.
const KICK: u32 = 0x3b4a5cu;
const SNARE: u32 = 0x3d5550u;
const CYMBALS: u32 = 0x57533fu;
const PERCUSSIVE: u32 = 0xa07c62u;
const HARMONIC: u32 = 0x7486a8u;

// Spectrum bars below this index are in the bass range (under 250 Hz), from
// it up to `FIRST_TREBLE_BAR` in the mid range (under 4 kHz). The 32 bars are
// log-spaced from 30 Hz to 16 kHz.
const FIRST_MID_BAR: u32 = 11u;
const FIRST_TREBLE_BAR: u32 = 25u;

// sRGB hex to linear light, where blending is physically correct.
fn color(hex: u32) -> vec3f {
    return srgb_to_linear(vec3f(f32((hex >> 16u) & 255u), f32((hex >> 8u) & 255u), f32(hex & 255u)) / 255.0);
}

// How much of the pixel at height `y` (pixel centers sit at .5) lies inside
// a bar whose top edge is at `top`: smooth instead of all-or-nothing, so a
// bar rising by a fraction of a pixel still moves.
fn below(y: f32, top: f32) -> f32 {
    return clamp(y + 0.5 - top, 0.0, 1.0);
}

fn bar_color(i: u32) -> vec3f {
    if i < FIRST_MID_BAR {
        return color(BASS);
    }
    if i < FIRST_TREBLE_BAR {
        return color(MID);
    }
    return color(TREBLE);
}

@fragment
fn fs_analysis(@builtin(position) position: vec4f) -> @location(0) vec4f {
    let p = position.xy; // pixels, from the top-left corner
    let res = frame.resolution;

    let margin = floor(min(res.x, res.y) * 0.07);
    let left = margin;
    let right = res.x - margin;
    let top = margin;
    let bottom = res.y - margin;
    let height = bottom - top;

    var c = color(BACKGROUND);

    // Waveform: the top of the screen.
    let wave_center = floor(top + height * 0.13) + 0.5;
    let wave_amplitude = height * 0.11;
    if p.x >= left && p.x < right {
        if abs(p.y - wave_center) < 0.5 {
            c = color(LINE);
        }
        // Where along the 256 points this pixel column falls, and the line
        // between the two points around it.
        let step = (right - left) / f32(WAVEFORM_POINTS - 1u);
        let t = (p.x - left) / step;
        let i = min(u32(t), WAVEFORM_POINTS - 2u);
        let a = waveform(i);
        let b = waveform(i + 1u);
        let y = wave_center - mix(a, b, t - f32(i)) * wave_amplitude;
        // Distance to a sloped line is the vertical gap shrunk by the slope;
        // without this, steep parts of the wave would look thin.
        let slope = (b - a) * wave_amplitude / step;
        let distance = abs(p.y - y) / sqrt(1.0 + slope * slope);
        c = mix(c, color(FOREGROUND), 1.0 - smoothstep(0.4, 1.4, distance));
    }

    // Spectrum bars and meters: the lower half.
    let bars_top = top + height * 0.5;
    let bars_height = bottom - bars_top;
    let meters_width = floor((right - left) * 0.34);
    let spectrum_right = right - meters_width - floor(margin * 0.5);
    if p.y >= bars_top && p.y < bottom {
        if p.x >= left && p.x < spectrum_right {
            let step = (spectrum_right - left) / f32(SPECTRUM_BANDS);
            let i = min(u32((p.x - left) / step), SPECTRUM_BANDS - 1u);
            let gap = max(1.0, floor(step * 0.25));
            if p.x < floor(left + f32(i + 1u) * step) - gap {
                c = color(TRACK);
                c = mix(c, bar_color(i), below(p.y, bottom - spectrum(i) * bars_height));
            }
        }
        // Eleven meters: bass, mid, treble, energy, beat, novelty; kick,
        // snare, cymbals; percussive, harmonic.
        let meters_left = right - meters_width;
        if p.x >= meters_left && p.x < right {
            let step = meters_width / 11.0;
            let i = min(u32((p.x - meters_left) / step), 10u);
            let gap = max(1.0, floor(step * 0.25));
            if p.x < floor(meters_left + f32(i + 1u) * step) - gap {
                var meter = array<vec3f, 11>(
                    color(BASS), color(MID), color(TREBLE), color(FOREGROUND), color(BEAT), color(NOVELTY),
                    color(KICK), color(SNARE), color(CYMBALS), color(PERCUSSIVE), color(HARMONIC),
                );
                var level = array<f32, 11>(
                    frame.bands.x, frame.bands.y, frame.bands.z, frame.bands.w, frame.beat, frame.novelty,
                    frame.drums.x, frame.drums.y, frame.drums.z, frame.layers.x, frame.layers.y,
                );
                c = color(TRACK);
                c = mix(c, meter[i], below(p.y, bottom - level[i] * bars_height));
            }
        }
    }

    // Timeline between the waveform and the bars, the last two seconds,
    // newest on the right. First the percussive and harmonic levels (two
    // lines), then a lane of dim spikes for each drum.
    if p.x >= left && p.x < right {
        let t = (p.x - left) / (right - left) * f32(HISTORY - 1u);
        let i = min(u32(t), HISTORY - 2u);
        let layers_top = top + height * 0.26;
        let layers_bottom = floor(top + height * 0.32) + 0.5;
        if p.y >= layers_top - 1.0 && p.y < layers_bottom + 1.0 {
            let strip = layers_bottom - layers_top;
            let h = mix(harmonic_history(i), harmonic_history(i + 1u), t - f32(i));
            c = mix(c, color(HARMONIC), 1.0 - smoothstep(0.4, 1.4, abs(p.y - (layers_bottom - h * strip))));
            let q = mix(percussive_history(i), percussive_history(i + 1u), t - f32(i));
            c = mix(c, color(PERCUSSIVE), 1.0 - smoothstep(0.4, 1.4, abs(p.y - (layers_bottom - q * strip))));
            if abs(p.y - layers_bottom) < 0.5 {
                c = max(c, color(LINE));
            }
        }
        var lane_color = array<vec3f, 3>(color(KICK), color(SNARE), color(CYMBALS));
        for (var d = 0u; d < 3u; d++) {
            let lane_top = top + height * (0.335 + 0.03 * f32(d));
            let lane_bottom = floor(top + height * (0.36 + 0.03 * f32(d))) + 0.5;
            if p.y >= lane_top && p.y < lane_bottom + 1.0 {
                let strip = lane_bottom - lane_top;
                c = mix(c, lane_color[d], below(p.y, lane_bottom - drum_history(d, u32(round(t))) * strip));
                if abs(p.y - lane_bottom) < 0.5 {
                    c = max(c, color(LINE));
                }
            }
        }
    }

    // Then beats (dim spikes) and novelty (a line).
    let strip_top = top + height * 0.43;
    let strip_bottom = floor(top + height * 0.47) + 0.5;
    if p.x >= left && p.x < right && p.y >= strip_top && p.y < strip_bottom + 1.0 {
        let t = (p.x - left) / (right - left) * f32(HISTORY - 1u);
        let i = min(u32(t), HISTORY - 2u);
        let strip = strip_bottom - strip_top;
        c = mix(c, color(BEAT), below(p.y, strip_bottom - beat_history(u32(round(t))) * strip));
        let n = mix(novelty_history(i), novelty_history(i + 1u), t - f32(i));
        let distance = abs(p.y - (strip_bottom - n * strip));
        c = mix(c, color(NOVELTY), 1.0 - smoothstep(0.4, 1.4, distance));
        if abs(p.y - strip_bottom) < 0.5 {
            c = max(c, color(LINE));
        }
    }

    // Linear light for the flash limiter, which encodes it for the screen.
    return vec4f(clamp(c, vec3f(0.0), vec3f(1.0)), 1.0);
}
