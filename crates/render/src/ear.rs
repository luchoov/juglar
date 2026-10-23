//! The analysis view (`Tab`): what Juglar hears, labeled (D59).
//!
//! Drawn entirely with `ink`: the wave as heard; the last eight seconds of
//! the heavy ear (loudness, grit, density, chaos), the drums as marks, the
//! percussive and harmonic parts against each other; the beats, the impacts
//! and the director's changes across all of them; and on the right, the
//! spectrum, the values now, and why the scene last changed. Every signal
//! keeps its color wherever it appears.

use std::collections::VecDeque;

use juglar_signals::{SPECTRUM_BANDS, Signals, WAVEFORM_POINTS};

use crate::hud::{GRAY, INK};
use crate::ink::{Ink, Weight};

/// Seconds of history on the timeline.
const WINDOW: f32 = 8.0;
/// Director changes listed on the right.
const CHANGES: usize = 4;

const fn rgb(hex: u32) -> [f32; 3] {
    [((hex >> 16) & 255) as f32 / 255.0, ((hex >> 8) & 255) as f32 / 255.0, (hex & 255) as f32 / 255.0]
}

const BACKGROUND: [f32; 3] = rgb(0x0a0a0d);
const RULE: [f32; 3] = rgb(0x24242c);
const LOUDNESS: [f32; 3] = rgb(0xe4e4e7);
const GRIT: [f32; 3] = rgb(0xa3e635);
const DENSITY: [f32; 3] = rgb(0x38bdf8);
const CHAOS: [f32; 3] = rgb(0xf43f5e);
const KICK: [f32; 3] = rgb(0xf97316);
const SNARE: [f32; 3] = rgb(0x22d3ee);
const CYMBALS: [f32; 3] = rgb(0xfacc15);
const PERCUSSIVE: [f32; 3] = rgb(0xfb7185);
const HARMONIC: [f32; 3] = rgb(0x818cf8);
const IMPACT: [f32; 3] = rgb(0xffffff);

/// One frame of the levels the timeline draws.
struct Sample {
    time: f32,
    loudness: f32,
    grit: f32,
    density: f32,
    chaos: f32,
    percussive: f32,
    harmonic: f32,
}

/// Something that happened at a moment: drawn as a mark.
#[derive(Clone, Copy, PartialEq)]
enum Mark {
    Kick(f32),
    Snare(f32),
    Cymbals(f32),
    Beat,
    Impact,
    Change,
}

#[derive(Default)]
pub(crate) struct Ear {
    samples: VecDeque<Sample>,
    marks: VecDeque<(f32, Mark)>,
    /// Last frame's kick, snare, cymbals, beat and impact, to tell a new hit.
    last: [f32; 5],
    /// The director's last changes, newest last: when, why, to which scene.
    changes: VecDeque<(f32, String, &'static str)>,
    now: f32,
    /// The scene showing, and since when.
    scene: (&'static str, f32),
}

impl Ear {
    /// Keeps this frame's signals; called every frame, so the view has its
    /// history the moment it is shown.
    pub(crate) fn hear(&mut self, s: &Signals) {
        self.now = s.time;
        self.samples.push_back(Sample {
            time: s.time,
            loudness: s.loudness,
            grit: s.grit,
            density: s.density,
            chaos: s.chaos,
            percussive: s.percussive,
            harmonic: s.harmonic,
        });
        // An envelope jumping up is a new hit.
        let now = [s.kick, s.snare, s.cymbals, s.beat, s.impact];
        let rose = |i: usize, at: f32| now[i] >= at && now[i] > self.last[i] + 0.3;
        let new: [(bool, Mark); 5] = [
            (rose(0, 0.6), Mark::Kick(s.kick)),
            (rose(1, 0.6), Mark::Snare(s.snare)),
            (rose(2, 0.6), Mark::Cymbals(s.cymbals)),
            (rose(3, 0.5), Mark::Beat),
            (rose(4, 0.7), Mark::Impact),
        ];
        for (happened, mark) in new {
            if happened {
                self.marks.push_back((s.time, mark));
            }
        }
        self.last = now;
        let oldest = s.time - WINDOW - 0.5;
        while self.samples.front().is_some_and(|x| x.time < oldest) {
            self.samples.pop_front();
        }
        while self.marks.front().is_some_and(|&(t, _)| t < oldest) {
            self.marks.pop_front();
        }
    }

    /// The director changed scene, at a moment of the music called `why`.
    pub(crate) fn changed(&mut self, why: &str, scene: &'static str) {
        self.changes.push_back((self.now, why.to_owned(), scene));
        if self.changes.len() > CHANGES {
            self.changes.pop_front();
        }
        self.marks.push_back((self.now, Mark::Change));
    }

    /// Queues the whole view into `ink`, for a screen of `size` pixels.
    pub(crate) fn draw(&mut self, ink: &mut Ink, queue: &wgpu::Queue, size: [f32; 2], s: &Signals, scene: &'static str) {
        if !ink.has_fonts() {
            return;
        }
        if self.scene.0 != scene {
            self.scene = (scene, s.time);
        }
        let [w, h] = size;
        let u = (h * 0.019).clamp(11.0, 28.0);
        let m = (h * 0.05).round();
        let line = (h / 700.0).max(1.0);
        ink.panel([0.0, 0.0, w, h], 0.0, BACKGROUND, 1.0);

        // The title, the scene, and the tempo with a dot on each beat.
        let title = m + u * 1.3;
        ink.text(queue, "what juglar hears", [m, title], u * 1.35, Weight::Strong, INK, 1.0);
        let shown = s.time - self.scene.1;
        let mut x = ink.text(queue, "scene  ", [m, title + u * 1.6], u, Weight::Regular, GRAY, 1.0);
        x = ink.text(queue, &title_case(scene), [x, title + u * 1.6], u, Weight::Strong, INK, 1.0);
        let mut x = ink.text(queue, &format!("  for {}   intensity  ", seconds(shown)), [x, title + u * 1.6], u, Weight::Regular, GRAY, 1.0);
        x = ink.text(queue, &format!("{:.2}", s.intensity), [x, title + u * 1.6], u, Weight::Strong, INK, 1.0);
        let _ = x;
        let big = u * 2.4;
        let (tempo, unit) = if s.tempo > 0.0 { (format!("{:.0}", s.tempo), " bpm") } else { ("\u{2014}".to_owned(), " no steady beat") };
        let unit_width = ink.width(queue, unit, u, Weight::Regular);
        let tempo_width = ink.width(queue, &tempo, big, Weight::Strong);
        let right = w - m;
        ink.text(queue, unit, [right - unit_width, title + u * 0.9], u, Weight::Regular, GRAY, 1.0);
        ink.text(queue, &tempo, [right - unit_width - tempo_width, title + u * 0.9], big, Weight::Strong, INK, 1.0);
        let dot = u * 0.45;
        let center = [right - unit_width - tempo_width - u * 1.2, title + u * 0.9 - big * 0.32];
        ink.panel([center[0] - dot, center[1] - dot, 2.0 * dot, 2.0 * dot], dot, RULE, 1.0);
        ink.panel([center[0] - dot, center[1] - dot, 2.0 * dot, 2.0 * dot], dot, KICK, s.beat.clamp(0.0, 1.0));

        // The wave, across the whole width.
        let wave_top = title + u * 3.2;
        let wave_height = h * 0.11;
        let wave_mid = wave_top + wave_height * 0.5;
        ink.line([m, wave_mid], [w - m, wave_mid], line, RULE, 1.0);
        let step = (w - 2.0 * m) / (WAVEFORM_POINTS - 1) as f32;
        let point = |i: usize| [m + i as f32 * step, wave_mid - s.waveform[i].clamp(-1.0, 1.0) * wave_height * 0.5];
        for i in 1..WAVEFORM_POINTS {
            ink.line(point(i - 1), point(i), line * 1.6, INK, 0.9);
        }

        // The timeline on the left, the panel on the right.
        let top = wave_top + wave_height + u * 2.2;
        let bottom = h - m - u * 1.6;
        let label = u * 7.0;
        let (x0, x1) = (m + label, (w * 0.66).round());
        let (p0, p1) = (x1 + u * 3.0, w - m);
        self.timeline(ink, queue, [m, x0, x1], [top, bottom], u, line, s);
        self.panel(ink, queue, [p0, p1], [top, bottom], u, s);
    }

    /// The tracks over the last `WINDOW` seconds, with their labels in the
    /// column from `xs[0]` and the marks across them.
    fn timeline(&self, ink: &mut Ink, queue: &wgpu::Queue, xs: [f32; 3], ys: [f32; 2], u: f32, line: f32, s: &Signals) {
        let [m, x0, x1] = xs;
        let [top, bottom] = ys;
        let x_at = |t: f32| x1 - (self.now - t) / WINDOW * (x1 - x0);
        // Filled columns on whole pixels, so neighbors neither overlap nor
        // leave seams; `None` where a column would fall within the last.
        let columns = |a: f32, b: f32| {
            let (xa, xb) = (x_at(a).round(), x_at(b).round());
            (xb > xa).then_some((xa, xb - xa))
        };
        // Track heights in shares, and the gap between tracks.
        let shares = [1.0, 0.6, 0.6, 1.0, 1.3, 1.3];
        let gap = u * 1.1;
        let unit = (bottom - top - gap * (shares.len() - 1) as f32) / shares.iter().sum::<f32>();
        let mut bands = Vec::new();
        let mut y = top;
        for share in shares {
            bands.push((y, y + share * unit));
            y += share * unit + gap;
        }

        // The beats, faint, behind everything.
        for &(t, mark) in &self.marks {
            if mark == Mark::Beat && t >= self.now - WINDOW {
                ink.line([x_at(t), top], [x_at(t), bottom], line, RULE, 0.8);
            }
        }

        let label = |ink: &mut Ink, band: (f32, f32), name: &str, value: Option<f32>, color: [f32; 3]| {
            let base = band.0 + u * 0.95;
            ink.panel([m, base - u * 0.62, u * 0.5, u * 0.5], u * 0.12, color, 1.0);
            ink.text(queue, name, [m + u * 0.85, base], u, Weight::Regular, GRAY, 1.0);
            if let Some(v) = value {
                ink.text(queue, &format!("{v:.2}"), [m + u * 0.85, base + u * 1.35], u * 1.05, Weight::Strong, INK, 1.0);
            }
        };
        let samples: Vec<&Sample> = self.samples.iter().filter(|x| x.time >= self.now - WINDOW).collect();
        let area = |ink: &mut Ink, band: (f32, f32), value: fn(&Sample) -> f32, color: [f32; 3]| {
            let (t, b) = band;
            ink.line([x0, b], [x1, b], line, RULE, 1.0);
            for pair in samples.windows(2) {
                let (va, vb) = (value(pair[0]).clamp(0.0, 1.0), value(pair[1]).clamp(0.0, 1.0));
                let ya = b - va * (b - t);
                if let Some((x, width)) = columns(pair[0].time, pair[1].time) {
                    ink.panel([x, ya, width, b - ya], 0.0, color, 0.28);
                }
                ink.line([x_at(pair[0].time), ya], [x_at(pair[1].time), b - vb * (b - t)], line * 1.5, color, 1.0);
            }
        };
        let trace = |ink: &mut Ink, band: (f32, f32), value: fn(&Sample) -> f32, color: [f32; 3]| {
            let (t, b) = band;
            ink.line([x0, b], [x1, b], line, RULE, 1.0);
            for pair in samples.windows(2) {
                let ya = b - value(pair[0]).clamp(0.0, 1.0) * (b - t);
                let yb = b - value(pair[1]).clamp(0.0, 1.0) * (b - t);
                ink.line([x_at(pair[0].time), ya], [x_at(pair[1].time), yb], line * 1.8, color, 1.0);
            }
        };

        area(ink, bands[0], |x| x.loudness, LOUDNESS);
        label(ink, bands[0], "loudness", Some(s.loudness), LOUDNESS);
        trace(ink, bands[1], |x| x.grit, GRIT);
        label(ink, bands[1], "grit", Some(s.grit), GRIT);
        trace(ink, bands[2], |x| x.density, DENSITY);
        label(ink, bands[2], "density", Some(s.density), DENSITY);
        area(ink, bands[3], |x| x.chaos, CHAOS);
        label(ink, bands[3], "chaos", Some(s.chaos), CHAOS);

        // The drums, a row each, a mark per hit as tall as the hit.
        let (dt, db) = bands[4];
        let row = (db - dt) / 3.0;
        for (n, (name, color)) in [("kick", KICK), ("snare", SNARE), ("cymbals", CYMBALS)].into_iter().enumerate() {
            let rb = dt + row * (n + 1) as f32 - row * 0.15;
            ink.line([x0, rb], [x1, rb], line, RULE, 1.0);
            ink.panel([m, rb - row * 0.5 - u * 0.31, u * 0.5, u * 0.5], u * 0.12, color, 1.0);
            ink.text(queue, name, [m + u * 0.85, rb - row * 0.5 + u * 0.35], u, Weight::Regular, GRAY, 1.0);
        }
        for &(t, mark) in &self.marks {
            if t < self.now - WINDOW {
                continue;
            }
            let (n, v, color) = match mark {
                Mark::Kick(v) => (0, v, KICK),
                Mark::Snare(v) => (1, v, SNARE),
                Mark::Cymbals(v) => (2, v, CYMBALS),
                _ => continue,
            };
            let rb = dt + row * (n + 1) as f32 - row * 0.15;
            let x = x_at(t);
            ink.line([x, rb], [x, rb - v.clamp(0.0, 1.0) * row * 0.7], line * 2.5, color, 1.0);
        }

        // Percussive above the middle, harmonic below.
        let (pt, pb) = bands[5];
        let mid = (pt + pb) * 0.5;
        ink.line([x0, mid], [x1, mid], line, RULE, 1.0);
        for pair in samples.windows(2) {
            let Some((x, width)) = columns(pair[0].time, pair[1].time) else { continue };
            let up = pair[0].percussive.clamp(0.0, 1.0) * (mid - pt);
            let down = pair[0].harmonic.clamp(0.0, 1.0) * (pb - mid);
            ink.panel([x, mid - up, width, up], 0.0, PERCUSSIVE, 0.75);
            ink.panel([x, mid, width, down], 0.0, HARMONIC, 0.75);
        }
        let base = pt + u * 0.95;
        ink.panel([m, base - u * 0.62, u * 0.5, u * 0.5], u * 0.12, PERCUSSIVE, 1.0);
        ink.text(queue, "percussive", [m + u * 0.85, base], u, Weight::Regular, GRAY, 1.0);
        ink.panel([m, pb - u * 0.62, u * 0.5, u * 0.5], u * 0.12, HARMONIC, 1.0);
        ink.text(queue, "harmonic", [m + u * 0.85, pb], u, Weight::Regular, GRAY, 1.0);

        // Impacts and the director's changes, across every track.
        for &(t, mark) in &self.marks {
            if t < self.now - WINDOW {
                continue;
            }
            let x = x_at(t);
            match mark {
                Mark::Impact => {
                    ink.line([x, top - u * 0.6], [x, bottom], line * 2.0, IMPACT, 0.85);
                    ink.text(queue, "impact", [x + u * 0.3, top - u * 0.4], u * 0.85, Weight::Strong, IMPACT, 1.0);
                }
                Mark::Change => {
                    let mut y = top - u * 0.6;
                    while y < bottom {
                        ink.line([x, y], [x, (y + u * 0.5).min(bottom)], line * 1.5, INK, 0.7);
                        y += u * 0.9;
                    }
                }
                _ => {}
            }
        }

        // The time axis.
        let axis = bottom + u * 1.3;
        for (t, text) in [(WINDOW, "\u{2212}8 s"), (WINDOW * 0.5, "\u{2212}4 s")] {
            ink.text(queue, text, [x_at(self.now - t), axis], u * 0.85, Weight::Regular, GRAY, 1.0);
        }
        let now = ink.width(queue, "now", u * 0.85, Weight::Regular);
        ink.text(queue, "now", [x1 - now, axis], u * 0.85, Weight::Regular, GRAY, 1.0);
    }

    /// The spectrum, the drums and levels now, and the director's record.
    fn panel(&self, ink: &mut Ink, queue: &wgpu::Queue, xs: [f32; 2], ys: [f32; 2], u: f32, s: &Signals) {
        let [p0, p1] = xs;
        let [top, bottom] = ys;
        let width = p1 - p0;

        // The spectrum, each range in the color of the drum it holds.
        ink.text(queue, "spectrum", [p0, top + u * 0.95], u, Weight::Regular, GRAY, 1.0);
        let (st, sb) = (top + u * 1.8, top + (bottom - top) * 0.3);
        let slot = width / SPECTRUM_BANDS as f32;
        for (i, &v) in s.spectrum.iter().enumerate() {
            let color = if i < 11 { KICK } else if i < 25 { SNARE } else { CYMBALS };
            let height = v.clamp(0.0, 1.0) * (sb - st);
            ink.panel([p0 + i as f32 * slot, st, slot * 0.7, sb - st], slot * 0.2, RULE, 0.6);
            ink.panel([p0 + i as f32 * slot, sb - height, slot * 0.7, height], slot * 0.2, color, 0.95);
        }

        // The drums as pads that light up on each hit.
        let pads = sb + u * 2.0;
        let radius = (width / 3.0 * 0.22).min(u * 1.6);
        for (n, (name, v, color)) in [("kick", s.kick, KICK), ("snare", s.snare, SNARE), ("cymbals", s.cymbals, CYMBALS)].into_iter().enumerate() {
            let cx = p0 + width / 6.0 * (2 * n + 1) as f32;
            let cy = pads + radius;
            ink.panel([cx - radius, cy - radius, 2.0 * radius, 2.0 * radius], radius, RULE, 1.0);
            ink.panel([cx - radius, cy - radius, 2.0 * radius, 2.0 * radius], radius, color, 0.15 + 0.85 * v.clamp(0.0, 1.0));
            let text = ink.width(queue, name, u * 0.9, Weight::Regular);
            ink.text(queue, name, [cx - text * 0.5, cy + radius + u * 1.3], u * 0.9, Weight::Regular, GRAY, 1.0);
        }

        // Impact, and the balance of percussive against harmonic.
        let mut y = pads + 2.0 * radius + u * 3.6;
        let bar = |ink: &mut Ink, y: f32, name: &str, v: f32, color: [f32; 3]| {
            ink.text(queue, name, [p0, y], u, Weight::Regular, GRAY, 1.0);
            let value = format!("{v:.2}");
            let vw = ink.width(queue, &value, u, Weight::Strong);
            ink.text(queue, &value, [p1 - vw, y], u, Weight::Strong, INK, 1.0);
            ink.panel([p0, y + u * 0.55, width, u * 0.35], u * 0.17, RULE, 1.0);
            ink.panel([p0, y + u * 0.55, width * v.clamp(0.0, 1.0), u * 0.35], u * 0.17, color, 1.0);
        };
        bar(ink, y, "impact", s.impact, IMPACT);
        y += u * 2.4;
        bar(ink, y, "hits", s.hit, KICK);
        y += u * 2.4;
        bar(ink, y, "novelty", s.novelty, HARMONIC);

        // Why the scene changed.
        y += u * 3.0;
        ink.text(queue, "the director", [p0, y], u, Weight::Regular, GRAY, 1.0);
        y += u * 1.7;
        if self.changes.is_empty() {
            ink.text(queue, "no change yet", [p0, y], u, Weight::Regular, GRAY, 0.7);
        }
        for (n, (when, why, scene)) in self.changes.iter().rev().enumerate() {
            if y > bottom + u {
                break;
            }
            let alpha = if n == 0 { 1.0 } else { 0.55 };
            let mut x = ink.text(queue, &format!("{}  ", clock(*when)), [p0, y], u, Weight::Regular, GRAY, alpha);
            x = ink.text(queue, why, [x, y], u, Weight::Strong, if why == "impact" { IMPACT } else { INK }, alpha);
            ink.text(queue, &format!("  \u{2192}  {}", title_case(scene)), [x, y], u, Weight::Regular, INK, alpha);
            y += u * 1.6;
        }
    }
}

fn title_case(name: &str) -> String {
    let mut chars = name.chars();
    chars.next().map(|first| first.to_uppercase().chain(chars).collect()).unwrap_or_default()
}

/// A duration as seconds, or minutes and seconds.
fn seconds(t: f32) -> String {
    let t = t.max(0.0) as u32;
    if t < 60 { format!("{t} s") } else { format!("{}:{:02}", t / 60, t % 60) }
}

/// A moment as minutes and seconds since Juglar started.
fn clock(t: f32) -> String {
    let t = t.max(0.0) as u32;
    format!("{}:{:02}", t / 60, t % 60)
}
