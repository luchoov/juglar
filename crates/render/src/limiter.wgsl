// The flash limiter: the last step before the screen, for every view.
//
// The frame is drawn into a "candidate" image first. Before it is shown:
//
// 1. `fs_decide`, once per cell of a grid (8×8 pixels at 1080 rows): the
//    cell's mean luminance in the candidate, and whether showing it would
//    complete a third flash within one second. Each cell remembers its last extreme of
//    luminance, which way it was heading, and the age of its last four
//    transitions (a transition is a swing of 0.1 or more against the
//    direction it was heading; two make a flash). With four transitions in
//    the last second, a swing that would make a fifth is held back: the
//    cell shows a mix of the last shown frame and the candidate, exactly as
//    much of the candidate as keeps the swing under 0.1.
// 2. `fs_present`, once per pixel: the mix, a guard against saturated red,
//    dithering, and encoding for the screen. The mix is also kept, as the
//    "last shown frame" for the next one.
//
// The decision is made in the same frame it applies to, so nothing reaches
// the screen before it is checked. `limiter.rs` has the same logic in Rust,
// for tests and for the explorable.

struct Limit {
    // Pixels of the screen, and of the grid in cells.
    size: vec2f,
    cells: vec2f,
    // Pixels per cell side (even).
    cell: f32,
    // Seconds since the last frame.
    dt: f32,
    // Changes every frame, for the dithering noise.
    seed: f32,
    // 1.0 when the screen format doesn't encode to sRGB by itself.
    encode_srgb: f32,
}

@group(0) @binding(0) var<uniform> limit: Limit;
@group(0) @binding(1) var smooth_sampler: sampler;
@group(0) @binding(2) var candidate: texture_2d<f32>;
// The last shown frame, in linear light.
@group(0) @binding(3) var shown: texture_2d<f32>;
// Per cell: (last extreme, direction, last shown luminance, how much of the
// candidate is shown). In `fs_present`, this frame's decision.
@group(0) @binding(4) var cells_a: texture_2d<f32>;
// Per cell: ages of the last four transitions, newest first, in seconds.
@group(0) @binding(5) var cells_b: texture_2d<f32>;

// A swing of luminance this large is a transition (WCAG's general flash).
const DELTA: f32 = 0.1;
// Held swings stay this far under DELTA.
const MARGIN: f32 = 0.01;
// Transitions allowed within WINDOW seconds: two flashes.
const MAX_TRANSITIONS: u32 = 4u;
const WINDOW: f32 = 1.0;
// Linear red's largest share of the light (saturated red starts at 0.8).
const RED_SHARE: f32 = 0.75;
const LUMA: vec3f = vec3f(0.2126, 0.7152, 0.0722);

@vertex
fn vs_main(@builtin(vertex_index) i: u32) -> @builtin(position) vec4f {
    let corner = vec2f(f32((i << 1u) & 2u), f32(i & 2u));
    return vec4f(corner * 2.0 - 1.0, 0.0, 1.0);
}

struct Cell {
    @location(0) a: vec4f,
    @location(1) b: vec4f,
}

@fragment
fn fs_decide(@builtin(position) position: vec4f) -> Cell {
    let cell = vec2u(position.xy);
    // Mean luminance of the cell. Each sample sits on the corner between
    // four pixels, so the filtering averages them: a quarter of the reads.
    let origin = vec2f(cell) * limit.cell;
    let end = min(origin + limit.cell, limit.size);
    var sum = 0.0;
    var count = 0.0;
    for (var y = origin.y; y < end.y; y += 2.0) {
        for (var x = origin.x; x < end.x; x += 2.0) {
            let uv = vec2f(x + 1.0, y + 1.0) / limit.size;
            let c = textureSampleLevel(candidate, smooth_sampler, uv, 0.0).rgb;
            sum += dot(min(c, vec3f(1.0)), LUMA);
            count += 1.0;
        }
    }
    let y = sum / max(count, 1.0);

    let a = textureLoad(cells_a, cell, 0);
    var extreme = a.x;
    var direction = a.y;
    let last = a.z;
    var ages = textureLoad(cells_b, cell, 0) + limit.dt;
    let recent = u32(ages.x < WINDOW) + u32(ages.y < WINDOW) + u32(ages.z < WINDOW) + u32(ages.w < WINDOW);

    // With the budget spent, the cell may keep going the way it is heading,
    // but may not swing back by DELTA.
    var low = -1.0;
    var high = 2.0;
    if recent >= MAX_TRANSITIONS {
        if direction > 0.0 {
            low = extreme - (DELTA - MARGIN);
        } else if direction < 0.0 {
            high = extreme + (DELTA - MARGIN);
        }
    }
    var alpha = 1.0;
    if y < low {
        alpha = (low - last) / (y - last);
    } else if y > high {
        alpha = (high - last) / (y - last);
    }
    alpha = clamp(alpha, 0.0, 1.0);
    let shown_y = mix(last, y, alpha);

    // Follow the luminance actually shown.
    var transition = false;
    if direction == 0.0 {
        if abs(shown_y - extreme) >= DELTA {
            direction = sign(shown_y - extreme);
            extreme = shown_y;
            transition = true;
        }
    } else if direction > 0.0 {
        if shown_y > extreme {
            extreme = shown_y;
        } else if extreme - shown_y >= DELTA {
            direction = -1.0;
            extreme = shown_y;
            transition = true;
        }
    } else {
        if shown_y < extreme {
            extreme = shown_y;
        } else if shown_y - extreme >= DELTA {
            direction = 1.0;
            extreme = shown_y;
            transition = true;
        }
    }
    if transition {
        ages = vec4f(0.0, ages.xyz);
    }
    var out: Cell;
    out.a = vec4f(extreme, direction, shown_y, alpha);
    out.b = min(ages, vec4f(1000.0));
    return out;
}

// A well-mixed 32-bit hash (PCG), for noise without textures.
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

struct Present {
    @location(0) screen: vec4f,
    @location(1) shown: vec4f,
}

@fragment
fn fs_present(@builtin(position) position: vec4f) -> Present {
    let pixel = vec2u(position.xy);
    let cell = vec2u(position.xy / limit.cell);
    let alpha = textureLoad(cells_a, cell, 0).w;
    let fresh = min(textureLoad(candidate, pixel, 0).rgb, vec3f(1.0));
    let before = textureLoad(shown, pixel, 0).rgb;
    var c = max(mix(before, fresh, alpha), vec3f(0.0));

    // No saturated red, ever: where red carries more than RED_SHARE of the
    // light, move toward gray of the same luminance until it doesn't.
    // Gray has a third of its light in red, so mixing in k of it changes red
    // by k (gray - red) and the total by k (3 gray - total); solved for the
    // red share landing exactly on RED_SHARE.
    let total = c.r + c.g + c.b;
    let excess = c.r - RED_SHARE * total;
    if total > 1e-4 && excess > 0.0 {
        let gray = dot(c, LUMA);
        let k = excess / (excess + (3.0 * RED_SHARE - 1.0) * gray);
        c = mix(c, vec3f(gray), clamp(k, 0.0, 1.0));
    }

    var out: Present;
    out.shown = vec4f(c, 1.0);
    // Dither in the screen's own encoding, where its 256 levels are evenly
    // spaced: two random numbers summed give triangular noise of ±1 level.
    let n = (pixel.y * 8192u + pixel.x) * 2u + u32(limit.seed) * 33554432u;
    let encoded = clamp(linear_to_srgb(c) + (random(n) + random(n + 1u) - 1.0) / 255.0, vec3f(0.0), vec3f(1.0));
    if limit.encode_srgb > 0.5 {
        out.screen = vec4f(encoded, 1.0);
    } else {
        // The screen format encodes by itself: hand it linear light that
        // encodes back to exactly these dithered values.
        out.screen = vec4f(srgb_to_linear(encoded), 1.0);
    }
    return out;
}
