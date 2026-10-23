// Bore: a tunnel drilled through fractal rock, seen by marching rays from
// a camera riding its course toward a white-hot core. Set in `bore.rs`.
//
// Knobs: 0 how deep the camera is, 1 the mirrors' turn, 2 folds drawn,
// 3 how much of the frame is new, 4 how open the tunnel is, 5 sparks,
// 6 a clock for the sparks, 7 all of the frame new (a cut). The waveform's
// slots hold the camera (`space.rs`): 0–2 where it is, 4–6 where it looks,
// 8–10 its up, 12–14 its right; 16 the course's branch, 17 the core's heat.

const STEPS: i32 = 64;
const FAR: f32 = 10.0;
// The tunnel's radius, and how far out the rock is solid all through.
const RADIUS: f32 = 0.45;
const SHELL: f32 = 1.6;
// How far ahead along the course the core burns.
const CORE_AHEAD: f32 = 5.0;

fn camera(i: u32) -> vec3f {
    return vec3f(waveform(i), waveform(i + 1u), waveform(i + 2u));
}

// Where the tunnel's middle is at depth `z`: the same waves as `bore.rs`.
fn course(z: f32) -> vec2f {
    let b = waveform(16u);
    return vec2f(
        1.4 * sin(0.21 * z + b) + 0.9 * sin(0.13 * z + 2.0 * b),
        1.1 * cos(0.17 * z + 1.7 * b) + 0.5 * sin(0.29 * z),
    );
}

// How far `p` is from the nearest rock (a lower bound), and how near the
// fold came to its center, for the rock's color. Around the tunnel the
// rock is a kaleidoscopic fold, repeated every 2 units along it and
// twisted as it goes: mirrored into one wedge, turned, scaled, again.
fn rock(p: vec3f) -> vec2f {
    let around = p.xy - course(p.z);
    let r = length(around);
    var q = vec3f(rotate(around, 0.35 * p.z), p.z - 2.0 * round(p.z / 2.0));
    let turn = knob(1u);
    let folds = i32(knob(2u));
    var size = 1.0;
    var trap = 1e3;
    for (var i = 0; i < folds; i++) {
        q = abs(q);
        if q.x < q.y {
            q = q.yxz;
        }
        if q.x < q.z {
            q = q.zyx;
        }
        if q.y < q.z {
            q = q.xzy;
        }
        q = vec3f(rotate(q.xy, turn), q.z);
        let yz = rotate(q.yz, 0.5 * turn + 0.3);
        q = vec3f(q.x, yz) * 2.5 - vec3f(1.5);
        size *= 2.5;
        trap = min(trap, dot(q, q));
    }
    let block = length(max(abs(q) - vec3f(0.6), vec3f(0.0))) / size;
    // Beyond the shell all is rock; inside the tunnel, none.
    let solid = min(block, SHELL - r);
    let bore = knob(4u) * RADIUS - r;
    return vec2f(max(solid, bore), trap);
}

fn sources(p: vec2f, pixel: f32) -> vec3f {
    let laid = max(knob(3u), knob(7u));
    let eye = camera(0u);
    let ray = normalize(camera(4u) + 1.25 * (p.x * camera(12u) - p.y * camera(8u)));
    // March: step as far as the nearest rock allows (less, for the twist
    // stretches space), until it is nearer than this pixel's footprint.
    var t = 0.02;
    var near = vec2f(1.0, 1.0);
    var steps = 0;
    var hit = false;
    for (; steps < STEPS; steps++) {
        near = rock(eye + ray * t);
        if near.x < 1.5 * pixel * t + 1e-4 {
            hit = true;
            break;
        }
        t += near.x * 0.75;
        if t > FAR {
            break;
        }
    }

    // The core: a point of light on the course ahead, glowing wherever a
    // ray passes near it before meeting rock.
    let heat = waveform(17u);
    let core = vec3f(course(eye.z + CORE_AHEAD), eye.z + CORE_AHEAD);
    let to_core = core - eye;
    let along = dot(to_core, ray);
    let miss = length(to_core - ray * along);
    var light = 0.0;
    var color = 1.0;
    if along > 0.0 && (!hit || t > along) {
        light = heat * 0.01 / (miss * miss + 0.002);
    }
    if hit {
        let at = eye + ray * t;
        let e = max(3.0 * pixel * t, 1e-4);
        let a = vec3f(1.0, -1.0, -1.0);
        let b = vec3f(-1.0, -1.0, 1.0);
        let c = vec3f(-1.0, 1.0, -1.0);
        let d = vec3f(1.0, 1.0, 1.0);
        let normal = normalize(a * rock(at + a * e).x + b * rock(at + b * e).x + c * rock(at + c * e).x + d * rock(at + d * e).x);
        let reach = 0.03 * t + 0.003;
        let open = clamp(rock(at + normal * reach).x / reach, 0.0, 1.0);
        let occlusion = open * (1.0 - f32(steps) / f32(STEPS));
        // Lit by the core, falling off with distance; a faint light from
        // the camera so the nearest rock is never quite lost.
        let lamp = core - at;
        let distance = length(lamp);
        let diffuse = max(dot(normal, lamp / distance), 0.0);
        let burn = heat * diffuse / (1.0 + 0.25 * distance * distance);
        let near_light = 0.05 * max(dot(normal, -ray), 0.0) / (1.0 + 4.0 * t * t);
        // Sparks: on the cymbals, a point flares in a few cells of the
        // rock near the camera, a new few each instant; the flight's
        // trails draw them out into streaks.
        let grid = at * 30.0;
        let cell = vec3i(floor(grid));
        let flicker = i32(floor(knob(6u) * 24.0));
        let chosen = step(0.99, grid_random(vec2i(cell.x + 7919 * cell.z, cell.y + 131 * flicker)));
        let point = 1.0 - smoothstep(0.1, 0.3, length(fract(grid) - 0.5));
        let spark = chosen * point * knob(5u) * 10.0 / (1.0 + t * t);
        light = 6.0 * (burn + near_light) * occlusion + spark;
        // Dull red where the light is faint, amber where it burns, white
        // at the sparks.
        color = 0.1 + 0.25 * clamp(sqrt(near.y), 0.0, 1.0) + 0.4 * clamp(burn, 0.0, 1.0) + min(spark, 1.0);
    }
    return vec3f(laid, laid * light, laid * light * color);
}

fn overlay(p: vec2f, pixel: f32) -> vec3f {
    return vec3f(0.0);
}

fn warp(p: vec2f, q: vec2f) -> vec2f {
    return q;
}
