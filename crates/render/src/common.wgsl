// Shared by every shader: the frame's numbers and a few helpers. Each
// shader module is this file followed by its own.

struct Frame {
    // Pixels of the window, and of the feedback images (half as many).
    resolution: vec2f,
    feedback_size: vec2f,
    time: f32,
    dt: f32,
    // How much glow the scene wants (see `bloom.rs`).
    bloom: f32,
    // Changes every frame, for noise that should too.
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
    // 1.0 when the palette loops (positions wrap around), 0.0 when it is a
    // path between two ends (positions are clamped).
    palette_wrap: f32,

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

// A well-mixed 32-bit hash (PCG), as in `limiter.wgsl`.
fn pcg(n: u32) -> u32 {
    let state = n * 747796405u + 2891336453u;
    let word = ((state >> ((state >> 28u) + 4u)) ^ state) * 277803737u;
    return (word >> 22u) ^ word;
}

// A random number in 0..1 for each point of an integer grid.
fn grid_random(i: vec2i) -> f32 {
    // One hash of both coordinates folded into one number; the odd
    // multiplier keeps nearby points apart.
    return f32(pcg(bitcast<u32>(i.x) * 0x27d4eb2du + bitcast<u32>(i.y))) / 4294967296.0;
}

// Smooth noise: random values on a grid, blended with an S-curve. The grid
// points are integers so that both cells sharing a point agree on its
// value exactly. The usual shortcut, fract(sin(x) × 43758.5), amplifies a
// rounding difference in x into a different value, and GPUs may round the
// same point differently when it is reached from the cell on either side:
// straight seams appeared across the picture.
fn noise(p: vec2f) -> f32 {
    let cell = floor(p);
    let i = vec2i(cell);
    let f = p - cell;
    let u = f * f * (3.0 - 2.0 * f);
    let bottom = mix(grid_random(i), grid_random(i + vec2i(1, 0)), u.x);
    let top = mix(grid_random(i + vec2i(0, 1)), grid_random(i + vec2i(1, 1)), u.x);
    return mix(bottom, top, u.y);
}
