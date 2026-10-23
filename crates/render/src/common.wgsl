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

    // A transition between two scenes (`composite.wgsl`): x = how far it
    // has come, 0..1; y = its style; z = how soft its edge is; w = a seed
    // that makes each one's edge different.
    reveal: vec4f,
    // A calm mode (`composite.wgsl`): x = how much color is kept, y and z =
    // the luminance floor and ceiling, w = how far it has faded in.
    calm: vec4f,

    // Each drum: x = kick, y = snare, z = cymbals, 1 at a hit, falling to 0.
    drums: vec4f,
    // x = how loud the percussive part is, y = the harmonic part.
    layers: vec4f,
    // The last 128 frames of each, oldest first.
    kick_history: array<vec4f, 32>,
    snare_history: array<vec4f, 32>,
    cymbals_history: array<vec4f, 32>,
    percussive_history: array<vec4f, 32>,
    harmonic_history: array<vec4f, 32>,

    // The scene's screen effects (`Fx` in `scene/mod.rs`, applied in
    // `composite.wgsl`): [0] = shake x, shake y, chromatic split, invert;
    // [1] = flash, glitch, glitch seed, relief; [2] = ground, ground's
    // palette position, stencil.
    fx: array<vec4f, 3>,
    // The heavy side of the music: x = chaos, y = impact, z = hit,
    // w = loudness.
    heavy: vec4f,
    // How the old picture works on itself (`Material` in `scene/mod.rs`,
    // applied in `feedback.wgsl`): x = sharpen, y = cap, z = erode,
    // w = reach.
    material: vec4f,
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

// Drum `d` (0 kick, 1 snare, 2 cymbals) at history frame `i`.
fn drum_history(d: u32, i: u32) -> f32 {
    if d == 0u {
        return frame.kick_history[i / 4u][i % 4u];
    }
    if d == 1u {
        return frame.snare_history[i / 4u][i % 4u];
    }
    return frame.cymbals_history[i / 4u][i % 4u];
}

fn percussive_history(i: u32) -> f32 {
    return frame.percussive_history[i / 4u][i % 4u];
}

fn harmonic_history(i: u32) -> f32 {
    return frame.harmonic_history[i / 4u][i % 4u];
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

// A well-mixed 32-bit hash (PCG), as in `present.wgsl`.
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

// A flow with no sources or sinks, for stirring paint like a liquid: the
// curl of the noise above (its slope turned a quarter), so moving along it
// never piles paint up or tears it open. `p` in units of the short side;
// speeds of about 1 at `scale` 1. Two layers, the finer one half as strong
// and drifting the other way, so the eddies don't look like a grid.
fn curl_noise(p: vec2f) -> vec2f {
    let e = 0.01;
    let dx = noise(p + vec2f(e, 0.0)) - noise(p - vec2f(e, 0.0));
    let dy = noise(p + vec2f(0.0, e)) - noise(p - vec2f(0.0, e));
    return vec2f(dy, -dx) / (2.0 * e);
}

fn flow(p: vec2f, scale: f32, time: f32) -> vec2f {
    let coarse = curl_noise(p * scale + vec2f(time, 0.3 * time));
    let fine = curl_noise(p * scale * 2.3 + vec2f(5.2 - 0.7 * time, 1.3 + time));
    return coarse + 0.5 * fine;
}
