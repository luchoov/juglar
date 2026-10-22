// Milestone 3: the analysis drawn by a single shader.
//
// One triangle covers the screen. For every pixel, the GPU runs `fs_main`,
// which works out what that pixel shows (part of the waveform, a spectrum
// bar, a band meter, or background) from the numbers in `frame`. Millions of
// pixels run this in parallel; the CPU only uploads ~1 KB per frame.

struct Frame {
    resolution: vec2f,
    time: f32,
    // 1.0 when the screen format doesn't encode to sRGB by itself.
    encode_srgb: f32,
    // x = bass, y = mid, z = treble, w = energy.
    bands: vec4f,
    // Four values per row: GPU uniform memory is laid out in 16-byte rows.
    spectrum: array<vec4f, 8>,
    waveform: array<vec4f, 64>,
}

@group(0) @binding(0) var<uniform> frame: Frame;

const SPECTRUM_BANDS: u32 = 32u;
const WAVEFORM_POINTS: u32 = 256u;

// The explorables' palette, as sRGB hex.
const BACKGROUND: u32 = 0x09090bu;
const LINE: u32 = 0x1c1c21u;
const TRACK: u32 = 0x131317u;
const FOREGROUND: u32 = 0xd4d4d8u;
const BASS: u32 = 0x5b7fa6u;
const MID: u32 = 0x5f9488u;
const TREBLE: u32 = 0xa39a78u;

// Spectrum bars below this index are in the bass range (under 250 Hz), from
// it up to `FIRST_TREBLE_BAR` in the mid range (under 4 kHz). The 32 bars are
// log-spaced from 30 Hz to 16 kHz.
const FIRST_MID_BAR: u32 = 11u;
const FIRST_TREBLE_BAR: u32 = 25u;

@vertex
fn vs_main(@builtin(vertex_index) i: u32) -> @builtin(position) vec4f {
    // Corners (-1,-1), (3,-1), (-1,3): a triangle twice the size of the
    // screen, whose visible part is exactly the screen. Cheaper than two
    // triangles, which would shade the pixels along their shared edge twice.
    let corner = vec2f(f32((i << 1u) & 2u), f32(i & 2u));
    return vec4f(corner * 2.0 - 1.0, 0.0, 1.0);
}

// sRGB hex to linear light, where blending is physically correct.
fn color(hex: u32) -> vec3f {
    let c = vec3f(f32((hex >> 16u) & 255u), f32((hex >> 8u) & 255u), f32(hex & 255u)) / 255.0;
    return select(pow((c + 0.055) / 1.055, vec3f(2.4)), c / 12.92, c <= vec3f(0.04045));
}

fn linear_to_srgb(c: vec3f) -> vec3f {
    return select(1.055 * pow(c, vec3f(1.0 / 2.4)) - 0.055, c * 12.92, c <= vec3f(0.0031308));
}

fn spectrum(i: u32) -> f32 {
    return frame.spectrum[i / 4u][i % 4u];
}

fn waveform(i: u32) -> f32 {
    return frame.waveform[i / 4u][i % 4u];
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
fn fs_main(@builtin(position) position: vec4f) -> @location(0) vec4f {
    let p = position.xy; // pixels, from the top-left corner
    let res = frame.resolution;

    let margin = floor(min(res.x, res.y) * 0.07);
    let left = margin;
    let right = res.x - margin;
    let top = margin;
    let bottom = res.y - margin;
    let height = bottom - top;

    var c = color(BACKGROUND);

    // Waveform: the upper part of the screen.
    let wave_center = floor(top + height * 0.2) + 0.5;
    let wave_amplitude = height * 0.18;
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

    // Spectrum bars and band meters: the lower half.
    let bars_top = top + height * 0.5;
    let bars_height = bottom - bars_top;
    let meters_width = floor((right - left) * 0.14);
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
        let meters_left = right - meters_width;
        if p.x >= meters_left && p.x < right {
            let step = meters_width / 4.0;
            let i = min(u32((p.x - meters_left) / step), 3u);
            let gap = max(1.0, floor(step * 0.25));
            if p.x < floor(meters_left + f32(i + 1u) * step) - gap {
                var meter = array<vec3f, 4>(color(BASS), color(MID), color(TREBLE), color(FOREGROUND));
                c = color(TRACK);
                c = mix(c, meter[i], below(p.y, bottom - frame.bands[i] * bars_height));
            }
        }
    }

    if frame.encode_srgb > 0.5 {
        c = linear_to_srgb(c);
    }
    return vec4f(c, 1.0);
}
