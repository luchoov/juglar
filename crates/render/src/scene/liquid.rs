//! Liquid: the screen a marbled liquid that barely fades, only stirs. The
//! waveform is poured in as a ribbon across the middle and three blobs of
//! color orbit through it; the liquid's currents (curl noise) carry it all
//! away into swirls, and a palette that runs through its colors several
//! times turns the swirls into bands, like oil paint dragged with a comb.
//! Every kick surges the currents, the snare reverses their turn, and an
//! impact blows the marble outward and shifts every band to new colors.
//! The ink sharpens itself a little each frame, so the bands stay crisp
//! instead of blurring into mud.
//!
//! Each layer answers to its own part of the music. The currents are the
//! drums': the kick surges them, the snare turns them, the busier the
//! playing the faster they run, the grittier the sound the finer the
//! eddies. The paint is the harmony's: held chords louder than before
//! pour more of it. The light is the band's: louder than the last few
//! seconds, the liquid brightens and opens out. And the cymbals have a
//! layer of their own, white sparks thrown into the current and dragged
//! into streaks.
//!
//! | Signal          | Moves                                              |
//! |-----------------|----------------------------------------------------|
//! | waveform        | the ribbon poured across the middle                |
//! | kick, hit       | a surge of the currents (spring)                   |
//! | snare           | the currents' turn reversed, the ribbon twisted    |
//! | cymbals         | sparks thrown into the current                     |
//! | density         | how fast the currents run                          |
//! | grit            | how fine the eddies are                            |
//! | harmonic, rel.  | how much paint is poured (held chords, more)       |
//! | energy, rel.    | how bright the liquid is, how far it opens out     |
//! | impact          | the marble blown outward, the colors shifted, flash|
//! | chaos           | torn colors                                        |
//! | harmonic        | how fast the bands drift through the palette       |

use juglar_signals::{Signals, WAVEFORM_POINTS};

use super::react::{Pulse, Reactor};
use super::{Character, Fx, Look, Material, Palette, Scene};

/// Through the colors twice around: hot pink, gold, violet, ink blue.
const PALETTE: Palette = Palette {
    a: [0.55, 0.4, 0.55],
    b: [0.45, 0.45, 0.45],
    c: [2.0, 2.0, 2.0],
    d: [0.0, 0.2, 0.55],
};

pub(crate) struct Liquid {
    reactor: Reactor,
    waveform: [f32; WAVEFORM_POINTS],
    /// Where the currents are: they move on, faster with the music.
    flow: f32,
    angle: f32,
    hue: f32,
    clock: f32,
}

impl Liquid {
    pub fn new() -> Self {
        Self { reactor: Reactor::new(0x11c0), waveform: [0.0; WAVEFORM_POINTS], flow: 0.0, angle: 0.0, hue: 0.0, clock: 0.0 }
    }
}

impl Scene for Liquid {
    fn name(&self) -> &'static str {
        "liquid"
    }

    /// A surging liquid.
    fn character(&self) -> Character {
        Character { energy: 0.85, pulse: 0.75, flow: true }
    }

    fn shader(&self) -> &'static str {
        include_str!("liquid.wgsl")
    }

    fn update(&mut self, s: &Signals, motion: f32) -> Look {
        let dt = s.dt.clamp(0.0, 0.1);
        let moving = dt * motion;
        let Pulse { punch, snap, slam, shake, flash, turn, cuts, chaos, harmony, .. } = self.reactor.update(s, motion);
        let follow = 1.0 - (-moving / 0.03).exp();
        for (smooth, &raw) in self.waveform.iter_mut().zip(&s.waveform) {
            *smooth += (raw - *smooth) * follow;
        }
        let live = motion * (0.5 + s.intensity.clamp(0.0, 1.0));
        let punch = punch.max(0.0);
        self.clock += moving;
        self.flow += moving * (0.08 + 0.4 * chaos + 0.8 * punch) * live;
        self.angle += moving * turn * (0.2 + 2.0 * snap.abs()) * live;
        self.hue += moving * (0.02 + 0.12 * harmony);

        // How hard the currents push this frame: with the playing's
        // density, surging on kicks.
        let push = live * (0.04 + 0.12 * s.density + 0.5 * punch.min(1.5)) * dt;
        // The band louder than the last seconds, and held chords louder.
        let swell = (s.relative.energy - 1.0).clamp(-0.5, 1.0);
        let held = (s.relative.harmonic - 1.0).clamp(-0.5, 1.0);
        Look {
            center: [0.0, 0.0],
            zoom: (live * (0.01 + 0.25 * swell.max(0.0) + 0.15 * punch + 2.5 * slam.max(0.0)) * dt).exp(),
            rotation: live * turn * (0.05 + 0.6 * snap.abs()) * dt,
            swirl: live * turn * 0.4 * snap.abs() * dt,
            // Slow to fade, so the marble lasts but the sparks don't pile up.
            decay: (-moving / 4.0).exp(),
            palette_drift: (0.04 + 0.15 * harmony) * moving,
            knobs: [
                push,                                       // the currents' push this frame
                self.flow,                                  // where the currents are
                1.2 + 1.6 * s.grit,                         // eddies per screen
                self.angle,                                 // the ribbon's and blobs' turn
                0.1 + 0.12 * s.loudness + 0.1 * punch,      // the ribbon's height
                (0.35 + 0.5 * held).max(0.08),              // paint poured per frame
                self.clock,                                 // the blobs' orbit
                motion * s.cymbals,                         // sparks thrown this frame
            ],
            palette: PALETTE,
            palette_shift: self.hue + (cuts % 6) as f32 * 0.17,
            palette_wrap: true,
            exposure: 1.3 + 0.5 * swell,
            bloom: 0.25,
            waveform: self.waveform,
            spectrum: s.spectrum,
            material: Material { sharpen: 0.12 + 0.1 * chaos, cap: 2.0, erode: 0.0, reach: 1.5 },
            fx: Fx {
                shake: [0.012 * shake[0], 0.012 * shake[1]],
                chroma: 0.002 + 0.01 * chaos,
                invert: 0.0,
                flash: 0.4 * flash,
                glitch: 0.0,
                glitch_seed: 0.0,
                relief: 0.35,
                ground: 0.0,
                ground_hue: 0.0,
                stencil: 0.0,
            },
        }
    }
}
