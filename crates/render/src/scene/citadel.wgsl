// Citadel: a city carved out of a Mandelbox, seen by marching rays from a
// camera flying through it. Set in `citadel.rs`, which also flies the
// camera through the same fold.
//
// Knobs: 0 the fold's scale, 1 folds drawn, 2 the cracks lit, 3 how much
// of the frame is new, 4 light on the stone, 5 the fog's thickness, 6 a
// clock for the grain, 7 all of the frame new (a cut). The waveform's
// slots hold the camera: 0–2 where it is, 4–6 where it looks, 8–10 its up,
// 12–14 its right; 16 the grain.

const MIN_RADIUS2: f32 = 0.25;
const FIXED_RADIUS2: f32 = 1.0;
const STEPS: i32 = 72;
const FAR: f32 = 8.0;

fn camera(i: u32) -> vec3f {
    return vec3f(waveform(i), waveform(i + 1u), waveform(i + 2u));
}

// How far `p` is from the nearest wall (a lower bound), and how near the
// fold came to its center, for the stone's color. Each fold reflects the
// point into a box, inverts it in a sphere, scales it and adds where it
// started: the same halls and blocks at every size.
fn city(p: vec3f) -> vec2f {
    let fold = knob(0u);
    let folds = i32(knob(1u));
    var z = p;
    var dr = 1.0;
    var trap = 1e3;
    for (var i = 0; i < folds; i++) {
        z = clamp(z, vec3f(-1.0), vec3f(1.0)) * 2.0 - z;
        let r2 = dot(z, z);
        trap = min(trap, r2);
        let k = select(select(1.0, FIXED_RADIUS2 / r2, r2 < FIXED_RADIUS2), FIXED_RADIUS2 / MIN_RADIUS2, r2 < MIN_RADIUS2);
        z = z * k * fold + p;
        dr = dr * k * abs(fold) + 1.0;
    }
    return vec2f(length(z) / abs(dr), trap);
}

fn sources(p: vec2f, pixel: f32) -> vec3f {
    // Some pixels keep the older frames this time: a dust of trails.
    let cell = vec2i(floor(p / pixel));
    let grain = grid_random(cell + vec2i(i32(knob(6u) * 977.0), i32(knob(6u) * 389.0)));
    let cut = knob(7u);
    let laid = max(knob(3u) * step(waveform(16u), grain), cut);
    if laid <= 0.0 {
        return vec3f(0.0);
    }

    let eye = camera(0u);
    let ray = normalize(camera(4u) + 1.35 * (p.x * camera(12u) - p.y * camera(8u)));
    // March: step as far as the nearest wall allows, until a wall is
    // nearer than this pixel's footprint at that distance (two pixels:
    // finer than that the fractal only glitters).
    var t = 0.0;
    var near = vec2f(1.0, 1.0);
    var steps = 0;
    var hit = false;
    for (; steps < STEPS; steps++) {
        near = city(eye + ray * t);
        if near.x < 2.0 * pixel * t + 1e-4 {
            hit = true;
            break;
        }
        t += near.x * 0.9;
        if t > FAR {
            break;
        }
    }

    // Rays that ran out of steps grazed walls all the way: a crack.
    hit = hit || steps == STEPS;
    // What is right by the camera is laid thinly, so it stays smeared by
    // the flight's trails: big and out of focus.
    let laid_here = laid * mix(0.3, 1.0, smoothstep(0.05, 0.5, t));
    // The fog pales the far halls, lighter looking up.
    let fog = 1.0 - exp(-knob(5u) * t * t);
    let haze = knob(4u) * 3.0 * (1.0 + 0.4 * dot(ray, camera(8u)));
    var light = haze;
    var color = 0.3 + 0.08 * dot(ray, camera(12u));
    if hit {
        let at = eye + ray * t;
        // The wall's slope, from four nearby distances a few pixels apart:
        // smooth stone, not the glitter of every fold.
        let e = max(4.0 * pixel * t, 1e-4);
        let a = vec3f(1.0, -1.0, -1.0);
        let b = vec3f(-1.0, -1.0, 1.0);
        let c = vec3f(-1.0, 1.0, -1.0);
        let d = vec3f(1.0, 1.0, 1.0);
        let normal = normalize(a * city(at + a * e).x + b * city(at + b * e).x + c * city(at + c * e).x + d * city(at + d * e).x);
        // Cracks: how open the space just off the wall is, and how many
        // steps it took to get here (grazing the walls takes many).
        let reach = 0.03 * t + 0.003;
        let open = clamp(city(at + normal * reach).x / reach, 0.0, 1.0);
        let crowded = f32(steps) / f32(STEPS);
        let occlusion = clamp(open * (1.0 - crowded), 0.0, 1.0);
        // A hard light from high on one side, so half of what faces the
        // camera is in shadow; a little of the fog's light from above.
        let key = normalize(camera(8u) * 0.8 + camera(12u) * 0.5 + camera(4u) * 0.2);
        let diffuse = max(dot(normal, key), 0.0);
        let sky = 0.5 + 0.5 * dot(normal, camera(8u));
        let stone = (0.9 * diffuse * diffuse + 0.012 * sky) * occlusion * occlusion;
        // The snare lights the cracks: the deep ones, near enough to see
        // (far off, every fold is a crack, and they would only glitter).
        let cracks = knob(2u) * smoothstep(0.4, 0.9, 1.0 - open) * (1.0 - fog) * 3.0;
        light = mix(knob(4u) * 20.0 * stone, haze, fog) + cracks;
        // The stone's color: teal or gray by district, paler where its
        // folds came near the center; the lit cracks lilac.
        let district = 0.5 + 0.5 * sin(dot(at, vec3f(1.9, -1.3, 1.1)));
        let tint = 0.3 * district + 0.2 * clamp(1.5 * sqrt(near.y), 0.0, 1.0) + 0.1 * normal.y;
        color = mix(tint, color, fog) + 0.5 * min(cracks, 1.0);
    }
    return vec3f(laid_here, laid_here * light, laid_here * light * color);
}

fn overlay(p: vec2f, pixel: f32) -> vec3f {
    return vec3f(0.0);
}

fn warp(p: vec2f, q: vec2f) -> vec2f {
    return q;
}
