//! When to change scene, to which one, and how.
//!
//! A VJ doesn't change the picture on a timer. They wait for the music to
//! turn a corner (a drop, a breakdown, a new song) and change on the beat,
//! to a picture that suits what comes next. This does the same with what
//! the director can hear:
//!
//! - **When:** at a *moment*, once the scene has had its time: the analysis'
//!   novelty jumping (a new song, a drop), or the feel of the music shifting
//!   (pulse, brightness and swell, against six seconds earlier). If no
//!   moment comes, after a while anyway, but not in the middle of a
//!   build-up: a VJ waits for the drop. Always on a beat when there is one,
//!   detected or counted.
//! - **Drops:** the deep bass hitting hard during a build-up, or just as
//!   it lets go, is the drop: a moment of its own, since novelty often
//!   stays high through a build-up and has nothing left to jump with. It
//!   needs no listening to tell what it is, so the change lands on the
//!   drop's first beat, and a scene may give way to it at half its time.
//!   Other moments in a build-up (the snare roll speeding up) are part of
//!   it, and change nothing.
//! - **Impacts:** the band slamming back in after a break, or a part far
//!   louder than the one before (`Signals::impact`), is a moment too, and
//!   the strongest one heavy music has: the change lands on the impact
//!   itself, not the next beat, with a near cut, and a scene may give way
//!   to it after a few seconds. Only an impact after a while without
//!   one counts: a stop-start riff slams back in every second or two.
//! - **To what:** the scene whose character (`Character`) is closest to the
//!   music now, among those not shown lately. Heavy music (`Signals::chaos`:
//!   loud, gritty, dense) asks for the wildest scenes, and shortens their
//!   time.
//! - **How:** by what the moment is (`Transition`): rising music opens the
//!   new scene from the middle, falling music closes the old one into it,
//!   quiet music lets it seep in, anything else sweeps it in.
//!
//! Measured on five minutes of dense metal, novelty only jumped when a song
//! started; within a four-and-a-half-minute song it stayed at zero while the
//! feel shifted twice. Hence both.

use std::collections::VecDeque;

use juglar_signals::Signals;

use crate::feel::Feel;
use crate::intensity::arousal;
use crate::transition::{Style, Transition};

/// What a scene is like, to match it with music.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Character {
    /// How lively it looks, `0..=1`: the music's liveliness (`arousal`)
    /// it suits.
    pub energy: f32,
    /// How much it moves with the beats rather than flowing, `0..=1`.
    pub pulse: f32,
    /// Whether it moves the whole picture (zooms, turns, a tunnel), not
    /// only things within it. The calm modes leave these out.
    pub flow: bool,
}

/// What made the director change scene.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Moment {
    /// The music let go of a build-up: the drop it was building to.
    Drop,
    /// The music grew suddenly: a drop nobody built up to, a song starting.
    Rise,
    /// It thinned out: a breakdown, an ending.
    Fall,
    /// It changed without growing or thinning.
    Turn,
    /// The music slammed in after a break: an explosion.
    Impact,
    /// Nothing happened for a long time.
    Long,
    /// A player said a new song started.
    Song,
}

/// A scene change the director decided on.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Change {
    pub scene: usize,
    pub transition: Transition,
    pub moment: Moment,
}

/// How long a scene stays, and how changes look: the calm modes are slower.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pacing {
    /// No automatic change before a scene has shown this long, in seconds.
    pub shortest: f32,
    /// A change comes at most this long after the last one, moment or not.
    pub longest: f32,
    /// Whether only scenes that don't move the whole picture are chosen.
    pub still_camera: bool,
    /// Every transition seeps in over this long instead (the calm modes),
    /// or `None` for transitions that follow the moment.
    pub seep_seconds: Option<f32>,
}

impl Pacing {
    pub const LIVELY: Pacing = Pacing { shortest: 30.0, longest: 100.0, still_camera: false, seep_seconds: None };
}

/// Novelty above this is a moment.
const NOVELTY_MOMENT: f32 = 0.5;
/// A shift of the feel above this is a moment (see `shift`). Over five
/// minutes of metal, 5% of seconds were above it, in four clusters.
const SHIFT_MOMENT: f32 = 0.4;
/// The shift compares the feel with this many seconds ago.
const SHIFT_SECONDS: f32 = 6.0;
/// How often the feel is remembered for that comparison.
const SNAPSHOT_SECONDS: f32 = 0.5;
/// After a moment, the director listens this long to hear where the music
/// goes before deciding what kind of moment it was: at the first frame of
/// a drop, only the novelty knows; the loudness hasn't moved yet.
const LISTEN: f32 = 0.5;
/// Loudness (`energy`) gained or lost across a moment that makes it a rise
/// or a fall.
const RISING: f32 = 0.12;
/// Once a change is due, how long it waits for a beat to land on.
const BEAT_WAIT: f32 = 0.75;
/// A moment that came while the scene was still young still counts this
/// long after it: the change follows it a little late rather than never.
const MOMENT_KEEP: f32 = 4.0;
/// Scenes shown within the last this many changes are avoided.
const RECENT: usize = 4;
/// Tension (`Feel::tension`) above this is a build-up.
const TENSE: f32 = 0.4;
/// The deep bass hitting within this long of a build-up can be its drop.
const DROP_WINDOW: f32 = 1.5;
/// How much harder than lately the deep bass must hit (`Feel::punch`) to
/// be the drop, and how much harder than anything in the build-up's last
/// few seconds: some build-ups keep a kick. Measured on a build-up and
/// drop played through the speakers, a snare roll's hits reached 0.13 and
/// the drop's first kick 0.62; in the synthetic song, the build-up's kicks
/// reach 0.32-0.41 and the drop 0.54.
const DROP_PUNCH: f32 = 0.3;
const DROP_OVER_BUILD: f32 = 1.25;
/// The build-up's punches are remembered in slots of this long, this many.
const PUNCH_SLOT: f32 = 0.25;
const PUNCH_SLOTS: usize = 16;
/// A change due during a build-up waits for the drop, at most this much
/// longer.
const DROP_WAIT: f32 = 30.0;
/// An impact (`Signals::impact` jumping) is a moment only after this long
/// without one. Pantera's "Walk" slams back in every one or two seconds,
/// each time an impact; Jezebel's explosion comes after a minute of break.
const IMPACT_REST: f32 = 8.0;
/// A scene may give way to an impact once it has shown this long,
/// however heavy the music: on Jezebel, a change came in the break eight
/// seconds before the explosion, and the explosion still needs its own.
const IMPACT_SHORTEST: f32 = 6.0;
/// How fast the change on an impact is: nearly a cut.
const IMPACT_SECONDS: f32 = 0.3;
/// Chaos, followed over a few seconds, from which the music is heavy, and
/// at which it is as heavy as it gets. Measured, the median over a song:
/// Jezebel and Arise 0.57-0.65, Walk 0.48, Black Hole Sun 0.45, Fell on
/// Black Days and classic rock 0.12-0.2.
const HEAVY_FROM: f32 = 0.3;
const HEAVY_FULL: f32 = 0.55;
/// Heavy music shortens a scene's time by up to this share: a riff that
/// never stops leaves no moment to change on.
const HEAVY_PACE: f32 = 0.5;

/// How much the music changed between two feels: pulse and brightness in
/// full, swell at half (it moves with every long crescendo).
fn shift(now: &Feel, before: &Feel) -> f32 {
    (now.pulse - before.pulse).abs() + (now.brightness - before.brightness).abs() + 0.5 * (now.swell - before.swell).abs()
}

pub struct Schedule {
    characters: Vec<Character>,
    pacing: Pacing,
    /// Seconds since the scene last changed.
    shown: f32,
    /// Scenes shown lately, newest last.
    recent: VecDeque<usize>,
    /// Feels of the last `SHIFT_SECONDS`, oldest first.
    snapshots: VecDeque<Feel>,
    since_snapshot: f32,
    last_novelty: f32,
    last_shift: f32,
    last_punch: f32,
    /// The strongest punch in each of the last `PUNCH_SLOTS` slots, the
    /// current one first, and how far into it we are.
    punches: [f32; PUNCH_SLOTS],
    punch_slot: f32,
    /// Loudness over about the last two seconds, and over the last few
    /// tenths: what a moment is compared across.
    steady: f32,
    quick: f32,
    /// A moment being listened to: the loudness before it, and how long
    /// ago it came.
    listening: Option<(f32, f32)>,
    /// The last moment and how long ago it came (see `MOMENT_KEEP`).
    remembered: Option<(Moment, f32)>,
    /// A change waiting for a beat, and for how long it has waited.
    due: Option<(Moment, f32)>,
    /// Seconds since the music was last building up.
    since_tense: f32,
    /// The last frame's impact, and seconds since one last jumped.
    last_impact: f32,
    since_impact: f32,
    /// How heavy the music is, `0..=1`: chaos, followed over a few seconds.
    heavy: f32,
    holding: bool,
    /// Scenes showing on other displays, avoided.
    elsewhere: Vec<usize>,
}

impl Schedule {
    pub fn new(characters: Vec<Character>, pacing: Pacing) -> Self {
        Self {
            characters,
            pacing,
            shown: 0.0,
            recent: VecDeque::new(),
            snapshots: VecDeque::new(),
            since_snapshot: 0.0,
            last_novelty: 0.0,
            last_shift: 0.0,
            last_punch: 0.0,
            punches: [0.0; PUNCH_SLOTS],
            punch_slot: 0.0,
            steady: 0.0,
            quick: 0.0,
            listening: None,
            remembered: None,
            due: None,
            since_tense: f32::MAX,
            last_impact: 0.0,
            since_impact: f32::MAX,
            heavy: 0.0,
            holding: false,
            elsewhere: Vec::new(),
        }
    }

    pub fn set_pacing(&mut self, pacing: Pacing) {
        self.pacing = pacing;
    }

    /// Keeps the current scene: no automatic changes while held.
    pub fn hold(&mut self, holding: bool) {
        self.holding = holding;
        self.due = None;
    }

    pub fn holding(&self) -> bool {
        self.holding
    }

    /// The listener (or anyone else) changed scene: the new one gets its
    /// full time.
    /// The scenes other displays show, for `choose` to avoid.
    pub fn elsewhere(&mut self, scenes: &[usize]) {
        self.elsewhere.clear();
        self.elsewhere.extend_from_slice(scenes);
    }

    pub fn changed_to(&mut self, scene: usize) {
        self.shown = 0.0;
        self.due = None;
        self.remembered = None;
        self.recent.push_back(scene);
        while self.recent.len() > RECENT {
            self.recent.pop_front();
        }
    }

    /// Follows the music; returns a change when one is due. `scene` is the
    /// one showing; `busy` says a transition is under way.
    pub fn update(&mut self, s: &Signals, feel: &Feel, beat: bool, scene: usize, busy: bool) -> Option<Change> {
        let dt = s.dt.clamp(0.0, 0.25);
        self.shown += dt;

        // Remember the feel every half second, for the shift.
        self.since_snapshot += dt;
        if self.since_snapshot >= SNAPSHOT_SECONDS {
            self.since_snapshot = 0.0;
            self.snapshots.push_back(*feel);
            while self.snapshots.len() as f32 * SNAPSHOT_SECONDS > SHIFT_SECONDS {
                self.snapshots.pop_front();
            }
        }
        let shift = self.snapshots.front().map_or(0.0, |before| shift(feel, before));

        // A moment is novelty or shift crossing its threshold upward.
        let jumped = s.novelty > NOVELTY_MOMENT && self.last_novelty <= NOVELTY_MOMENT;
        let shifted = shift > SHIFT_MOMENT && self.last_shift <= SHIFT_MOMENT;
        self.last_novelty = s.novelty;
        self.last_shift = shift;
        let follow = |value: &mut f32, seconds: f32| *value += (s.energy - *value) * (1.0 - (-dt / seconds).exp());
        follow(&mut self.quick, 0.25);
        self.remembered = self.remembered.map(|(moment, age)| (moment, age + dt)).filter(|&(_, age)| age <= MOMENT_KEEP);
        let building = feel.tension > TENSE;
        self.since_tense = if building { 0.0 } else { self.since_tense + dt };
        // Compared with the build-up's punches from half a second ago and
        // earlier: the last two slots may hold this same kick's attack.
        let before = self.punches[2..].iter().copied().fold(0.0, f32::max);
        let needed = DROP_PUNCH.max(DROP_OVER_BUILD * before);
        let punched = feel.punch > needed && self.last_punch <= needed;
        self.last_punch = feel.punch;
        self.punch_slot += dt;
        if self.punch_slot >= PUNCH_SLOT {
            self.punch_slot = 0.0;
            self.punches.rotate_right(1);
            self.punches[0] = 0.0;
        }
        self.punches[0] = self.punches[0].max(feel.punch);
        if punched && self.since_tense <= DROP_WINDOW && feel.sounding {
            // The build-up already said what this is.
            self.remembered = Some((Moment::Drop, 0.0));
            self.listening = None;
        } else if (jumped || shifted) && feel.sounding && self.listening.is_none() && !building {
            self.listening = Some((self.steady, 0.0));
        }
        if let Some((before, age)) = self.listening {
            if age + dt < LISTEN {
                self.listening = Some((before, age + dt));
            } else {
                self.listening = None;
                let moment = if self.quick - before > RISING {
                    Moment::Rise
                } else if before - self.quick > RISING {
                    Moment::Fall
                } else {
                    Moment::Turn
                };
                self.remembered = Some((moment, 0.0));
            }
        }
        // An impact needs no listening either; it is what it is. A drop
        // heard on the same frame stays a drop.
        let heavy = ((s.chaos - HEAVY_FROM) / (HEAVY_FULL - HEAVY_FROM)).clamp(0.0, 1.0);
        self.heavy += (heavy - self.heavy) * (1.0 - (-dt / 3.0).exp());
        let slammed = s.impact - self.last_impact > 0.3;
        self.last_impact = s.impact;
        if slammed {
            let rested = self.since_impact >= IMPACT_REST;
            self.since_impact = 0.0;
            if rested && feel.sounding && !matches!(self.remembered, Some((Moment::Drop, _))) {
                self.remembered = Some((Moment::Impact, 0.0));
                self.listening = None;
            }
        } else {
            self.since_impact += dt;
        }
        // The "before" of a moment stops following while it is heard.
        if self.listening.is_none() {
            follow(&mut self.steady, 1.0);
        }

        if self.holding || busy || !feel.sounding || self.characters.len() < 2 {
            self.due = None;
            return None;
        }
        if self.due.is_none() {
            let pace = 1.0 - HEAVY_PACE * self.heavy;
            let shortest = match self.remembered {
                Some((Moment::Drop, _)) => 0.5 * pace * self.pacing.shortest,
                // The calm modes take an impact as any other moment.
                Some((Moment::Impact, _)) if self.pacing.seep_seconds.is_none() => IMPACT_SHORTEST,
                _ => pace * self.pacing.shortest,
            };
            let longest = pace * self.pacing.longest;
            let waited_for_drop = !building || self.shown >= longest + DROP_WAIT;
            if self.shown >= shortest
                && let Some((moment, _)) = self.remembered.take()
            {
                self.due = Some((moment, 0.0));
            } else if self.shown >= longest && waited_for_drop {
                self.due = Some((Moment::Long, 0.0));
            }
        }
        // Land on a beat, or go after waiting a little for one; an impact
        // is its own beat.
        let (moment, waited) = self.due?;
        if !beat && moment != Moment::Impact && waited < BEAT_WAIT {
            self.due = Some((moment, waited + dt));
            return None;
        }
        self.due = None;
        // An explosion asks for the wildest scene, however quiet the break
        // before it was.
        let heavy = if moment == Moment::Impact { 1.0 } else { self.heavy };
        let next = self.choose_for(feel, scene, heavy);
        self.changed_to(next);
        Some(Change { scene: next, transition: self.transition(moment, feel), moment })
    }

    /// A new song started (a player said so): a new scene at once, chosen
    /// as for any moment from how the music felt lately, unless the scene
    /// is held.
    pub fn new_song(&mut self, feel: &Feel, scene: usize) -> Option<Change> {
        if self.holding || self.characters.len() < 2 {
            return None;
        }
        let next = self.choose_for(feel, scene, self.heavy);
        self.changed_to(next);
        Some(Change { scene: next, transition: self.transition(Moment::Song, feel), moment: Moment::Song })
    }

    /// The scene closest in character to the music, avoiding the one
    /// showing and those shown lately.
    ///
    /// The music's liveliness is stretched before it is matched: over 15
    /// minutes of real music (metal, dense and bright) it stayed between
    /// 0.3 and 0.65, while the scenes' energies spread from 0.2 to 0.9, and
    /// matched as it was, the liveliest scenes never came up. Stretched, 0.2
    /// and below asks for the calmest scene, 0.7 and above for the wildest.
    /// Heavy music asks for the wildest whatever the arousal: a wall of
    /// guitars at a slow tempo (doom, sludge) is no time for a calm scene.
    pub fn choose(&self, feel: &Feel, scene: usize) -> usize {
        self.choose_for(feel, scene, self.heavy)
    }

    fn choose_for(&self, feel: &Feel, scene: usize, heavy: f32) -> usize {
        let lively = ((arousal(feel) - 0.2) / 0.5).clamp(0.0, 1.0).max(heavy);
        let cost = |i: usize| {
            let c = &self.characters[i];
            // The pulse counts less the heavier the music: in a wall of
            // guitars the beats go unheard (pulse reads 0.1 on Walk and
            // Jezebel), and matching it would pick the scenes that move
            // least with the drums.
            let mismatch = (c.energy - lively).abs() + 0.5 * (1.0 - heavy) * (c.pulse - feel.pulse).abs();
            // Shown lately: the more recently, the less welcome.
            let staleness = match self.recent.iter().rev().position(|&r| r == i) {
                Some(age) => 1.0 - age as f32 / RECENT as f32,
                None => 0.0,
            };
            // On another display already: only if nothing else fits.
            let taken = if self.elsewhere.contains(&i) { 2.0 } else { 0.0 };
            mismatch + staleness + taken
        };
        (0..self.characters.len())
            .filter(|&i| i != scene)
            .filter(|&i| !self.pacing.still_camera || !self.characters[i].flow)
            .min_by(|&a, &b| cost(a).total_cmp(&cost(b)))
            .unwrap_or(scene)
    }

    fn transition(&self, moment: Moment, feel: &Feel) -> Transition {
        if let Some(seconds) = self.pacing.seep_seconds {
            return Transition { style: Style::Seep, seconds, per_beat: 0.0 };
        }
        match moment {
            Moment::Drop => Transition { style: Style::Bloom, seconds: 1.5, per_beat: 0.12 },
            Moment::Impact => Transition { style: Style::Bloom, seconds: IMPACT_SECONDS, per_beat: 0.0 },
            // Under the song's title, quick, whatever the last song's end.
            Moment::Song => Transition { style: Style::Bloom, seconds: 1.2, per_beat: 0.0 },
            _ if arousal(feel) < 0.35 => Transition { style: Style::Seep, seconds: 6.0, per_beat: 0.0 },
            Moment::Rise => Transition { style: Style::Bloom, seconds: 2.5, per_beat: 0.12 },
            Moment::Fall => Transition { style: Style::Iris, seconds: 4.0, per_beat: 0.06 },
            Moment::Turn | Moment::Long => Transition { style: Style::Spiral, seconds: 3.0, per_beat: 0.1 },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::feel::Listener;
    use juglar_signals::{SPECTRUM_BANDS, Synthetic};

    #[test]
    fn a_new_song_changes_scene_unless_held() {
        let mut schedule = Schedule::new(characters(), Pacing::LIVELY);
        let feel = Listener::new().feel();
        let change = schedule.new_song(&feel, 0).expect("a change");
        assert_ne!(change.scene, 0);
        assert_eq!(change.moment, Moment::Song);
        schedule.hold(true);
        assert!(schedule.new_song(&feel, change.scene).is_none());
    }

    fn characters() -> Vec<Character> {
        [(0.5, 0.6, true), (0.2, 0.2, false), (0.85, 0.7, true), (0.3, 0.3, false), (0.6, 0.5, false), (0.9, 0.8, true)]
            .into_iter()
            .map(|(energy, pulse, flow)| Character { energy, pulse, flow })
            .collect()
    }

    /// Plays `seconds` of `next`, changing scene whenever told; returns the
    /// changes with their times.
    fn run(seconds: f32, pacing: Pacing, mut next: impl FnMut(&mut Signals)) -> Vec<(f32, Change)> {
        let mut schedule = Schedule::new(characters(), pacing);
        let mut listener = Listener::new();
        let mut s = Signals::default();
        let mut scene = 0;
        let mut changes = Vec::new();
        let mut t = 0.0;
        while t < seconds {
            next(&mut s);
            let beat = listener.on_beat(&s);
            let feel = listener.update(&s);
            if let Some(change) = schedule.update(&s, &feel, beat, scene, false) {
                scene = change.scene;
                changes.push((t, change));
            }
            t += s.dt;
        }
        changes
    }

    fn song() -> impl FnMut(&mut Signals) {
        let mut song = Synthetic::new(9);
        move |s| song.next(1.0 / 60.0, s)
    }

    #[test]
    fn changes_on_the_drop() {
        // Sections of 16 s: the build-up from 16 s announces the drop at
        // 32 s, and the change lands on its first beat.
        let changes = run(40.0, Pacing::LIVELY, song());
        let &(t, change) = changes.first().expect("a change");
        assert!((31.95..32.05).contains(&t), "at {t}");
        assert_eq!(change.moment, Moment::Drop);
        assert_eq!(change.transition.style, Style::Bloom);
    }

    #[test]
    fn a_build_up_holds_back_a_change_that_is_due() {
        // Steady music for 90 s, a build-up for 20 s (hi-hats growing, the
        // bass held), then the drop. The change due at 100 s waits for it.
        let mut t = 0.0f32;
        let changes = run(130.0, Pacing::LIVELY, move |s| {
            t += 1.0 / 60.0;
            let since = (t * 2.0).fract() / 2.0;
            let beat = (-since / 0.15).exp();
            let (bass, treble, novelty) = match t {
                ..90.0 => (0.5, 0.3, 0.0),
                ..110.0 => (0.2, 0.3 + 0.4 * (t - 90.0) / 20.0, 0.0),
                _ => (0.9, 0.6, (-(t - 110.0) / 0.8).exp()),
            };
            let spectrum = [bass; SPECTRUM_BANDS];
            *s = Signals { dt: 1.0 / 60.0, energy: 0.6, bass, mid: 0.5, treble, beat, novelty, spectrum, ..Signals::default() };
        });
        let times: Vec<(f32, Moment)> = changes.iter().map(|&(t, c)| (t, c.moment)).collect();
        assert_eq!(times.len(), 1, "{times:?}");
        let (t, moment) = times[0];
        assert!((109.95..110.6).contains(&t) && moment == Moment::Drop, "{times:?}");
    }

    #[test]
    fn changes_on_a_counted_beat_when_none_is_heard() {
        // A steady tempo with no detected beats at all: changes still land
        // where the count puts the beat.
        let mut t = 0.0f32;
        let changes = run(220.0, Pacing::LIVELY, move |s| {
            t += 1.0 / 60.0;
            *s = Signals { dt: 1.0 / 60.0, energy: 0.6, bass: 0.6, mid: 0.5, treble: 0.4, tempo: 120.0, beat_phase: (t * 2.0).fract(), ..Signals::default() };
        });
        assert!(!changes.is_empty());
        for (t, _) in changes {
            assert!(((t * 2.0).round() - t * 2.0).abs() < 0.05, "at {t}");
        }
    }

    #[test]
    fn never_too_soon_never_too_late() {
        let changes = run(600.0, Pacing::LIVELY, song());
        assert!(changes.len() >= 5, "{} changes", changes.len());
        for pair in changes.windows(2) {
            let gap = pair[1].0 - pair[0].0;
            assert!(gap >= Pacing::LIVELY.shortest, "{gap}");
            assert!(gap <= Pacing::LIVELY.longest + 1.0, "{gap}");
        }
    }

    #[test]
    fn steady_music_still_changes() {
        // A steady beat with no moments at all: a change every `longest`.
        let mut t = 0.0f32;
        let changes = run(330.0, Pacing::LIVELY, move |s| {
            t += 1.0 / 60.0;
            let since = (t * 2.0).fract() / 2.0;
            *s = Signals { dt: 1.0 / 60.0, energy: 0.6, bass: 0.6, mid: 0.5, treble: 0.4, beat: (-since / 0.15).exp(), ..Signals::default() };
        });
        let times: Vec<f32> = changes.iter().map(|&(t, c)| {
            assert_eq!(c.moment, Moment::Long);
            t
        }).collect();
        assert_eq!(times.len(), 3, "{times:?}");
    }

    #[test]
    fn no_scene_comes_back_too_soon() {
        let changes = run(900.0, Pacing::LIVELY, song());
        let scenes: Vec<usize> = changes.iter().map(|(_, c)| c.scene).collect();
        for window in scenes.windows(3) {
            assert!(window[0] != window[1] && window[1] != window[2] && window[0] != window[2], "{scenes:?}");
        }
    }

    #[test]
    fn calm_pacing_keeps_the_camera_still() {
        let calm = Pacing { shortest: 60.0, longest: 120.0, still_camera: true, seep_seconds: Some(15.0) };
        let changes = run(900.0, calm, song());
        assert!(!changes.is_empty());
        for (_, change) in changes {
            assert!(!characters()[change.scene].flow, "{change:?}");
            assert_eq!(change.transition.style, Style::Seep);
        }
    }

    #[test]
    fn lively_music_gets_a_lively_scene() {
        let schedule = Schedule::new(characters(), Pacing::LIVELY);
        let loud = Feel { pulse: 0.9, loudness: 0.9, brightness: 0.8, swell: 0.8, rise: 0.0, tension: 0.0, punch: 0.0, sounding: true };
        let quiet = Feel { pulse: 0.0, loudness: 0.5, brightness: 0.1, swell: 0.4, rise: 0.0, tension: 0.0, punch: 0.0, sounding: true };
        assert_eq!(schedule.choose(&loud, 0), 5);
        assert_eq!(schedule.choose(&quiet, 0), 1);
    }

    /// Heavy steady music with impacts at the given times: a slam that
    /// jumps to 1 and falls over 0.6 s.
    fn slams(at: &'static [f32]) -> impl FnMut(&mut Signals) {
        let mut t = 0.0f32;
        move |s| {
            t += 1.0 / 60.0;
            let since = (t * 2.0).fract() / 2.0;
            let impact = at.iter().filter(|&&a| t >= a).map(|&a| (-(t - a) / 0.6).exp()).fold(0.0, f32::max);
            *s = Signals { dt: 1.0 / 60.0, energy: 0.6, bass: 0.6, mid: 0.5, treble: 0.4, beat: (-since / 0.15).exp(), chaos: 0.75, impact, ..Signals::default() };
        }
    }

    #[test]
    fn changes_on_an_impact_at_once() {
        // An impact needs the scene to have shown 6 s; the one at 9.13 s
        // lands off the beat, and the change comes on it all the same,
        // nearly a cut.
        let changes = run(20.0, Pacing::LIVELY, slams(&[9.13]));
        let &(t, change) = changes.first().expect("a change");
        assert!((9.1..9.2).contains(&t), "at {t}");
        assert_eq!(change.moment, Moment::Impact);
        assert!(change.transition.seconds < 0.5);
    }

    #[test]
    fn a_stop_start_riff_is_one_impact() {
        // Slams every 1.5 s from 20 s to 60 s: the first changes the
        // scene, and the rest are the riff, not new moments.
        const RIFF: [f32; 27] = {
            let mut at = [0.0; 27];
            let mut i = 0;
            while i < 27 {
                at[i] = 20.0 + 1.5 * i as f32;
                i += 1;
            }
            at
        };
        let changes = run(60.0, Pacing::LIVELY, slams(&RIFF));
        let moments: Vec<(f32, Moment)> = changes.iter().map(|&(t, c)| (t, c.moment)).collect();
        assert_eq!(moments.len(), 1, "{moments:?}");
        assert_eq!(moments[0].1, Moment::Impact);
    }

    #[test]
    fn heavy_music_gets_a_wild_scene() {
        // Slow and dark, as doom is, but a wall of guitars.
        let mut schedule = Schedule::new(characters(), Pacing::LIVELY);
        let slow = Feel { pulse: 0.1, loudness: 0.6, brightness: 0.2, swell: 0.5, rise: 0.0, tension: 0.0, punch: 0.0, sounding: true };
        assert_eq!(schedule.choose(&slow, 0), 1);
        let s = Signals { dt: 0.1, chaos: 0.8, ..Signals::default() };
        for _ in 0..100 {
            schedule.update(&s, &slow, false, 0, true);
        }
        assert_eq!(schedule.choose(&slow, 0), 5);
    }

    #[test]
    fn silence_changes_nothing() {
        let changes = run(300.0, Pacing::LIVELY, |s| *s = Signals { dt: 1.0 / 60.0, ..Signals::default() });
        assert!(changes.is_empty());
    }
}
