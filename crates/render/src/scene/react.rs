//! How the aggressive scenes react: hits as impulses into stiff springs,
//! hard cuts, and a few numbers that mean the same in every scene.
//!
//! The first scenes followed the music through three stages of smoothing
//! (`smooth3`), so a thin line never trembled. That also took the punch out
//! of every hit: a kick arrived as a soft swell a tenth of a second late.
//! Here a hit is an *impulse*: it kicks a spring, which jumps at once,
//! overshoots and rings back to rest within a few tenths of a second, like
//! a drum skin or a speaker cone. Stiff and lightly damped springs give a
//! snap with a recoil; the scene decides what each one moves.
//!
//! The roles follow one rule: **the percussion moves, the harmony paints,
//! the chaos breaks the form.**
//!
//! - `punch` (kick, and any attack when the drums go unheard), `snap`
//!   (snare) and `shimmer` (cymbals) are springs, for motion: zoom pumps,
//!   jolts, twists.
//! - `slam` is a slower, bigger spring kicked by an impact: the band
//!   slamming back in after a break.
//! - `shake` is a jolt in a random direction on every strong hit, sprung
//!   back to the middle.
//! - `flash` is a light envelope, full at an impact. Flashing on every
//!   hit was tried: in a wall of sound it lays a gray veil over everything.
//! - `turn` flips between +1 and -1 on impacts and on every fourth snare
//!   while the music is wild: whatever spins reverses at once.
//! - `cuts` counts hard cuts (impacts, and strong hits at most twice a
//!   second while chaos is high), for scenes to jump to a new pattern or
//!   color on each.
//! - `chaos` follows the music's chaos (fast up, slow down); `harmony`
//!   is how much of the sound is held notes rather than hits, for color.

use juglar_signals::Signals;

/// A damped spring, kicked by impulses. Its position jumps by nothing at
/// a kick, its velocity does; for an immediate jump too, `strike` sets
/// both.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Spring {
    pub x: f32,
    v: f32,
    /// Natural frequency (radians per second) and damping ratio.
    omega: f32,
    zeta: f32,
}

impl Spring {
    /// A spring ringing at `hz` with damping ratio `zeta` (below 1 it
    /// overshoots; 0.2 rings a few times, 0.7 barely once).
    pub fn new(hz: f32, zeta: f32) -> Self {
        Self { x: 0.0, v: 0.0, omega: std::f32::consts::TAU * hz, zeta }
    }

    /// Hits it: the position jumps up by `amount` at once and the spring
    /// carries on from there, overshooting through zero on its way back.
    pub fn strike(&mut self, amount: f32) {
        self.x += amount;
        // A small push onward too, so it peaks a moment after the hit
        // rather than at it.
        self.v += amount * self.omega * 0.3;
    }

    /// Advances by `dt` seconds, in steps small enough to stay stable for
    /// stiff springs at a low frame rate.
    pub fn step(&mut self, dt: f32) -> f32 {
        let steps = (dt * 480.0).ceil().max(1.0);
        let h = dt / steps;
        for _ in 0..steps as u32 {
            // Semi-implicit Euler: velocity first, then position.
            self.v += (-self.omega * self.omega * self.x - 2.0 * self.zeta * self.omega * self.v) * h;
            self.x += self.v * h;
        }
        self.x
    }
}

/// Finds the moments a decaying event (1 at a hit, falling back to 0) was
/// hit: when it jumps up.
#[derive(Clone, Copy, Debug, Default)]
struct Edge {
    last: f32,
}

impl Edge {
    /// The height of a new hit this frame, or 0.
    fn rise(&mut self, event: f32) -> f32 {
        let rise = if event > self.last + 0.2 { event } else { 0.0 };
        self.last = event;
        rise
    }
}

/// What a scene gets from `Reactor` each frame (see the module's notes).
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Pulse {
    pub punch: f32,
    pub snap: f32,
    pub shimmer: f32,
    pub slam: f32,
    pub shake: [f32; 2],
    pub flash: f32,
    pub turn: f32,
    pub cuts: u32,
    /// Whether a cut happened this frame.
    pub cut: bool,
    pub chaos: f32,
    pub harmony: f32,
    /// Hits this frame, for scenes that count them: kick or attack, snare,
    /// impact.
    pub kicked: bool,
    pub snared: bool,
    pub impacted: bool,
}

/// A tiny random generator (xorshift32) for the scenes' own choices:
/// the next number from `seed`, in `-1..1`. `seed` must not be 0.
pub(crate) fn random(seed: &mut u32) -> f32 {
    *seed ^= *seed << 13;
    *seed ^= *seed >> 17;
    *seed ^= *seed << 5;
    *seed as f32 / u32::MAX as f32 * 2.0 - 1.0
}

/// Turns each frame's signals into springs, jolts and cuts.
pub(crate) struct Reactor {
    punch: Spring,
    snap: Spring,
    shimmer: Spring,
    slam: Spring,
    shake: [Spring; 2],
    flash: f32,
    turn: f32,
    cuts: u32,
    snares: u32,
    since_cut: f32,
    chaos: f32,
    harmony: f32,
    edges: [Edge; 5],
    /// A tiny random generator for jolt directions.
    seed: u32,
}

/// Seconds the flash takes to fade.
const FLASH_FADE: f32 = 0.09;
/// While chaos is above this, hard hits flash and cut.
const WILD: f32 = 0.55;
/// The shortest time between two cuts made by hits (impacts always cut).
const CUT_REST: f32 = 0.45;

impl Reactor {
    pub fn new(seed: u32) -> Self {
        Self {
            punch: Spring::new(3.5, 0.28),
            snap: Spring::new(5.0, 0.3),
            shimmer: Spring::new(8.0, 0.4),
            slam: Spring::new(1.2, 0.22),
            shake: [Spring::new(6.0, 0.25), Spring::new(6.0, 0.25)],
            flash: 0.0,
            turn: 1.0,
            cuts: 0,
            snares: 0,
            since_cut: 0.0,
            chaos: 0.0,
            harmony: 0.0,
            edges: [Edge::default(); 5],
            seed: seed | 1,
        }
    }

    fn random(&mut self) -> f32 {
        random(&mut self.seed)
    }

    /// This frame's pulse. `motion` is 1 normally and 0 with reduced
    /// motion, which stops everything.
    pub fn update(&mut self, s: &Signals, motion: f32) -> Pulse {
        let dt = s.dt.clamp(0.0, 0.1) * motion;
        let events = [s.kick, s.snare, s.cymbals, s.hit, s.impact];
        let mut rises = [0.0; 5];
        for ((rise, edge), event) in rises.iter_mut().zip(&mut self.edges).zip(events) {
            *rise = edge.rise(event * motion);
        }
        let [kick, snare, cymbals, hit, impact] = rises;

        // Chaos rises within a tenth of a second and settles over two.
        let seconds = if s.chaos > self.chaos { 0.1 } else { 2.0 };
        self.chaos += (s.chaos * motion - self.chaos) * (1.0 - (-dt.max(1e-6) / seconds).exp());
        let total = s.percussive + s.harmonic;
        let harmony = if total > 1e-3 { s.harmonic / total } else { 0.5 };
        self.harmony += (harmony - self.harmony) * (1.0 - (-dt / 0.5).exp());
        let wild = self.chaos > WILD;

        // The percussion moves. A generic attack stands in for the kick
        // when the drums go unheard, which in a wall is most of the time.
        let percussive = (0.5 + 0.5 * s.percussive).min(1.0);
        let strike = kick.max(0.6 * hit);
        if strike > 0.0 {
            self.punch.strike(strike * percussive);
        }
        if snare > 0.0 {
            self.snap.strike(snare * percussive);
            self.snares += 1;
        }
        if cymbals > 0.0 {
            self.shimmer.strike(0.6 * cymbals);
        }
        if impact > 0.0 {
            self.slam.strike(1.0);
        }
        let jolt = impact.max(if wild { strike.max(snare) } else { 0.5 * snare });
        if jolt > 0.0 {
            let (x, y) = (self.random(), self.random());
            self.shake[0].strike(jolt * x);
            self.shake[1].strike(jolt * y);
        }

        // Cuts: every impact; while wild, a hard hit at most every
        // `CUT_REST`, and every fourth snare reverses the turn.
        self.since_cut += dt;
        let mut cut = impact > 0.0;
        if wild && strike.max(snare) > 0.8 && self.since_cut >= CUT_REST {
            cut = true;
        }
        if impact > 0.0 || (wild && snare > 0.0 && self.snares % 4 == 0) {
            self.turn = -self.turn;
        }
        if cut {
            self.cuts = self.cuts.wrapping_add(1);
            self.since_cut = 0.0;
        }

        // Light: an impact flashes the whole screen.
        self.flash *= (-dt / FLASH_FADE).exp();
        self.flash = self.flash.max(impact);

        Pulse {
            punch: self.punch.step(dt),
            snap: self.snap.step(dt),
            shimmer: self.shimmer.step(dt),
            slam: self.slam.step(dt),
            shake: [self.shake[0].step(dt), self.shake[1].step(dt)],
            flash: self.flash,
            turn: self.turn,
            cuts: self.cuts,
            cut,
            chaos: self.chaos,
            harmony: self.harmony,
            kicked: strike > 0.0,
            snared: snare > 0.0,
            impacted: impact > 0.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_struck_spring_jumps_overshoots_and_settles() {
        let mut spring = Spring::new(4.0, 0.25);
        spring.strike(1.0);
        let path: Vec<f32> = (0..120).map(|_| spring.step(1.0 / 60.0)).collect();
        let peak = path.iter().copied().fold(0.0, f32::max);
        let trough = path.iter().copied().fold(0.0, f32::min);
        assert!(peak > 1.0, "{peak}");
        assert!(trough < -0.2, "it should recoil: {trough}");
        assert!(path[119].abs() < 0.05, "settled within two seconds: {}", path[119]);
    }

    #[test]
    fn stiff_springs_stay_stable_at_a_low_frame_rate() {
        let mut spring = Spring::new(8.0, 0.2);
        spring.strike(1.0);
        for _ in 0..100 {
            assert!(spring.step(0.1).abs() < 3.0);
        }
    }

    #[test]
    fn an_impact_cuts_flips_and_flashes() {
        let mut reactor = Reactor::new(1);
        let quiet = Signals { dt: 1.0 / 60.0, ..Signals::default() };
        let before = reactor.update(&quiet, 1.0);
        let slam = reactor.update(&Signals { impact: 1.0, ..quiet.clone() }, 1.0);
        assert_eq!(slam.cuts, before.cuts + 1);
        assert_eq!(slam.turn, -before.turn);
        assert!(slam.flash > 0.9);
        // Held at 1 it is the same impact, not a new one.
        let held = reactor.update(&Signals { impact: 0.98, ..quiet.clone() }, 1.0);
        assert_eq!(held.cuts, slam.cuts);
    }

    #[test]
    fn reduced_motion_feels_nothing() {
        let mut reactor = Reactor::new(1);
        let loud = Signals { dt: 1.0 / 60.0, kick: 1.0, snare: 1.0, impact: 1.0, hit: 1.0, chaos: 1.0, ..Signals::default() };
        let pulse = reactor.update(&loud, 0.0);
        assert_eq!(pulse.cuts, 0);
        assert_eq!(pulse.punch, 0.0);
        assert_eq!(pulse.flash, 0.0);
    }
}
