// The last step before the screen, for every view: the frame, drawn into
// a "candidate" image in linear light, is dithered and encoded for the
// screen. Nothing is held back: flashes and hard cuts reach the screen as
// the scene draws them (D48 in `docs/decisions.md`).

struct Present {
    // Changes every frame, for the dithering noise.
    seed: f32,
    // 1.0 when the screen format doesn't encode to sRGB by itself.
    encode_srgb: f32,
}

@group(0) @binding(0) var<uniform> present: Present;
@group(0) @binding(1) var candidate: texture_2d<f32>;

@vertex
fn vs_main(@builtin(vertex_index) i: u32) -> @builtin(position) vec4f {
    let corner = vec2f(f32((i << 1u) & 2u), f32(i & 2u));
    return vec4f(corner * 2.0 - 1.0, 0.0, 1.0);
}

// A well-mixed 32-bit hash (PCG), as `pcg` in `common.wgsl`.
fn hash(n: u32) -> u32 {
    let state = n * 747796405u + 2891336453u;
    let word = ((state >> ((state >> 28u) + 4u)) ^ state) * 277803737u;
    return (word >> 22u) ^ word;
}

fn random(n: u32) -> f32 {
    return f32(hash(n)) / 4294967296.0;
}

fn srgb_to_linear(c: vec3f) -> vec3f {
    return select(pow((c + 0.055) / 1.055, vec3f(2.4)), c / 12.92, c <= vec3f(0.04045));
}

fn linear_to_srgb(c: vec3f) -> vec3f {
    return select(1.055 * pow(c, vec3f(1.0 / 2.4)) - 0.055, c * 12.92, c <= vec3f(0.0031308));
}

@fragment
fn fs_present(@builtin(position) position: vec4f) -> @location(0) vec4f {
    let pixel = vec2u(position.xy);
    let c = clamp(textureLoad(candidate, pixel, 0).rgb, vec3f(0.0), vec3f(1.0));
    // Dither in the screen's own encoding, where its 256 levels are evenly
    // spaced: two random numbers summed give triangular noise of ±1 level.
    let n = (pixel.y * 8192u + pixel.x) * 2u + u32(present.seed) * 33554432u;
    let encoded = clamp(linear_to_srgb(c) + (random(n) + random(n + 1u) - 1.0) / 255.0, vec3f(0.0), vec3f(1.0));
    if present.encode_srgb > 0.5 {
        return vec4f(encoded, 1.0);
    }
    // The screen format encodes by itself: hand it linear light that
    // encodes back to exactly these dithered values.
    return vec4f(srgb_to_linear(encoded), 1.0);
}
