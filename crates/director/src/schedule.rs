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
//!   moment comes, after a while anyway. Always on a beat when there is one.
//! - **To what:** the scene whose character (`Character`) is closest to the
//!   music now, among those not shown lately.
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
    /// The music grew suddenly: a drop, a song starting.
    Rise,
    /// It thinned out: a breakdown, an ending.
    Fall,
    /// It changed without growing or thinning.
    Turn,
    /// Nothing happened for a long time.
    Long,
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
    holding: bool,
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
            steady: 0.0,
            quick: 0.0,
            listening: None,
            remembered: None,
            due: None,
            holding: false,
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
        if (jumped || shifted) && feel.sounding && self.listening.is_none() {
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
        // The "before" of a moment stops following while it is heard.
        if self.listening.is_none() {
            follow(&mut self.steady, 1.0);
        }

        if self.holding || busy || !feel.sounding || self.characters.len() < 2 {
            self.due = None;
            return None;
        }
        if self.due.is_none() && self.shown >= self.pacing.shortest {
            if let Some((moment, _)) = self.remembered.take() {
                self.due = Some((moment, 0.0));
            } else if self.shown >= self.pacing.longest {
                self.due = Some((Moment::Long, 0.0));
            }
        }
        // Land on a beat, or go after waiting a little for one.
        let (moment, waited) = self.due?;
        if !beat && waited < BEAT_WAIT {
            self.due = Some((moment, waited + dt));
            return None;
        }
        self.due = None;
        let next = self.choose(feel, scene);
        self.changed_to(next);
        Some(Change { scene: next, transition: self.transition(moment, feel), moment })
    }

    /// The scene closest in character to the music, avoiding the one
    /// showing and those shown lately.
    ///
    /// The music's liveliness is stretched before it is matched: over 15
    /// minutes of real music (metal, dense and bright) it stayed between
    /// 0.3 and 0.65, while the scenes' energies spread from 0.2 to 0.9, and
    /// matched as it was, the liveliest scenes never came up. Stretched, 0.2
    /// and below asks for the calmest scene, 0.7 and above for the wildest.
    pub fn choose(&self, feel: &Feel, scene: usize) -> usize {
        let lively = ((arousal(feel) - 0.2) / 0.5).clamp(0.0, 1.0);
        let cost = |i: usize| {
            let c = &self.characters[i];
            let mismatch = (c.energy - lively).abs() + 0.5 * (c.pulse - feel.pulse).abs();
            // Shown lately: the more recently, the less welcome.
            let staleness = match self.recent.iter().rev().position(|&r| r == i) {
                Some(age) => 1.0 - age as f32 / RECENT as f32,
                None => 0.0,
            };
            mismatch + staleness
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
    use juglar_signals::Synthetic;

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
            let beat = listener.is_beat(&s);
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
    fn changes_at_the_drop_on_a_beat() {
        // Sections of 16 s: the first change waits for the scene's 30 s,
        // then for the next moment, the drop at 32 s, listens half a second
        // and lands on the next beat.
        let changes = run(40.0, Pacing::LIVELY, song());
        let &(t, change) = changes.first().expect("a change");
        assert!((32.4..33.1).contains(&t), "at {t}");
        assert_eq!(change.moment, Moment::Rise);
        assert_eq!(change.transition.style, Style::Bloom);
        // On the beat: the drop's beats fall on whole half seconds.
        assert!(((t * 2.0).round() - t * 2.0).abs() < 0.05, "at {t}");
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
        let loud = Feel { pulse: 0.9, loudness: 0.9, brightness: 0.8, swell: 0.8, rise: 0.0, sounding: true };
        let quiet = Feel { pulse: 0.0, loudness: 0.5, brightness: 0.1, swell: 0.4, rise: 0.0, sounding: true };
        assert_eq!(schedule.choose(&loud, 0), 5);
        assert_eq!(schedule.choose(&quiet, 0), 1);
    }

    #[test]
    fn silence_changes_nothing() {
        let changes = run(300.0, Pacing::LIVELY, |s| *s = Signals { dt: 1.0 / 60.0, ..Signals::default() });
        assert!(changes.is_empty());
    }
}
