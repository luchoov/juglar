//! Barrens: a low flight down a valley between jagged ridges, under a
//! storm. The land is ridged noise (noise folded at its middle, so every
//! octave adds sharp crests instead of round hills), flattened into a
//! winding valley floor that the camera skims. Above, clouds roll; ahead,
//! the horizon glows. Most of the time the ridges are only black shapes
//! against that glow; lightning shows them whole, casting shadows.
//!
//! The third of the "procedural 3D worlds", after Citadel (D61) and Bore
//! (D63), and the slow one: made for doom and sludge (D64).
//!
//! Each layer answers to its own part of the music (D54). The land is the
//! kick's: each one heaves the ridges up and shoves the camera on; the
//! busier the playing, the faster the flight; the louder, the lower it
//! skims, and in quiet passages it climbs over the barrens. The sky is the snare's:
//! each lights the clouds from inside. The cymbals raise mist. The
//! horizon is the band's: louder than the last seconds, it burns
//! brighter. The ridges' sharpness is the chaos': wild passages fold more
//! octaves into them. An impact is a lightning strike: the clouds blaze
//! at one point, and from there the land is lit white for a moment, the
//! ridges' shadows thrown across the valley.
//!
//! | Signal          | Moves                                               |
//! |-----------------|-----------------------------------------------------|
//! | kick, hit       | the ridges heave up, a shove forward                |
//! | loudness        | how low it flies: skimming loud, climbing quiet     |
//! | density         | how fast it flies                                   |
//! | snare           | lightning inside the clouds                         |
//! | cymbals         | mist drifting low over the valley floor             |
//! | energy, rel.    | how bright the horizon burns                        |
//! | chaos, grit     | how many octaves: sharper, more jagged ridges       |
//! | harmonic        | how fast the clouds roll and the colors drift       |
//! | impact          | a strike: the land lit from one point, shadows      |

use juglar_signals::Signals;

use super::react::{Pulse, Reactor, Spring, random};
use super::space::{camera, cross, normalize};
use super::{Character, Fx, Look, Material, Palette, Scene};

/// Storm: from a dark steel blue, through slate, to a pale violet.
const PALETTE: Palette = Palette {
    a: [0.55, 0.55, 0.7],
    b: [0.3, 0.25, 0.25],
    c: [0.5, 0.5, 0.5],
    d: [0.5, 0.5, 0.5],
};

/// How high the camera skims over the valley floor when the music is
/// loud, and how high it climbs when it is quiet.
const LOW: f32 = 0.38;
const HIGH: f32 = 1.2;
/// How far ahead along the valley the camera looks.
const LOOK_AHEAD: f32 = 2.5;
/// How far the camera flies before the valley starts over; it never shows.
const WRAP: f32 = 4096.0;

/// Where the valley's floor is across at depth `z`: the same waves as
/// `barrens.wgsl`'s `course`.
fn course(z: f32) -> f32 {
    1.6 * (0.09 * z).sin() + 0.9 * (0.05 * z + 1.3).sin()
}

pub(crate) struct Barrens {
    reactor: Reactor,
    seed: u32,
    depth: f32,
    speed: f32,
    /// How high the camera flies, following the loudness slowly.
    altitude: f32,
    /// The ridges heaved up by kicks, settling back.
    heave: Spring,
    /// Lightning: in the clouds (snares), and a strike (impacts).
    sheet: f32,
    strike: f32,
    bolt: f32,
    roll: f32,
    hue: f32,
    octaves: f32,
    clock: f32,
}

impl Barrens {
    pub fn new() -> Self {
        Self {
            reactor: Reactor::new(0xba22),
            seed: 0x5eed_ba22,
            depth: 0.0,
            speed: 0.0,
            altitude: LOW,
            heave: Spring::new(1.5, 0.5),
            sheet: 0.0,
            strike: 0.0,
            bolt: 0.0,
            roll: 0.0,
            hue: 0.0,
            octaves: 5.0,
            clock: 0.0,
        }
    }
}

impl Scene for Barrens {
    fn name(&self) -> &'static str {
        "barrens"
    }

    /// A heavy, slow flight.
    fn character(&self) -> Character {
        Character { energy: 0.75, pulse: 0.7, flow: true }
    }

    fn shader(&self) -> &'static str {
        include_str!("barrens.wgsl")
    }

    fn update(&mut self, s: &Signals, motion: f32) -> Look {
        let dt = s.dt.clamp(0.0, 0.1);
        let moving = dt * motion;
        let Pulse { punch, snap, shake, chaos, harmony, kicked, snared, impacted, .. } = self.reactor.update(s, motion);
        let live = motion * (0.5 + s.intensity.clamp(0.0, 1.0));
        self.clock += moving * (1.0 + 2.0 * harmony);

        // The kick heaves the land; the ridges settle back.
        if kicked && motion > 0.0 {
            self.heave.strike(0.3 * s.kick.max(0.5));
        }
        let heave = 1.0 + self.heave.step(moving);
        // Snares light the clouds; an impact strikes, somewhere ahead.
        if snared && motion > 0.0 {
            self.sheet = self.sheet.max(0.6 + 0.4 * s.snare);
        }
        if impacted && motion > 0.0 {
            self.strike = 1.0;
            self.sheet = 1.0;
            self.bolt = 1.2 * random(&mut self.seed);
        }
        self.sheet *= (-moving / 0.12).exp();
        self.strike *= (-moving / 0.35).exp();

        let punch = punch.max(0.0);
        let target = 0.4 + 1.2 * s.density + 0.4 * chaos + 2.5 * punch;
        self.speed += (target - self.speed) * (1.0 - (-dt / 0.2).exp());
        self.depth += self.speed * moving * live;
        if self.depth > WRAP {
            self.depth -= WRAP;
        }
        // Loud, it dives to skim the valley floor; quiet, it climbs over
        // the barrens, slowly either way.
        let high = ((0.85 - s.loudness) / 0.45).clamp(0.0, 1.0);
        let goal = LOW + (HIGH - LOW) * high;
        self.altitude += (goal - self.altitude) * (1.0 - (-moving / 4.0).exp());
        self.roll += moving * 0.4 * snap;
        self.roll *= (-moving / 1.5).exp();
        self.hue += moving * (0.01 + 0.05 * harmony);
        let want = 4.0 + 3.0 * (0.6 * chaos + 0.4 * s.grit);
        if (want - self.octaves).abs() > 0.8 {
            self.octaves = want.round();
        }

        let eye = [course(self.depth), self.altitude, self.depth];
        let ahead = [course(self.depth + LOOK_AHEAD), 0.75 * self.altitude - 0.15, self.depth + LOOK_AHEAD];
        let forward = normalize([ahead[0] - eye[0], ahead[1] - eye[1], ahead[2] - eye[2]]);
        let side = normalize(cross([0.0, 1.0, 0.0], forward));
        let up = cross(forward, side);
        let swell = (s.relative.energy - 1.0).clamp(-0.5, 1.0);
        let mut camera = camera(eye, forward, up, self.roll);
        // The strike: how far to the side it falls, and how bright.
        camera[16] = self.bolt;
        camera[17] = self.strike;
        // Mist, with the cymbals.
        camera[18] = motion * s.cymbals.max(0.0);
        Look {
            center: [0.0, 0.0],
            zoom: (live * (0.05 * self.speed + 0.3 * punch) * dt).exp(),
            rotation: 0.0,
            swirl: 0.0,
            decay: (-moving / 0.8).exp(),
            palette_drift: 0.0,
            knobs: [
                self.depth,                                  // how deep
                heave,                                       // how high the ridges stand
                self.octaves,                                // octaves drawn
                0.75 - 0.3 * punch.min(1.0),                 // how much of the frame is new
                0.6 + 0.6 * s.loudness + 1.2 * swell.max(0.0),  // the horizon's glow
                self.sheet,                                  // lightning in the clouds
                self.clock,                                  // the clouds' and mist's clock
                if impacted && motion > 0.0 { 1.0 } else { 0.0 }, // all of the frame new
            ],
            palette: PALETTE,
            palette_shift: 0.05 * self.hue.sin(),
            palette_wrap: false,
            exposure: 1.0 + 0.2 * swell,
            bloom: 0.3,
            waveform: camera,
            spectrum: s.spectrum,
            material: Material::NONE,
            fx: Fx {
                shake: [0.01 * shake[0], 0.008 * shake[1]],
                chroma: 0.001 + 0.003 * chaos,
                ..Fx::NONE
            },
        }
    }
}
