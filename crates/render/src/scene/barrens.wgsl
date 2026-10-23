// Barrens: a low flight down a valley between jagged ridges, under a
// storm, seen by marching rays over the land. Set in `barrens.rs`.
//
// Knobs: 0 how deep the camera is, 1 how high the ridges stand, 2 octaves
// drawn, 3 how much of the frame is new, 4 the horizon's glow, 5 lightning
// in the clouds, 6 a clock for the clouds and the mist, 7 all of the frame
// new (a cut). The waveform's slots hold the camera (`space.rs`): 0–2
// where it is, 4–6 where it looks, 8–10 its up, 12–14 its right; 16 where
// lightning strikes (to the side, in units of the camera's right), 17
// how bright the strike is, 18 mist.

const STEPS: i32 = 90;
const FAR: f32 = 24.0;
// Higher than any crest can stand (at rest; the kick's heave scales it).
const CEILING: f32 = 3.2;

fn camera(i: u32) -> vec3f {
    return vec3f(waveform(i), waveform(i + 1u), waveform(i + 2u));
}

// Where the valley's floor is across at depth `z`: the same waves as
// `barrens.rs`.
fn course(z: f32) -> f32 {
    return 1.6 * sin(0.09 * z) + 0.9 * sin(0.05 * z + 1.3);
}

// The land's height at `xz`. Ridged noise: each octave folded at its
// middle, so it rises to a sharp crest, and squared, so the crests are
// thin and the slopes between them fall away. Low along the valley, where
// only a little rubble shows; the ridges stand on both sides. Far off,
// fewer octaves: their crests are smaller than a pixel there.
fn ground(xz: vec2f, octaves: i32) -> f32 {
    var height = 0.0;
    var size = 1.0;
    var q = xz * 0.35;
    for (var i = 0; i < octaves; i++) {
        let ridge = 1.0 - abs(2.0 * noise(q) - 1.0);
        height += size * ridge * ridge;
        size *= 0.45;
        q = rotate(q, 0.6) * 2.1 + vec2f(1.7, 9.2);
    }
    let across = abs(xz.x - course(xz.y));
    let valley = smoothstep(0.25, 2.2, across);
    return knob(1u) * (0.1 * height + 1.6 * valley * height) - 0.05;
}

// Clouds overhead: two layers of noise drifting on the sky's plane.
fn clouds(ray: vec3f) -> f32 {
    let on = ray.xz / max(ray.y, 0.05) * 0.6 + vec2f(0.3, 1.0) * knob(6u) * 0.2;
    return 0.6 * noise(on) + 0.4 * noise(on * 2.3 + vec2f(5.1, 2.7));
}

// How many octaves to draw at distance `t`.
fn detail(t: f32) -> i32 {
    return max(i32(knob(2u)) - i32(t / 4.0), 2);
}

fn sources(p: vec2f, pixel: f32) -> vec3f {
    let laid = max(knob(3u), knob(7u));
    let eye = camera(0u);
    let up = camera(8u);
    let ray = normalize(camera(4u) + 1.2 * (p.x * camera(12u) - p.y * up));
    // March over the land: the height above it is not a distance, so step
    // only part of it; finer far off, where a pixel spans more.
    var t = 0.05;
    var hit = false;
    for (var i = 0; i < STEPS; i++) {
        let at = eye + ray * t;
        let above = at.y - ground(at.xz, detail(t));
        if above < 0.002 * t {
            hit = true;
            break;
        }
        t += max(0.5 * above, 0.025 * t);
        // Past the far haze, or climbing above the highest crest: sky.
        if t > FAR || (ray.y > 0.0 && at.y > CEILING * knob(1u)) {
            break;
        }
    }

    let glow = knob(4u);
    let sheet = knob(5u);
    let strike = waveform(17u);
    // The strike: a point inside the clouds ahead, to one side. It is the
    // impact's whole flash: no veil over the screen (`barrens.rs`).
    let bolt = eye + normalize(camera(4u) + waveform(16u) * camera(12u)) * 8.0 + vec3f(0.0, 1.8, 0.0);
    // The sky: a band of glow along the horizon, clouds above lit from
    // inside by the lightning, brightest around the strike.
    let horizon = exp(-abs(ray.y) * 9.0);
    let cloud = clouds(ray) * smoothstep(-0.02, 0.15, ray.y);
    let around = pow(max(dot(ray, normalize(bolt - eye)), 0.0), 8.0);
    var light = glow * (1.6 * horizon + 0.25 * cloud) + (3.0 * sheet + 10.0 * strike * around) * cloud * cloud;
    var color = 0.55 + 0.35 * horizon + 0.3 * strike * around;
    if hit {
        let at = eye + ray * t;
        let e = max(2.0 * pixel * t, 0.002);
        let octaves = detail(t);
        let normal = normalize(vec3f(
            ground(at.xz - vec2f(e, 0.0), octaves) - ground(at.xz + vec2f(e, 0.0), octaves),
            2.0 * e,
            ground(at.xz - vec2f(0.0, e), octaves) - ground(at.xz + vec2f(0.0, e), octaves),
        ));
        // Lit low from the glowing horizon ahead: only what faces it, the
        // ridges' rims.
        let toward = normalize(vec3f(camera(4u).x, 0.12, camera(4u).z));
        let rim = pow(max(dot(normal, toward), 0.0), 3.0);
        // And by the strike, from where it fell: what faces it white, the
        // rest black, the ridges casting shadows across the valley.
        var flare = 0.0;
        if strike > 0.03 {
            let to_bolt = normalize(bolt - at);
            flare = max(dot(normal, to_bolt), 0.0);
            var s = 0.05;
            for (var i = 0; i < 24 && flare > 0.0; i++) {
                let along = at + to_bolt * s;
                let above = along.y - ground(along.xz, 3);
                flare = min(flare, 12.0 * above / s);
                s += max(0.5 * above, 0.05);
            }
            flare = clamp(flare, 0.0, 1.0);
        }
        // A faint light from the clouds over everything, so the valley
        // floor keeps its relief in the dark.
        let sky = 0.05 * glow * max(normal.y, 0.0);
        let lit = glow * 2.0 * rim + sky + 0.3 * sheet * max(normal.y, 0.0) + 5.0 * strike * flare;
        // Far land sinks into the horizon's haze; near, it stays black.
        let haze = 1.0 - exp(-0.002 * t * t);
        light = mix(lit, glow * 1.2, haze);
        color = mix(0.15 + 0.25 * rim + 0.4 * strike * flare, 0.6, haze);
        // Mist, with the cymbals: low over the valley floor, drifting,
        // lit by the horizon; thin near the camera, thicker off ahead.
        let mist = waveform(18u) * smoothstep(0.35, 0.0, at.y) * (1.0 - exp(-0.4 * t))
            * noise(at.xz * vec2f(1.2, 0.5) + vec2f(0.0, knob(6u) * 1.5));
        light += glow * 1.5 * mist;
        color = mix(color, 0.6, clamp(mist, 0.0, 1.0));
    }
    return vec3f(laid, laid * light, laid * light * color);
}

fn overlay(p: vec2f, pixel: f32) -> vec3f {
    return vec3f(0.0);
}

fn warp(p: vec2f, q: vec2f) -> vec2f {
    return q;
}
