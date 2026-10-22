// Shared by every shader: the frame's numbers and a few helpers. Each
// shader module is this file followed by its own.

struct Frame {
    // Pixels of the window, and of the feedback images (half as many).
    resolution: vec2f,
    feedback_size: vec2f,
    time: f32,
    dt: f32,
    // 1.0 when the screen format doesn't encode to sRGB by itself.
    encode_srgb: f32,
    // Changes every frame, so the dithering noise does too.
    seed: f32,

    // The warp, as this frame's step: where the old picture moves to.
    // Lengths are in units of the window's short side.
    center: vec2f,
    zoom: f32,
    rotation: f32,
    swirl: f32,
    decay: f32,
    // How far older ink moves along the palette.
    palette_drift: f32,
    _pad0: f32,

    // Eight numbers the scene's sources use as they like.
    knobs: array<vec4f, 2>,

    // Cosine palette: color(t) = a + b * cos(2π (c t + d)), in sRGB.
    palette_a: vec4f,
    palette_b: vec4f,
    palette_c: vec4f,
    palette_d: vec4f,
    palette_shift: f32,
    exposure: f32,
    // 1 at a beat, falling to 0; and how much the music is changing.
    beat: f32,
    novelty: f32,

    // x = bass, y = mid, z = treble, w = energy.
    bands: vec4f,
    // Four values per row: GPU uniform memory is laid out in 16-byte rows.
    spectrum: array<vec4f, 8>,
    waveform: array<vec4f, 64>,
    // The last 128 frames of beat and novelty, oldest first.
    beat_history: array<vec4f, 32>,
    novelty_history: array<vec4f, 32>,
}

@group(0) @binding(0) var<uniform> frame: Frame;

const PI: f32 = 3.14159265;
const TAU: f32 = 6.28318531;
const SPECTRUM_BANDS: u32 = 32u;
const WAVEFORM_POINTS: u32 = 256u;

@vertex
fn vs_main(@builtin(vertex_index) i: u32) -> @builtin(position) vec4f {
    // Corners (-1,-1), (3,-1), (-1,3): a triangle twice the size of the
    // screen, whose visible part is exactly the screen. Cheaper than two
    // triangles, which would shade the pixels along their shared edge twice.
    let corner = vec2f(f32((i << 1u) & 2u), f32(i & 2u));
    return vec4f(corner * 2.0 - 1.0, 0.0, 1.0);
}

fn srgb_to_linear(c: vec3f) -> vec3f {
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

const HISTORY: u32 = 128u;

fn beat_history(i: u32) -> f32 {
    return frame.beat_history[i / 4u][i % 4u];
}

fn novelty_history(i: u32) -> f32 {
    return frame.novelty_history[i / 4u][i % 4u];
}

fn knob(i: u32) -> f32 {
    return frame.knobs[i / 4u][i % 4u];
}

// The palette's color at `t`, in sRGB. It repeats every 1.0.
fn palette(t: f32) -> vec3f {
    let c = frame.palette_a.rgb + frame.palette_b.rgb * cos(TAU * (frame.palette_c.rgb * t + frame.palette_d.rgb));
    return clamp(c, vec3f(0.0), vec3f(1.0));
}

fn rotate(p: vec2f, angle: f32) -> vec2f {
    let c = cos(angle);
    let s = sin(angle);
    return vec2f(c * p.x - s * p.y, s * p.x + c * p.y);
}
