//! The analysis view (`Tab`): what Juglar hears, labeled (D59).
//!
//! Drawn entirely with `ink`, like a measuring instrument: thin lines in
//! off-white and grays on near-black, and one color, kept for what breaks
//! the form (chaos and impacts). The wave as heard; the last eight seconds
//! of the heavy ear (loudness, grit, density, chaos), the drums as ticks,
//! the percussive part against the harmonic; the impacts and the
//! director's changes across all of them, named; each track's value now
//! at its right end. On the right, the spectrum, the drums and events now,
//! and why the scene last changed.

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

const BACKGROUND: [f32; 3] = rgb(0x0b0b0d);
/// Hairlines: baselines, the seconds, the empty part of a bar.
const RULE: [f32; 3] = rgb(0x222227);
/// Labels that matter less than the gray ones.
const DIM: [f32; 3] = rgb(0x5c5c63);
/// The one color: chaos and impacts.
const ACCENT: [f32; 3] = rgb(0xff5b3a);
/// The spectrum's range (`SPECTRUM_HZ` in the analyzer), for its scale.
const SPECTRUM_HZ: (f32, f32) = (30.0, 16_000.0);

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
    /// The director changed scene, to this one.
    Change(&'static str),
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
        self.marks.push_back((self.now, Mark::Change(scene)));
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
        let u = (h * 0.0175).clamp(10.0, 26.0);
        let m = (h * 0.055).round();
        let line = (h / 800.0).max(1.0);
        ink.panel([0.0, 0.0, w, h], 0.0, BACKGROUND, 1.0);

        // The title and what is showing; the tempo, its dot lit on beats.
        let title = m + ink.metrics(u * 1.1).0;
        let x = ink.text(queue, "what juglar hears", [m, title], u * 1.1, Weight::Strong, INK, 1.0);
        let shown = s.time - self.scene.1;
        let mut x = ink.text(queue, "    ", [x, title], u * 0.9, Weight::Regular, GRAY, 1.0);
        x = ink.text(queue, &title_case(scene), [x, title], u * 0.9, Weight::Strong, GRAY, 1.0);
        let detail = format!("  for {}  \u{b7}  intensity {:.2}", seconds(shown), s.intensity);
        ink.text(queue, &detail, [x, title], u * 0.9, Weight::Regular, DIM, 1.0);
        let right = w - m;
        let (tempo, unit) = if s.tempo > 0.0 { (format!("{:.0}", s.tempo), " bpm") } else { (String::new(), "no steady beat") };
        let unit_width = ink.width(queue, unit, u * 0.9, Weight::Regular);
        let tempo_width = ink.width(queue, &tempo, u * 1.1, Weight::Strong);
        ink.text(queue, unit, [right - unit_width, title], u * 0.9, Weight::Regular, DIM, 1.0);
        ink.text(queue, &tempo, [right - unit_width - tempo_width, title], u * 1.1, Weight::Strong, INK, 1.0);
        let dot = u * 0.3;
        let at = [right - unit_width - tempo_width - u * 0.9, title - u * 0.38];
        ink.panel([at[0] - dot, at[1] - dot, 2.0 * dot, 2.0 * dot], dot, RULE, 1.0);
        ink.panel([at[0] - dot, at[1] - dot, 2.0 * dot, 2.0 * dot], dot, INK, s.beat.clamp(0.0, 1.0));

        // The wave, across the whole width, quiet.
        let wave_top = title + u * 1.8;
        let wave_height = h * 0.075;
        let wave_mid = wave_top + wave_height * 0.5;
        let step = (w - 2.0 * m) / (WAVEFORM_POINTS - 1) as f32;
        let point = |i: usize| [m + i as f32 * step, wave_mid - s.waveform[i].clamp(-1.0, 1.0) * wave_height * 0.5];
        for i in 1..WAVEFORM_POINTS {
            ink.line(point(i - 1), point(i), line, INK, 0.45);
        }

        // The timeline on the left, its values just right of it, the
        // panel on the right.
        let top = wave_top + wave_height + u * 3.4;
        let bottom = h - m - u * 1.8;
        let (x0, x1) = (m + u * 6.5, (w * 0.64).round());
        let (p0, p1) = (x1 + u * 6.0, w - m);
        self.timeline(ink, queue, [m, x0, x1], [top, bottom], u, line, s);
        self.panel(ink, queue, [p0, p1], [top, bottom], u, line, s);
    }

    /// The tracks over the last `WINDOW` seconds, labeled from `xs[0]`,
    /// drawn from `xs[1]` to `xs[2]`, their values now past `xs[2]`.
    fn timeline(&self, ink: &mut Ink, queue: &wgpu::Queue, xs: [f32; 3], ys: [f32; 2], u: f32, line: f32, s: &Signals) {
        let [m, x0, x1] = xs;
        let [top, bottom] = ys;
        let x_at = |t: f32| x1 - (self.now - t) / WINDOW * (x1 - x0);
        let since = self.now - WINDOW;
        // Filled columns on whole pixels, so neighbors neither overlap nor
        // leave seams; `None` where a column would fall within the last.
        let columns = |a: f32, b: f32| {
            let (xa, xb) = (x_at(a).round(), x_at(b).round());
            (xb > xa).then_some((xa, xb - xa))
        };
        // Track heights in shares, and the gap between tracks.
        let shares = [0.9, 0.7, 0.7, 1.0, 1.2, 1.2];
        let gap = u * 1.2;
        let unit = (bottom - top - gap * (shares.len() - 1) as f32) / shares.iter().sum::<f32>();
        let mut bands = Vec::new();
        let mut y = top;
        for share in shares {
            bands.push((y, y + share * unit));
            y += share * unit + gap;
        }

        // A hairline every second, moving with the music.
        let mut second = self.now.floor();
        while second > since {
            let x = x_at(second).round();
            ink.line([x, top], [x, bottom], line, RULE, 0.6);
            second -= 1.0;
        }

        let small = u * 0.85;
        let ascent = ink.metrics(small).0;
        let label = |ink: &mut Ink, y: f32, name: &str| {
            ink.text(queue, name, [m, y + ascent], small, Weight::Regular, GRAY, 1.0);
        };
        let value = |ink: &mut Ink, y: f32, v: f32, color: [f32; 3]| {
            ink.text(queue, &format!("{v:.2}"), [x1 + u * 0.9, y + ascent], small, Weight::Strong, color, 1.0);
        };
        let samples: Vec<&Sample> = self.samples.iter().filter(|x| x.time >= since).collect();
        let trace = |ink: &mut Ink, band: (f32, f32), get: fn(&Sample) -> f32, color: [f32; 3], alpha: f32, fill: f32| {
            let (t, b) = band;
            ink.line([x0, b], [x1, b], line, RULE, 1.0);
            for pair in samples.windows(2) {
                let ya = b - get(pair[0]).clamp(0.0, 1.0) * (b - t);
                let yb = b - get(pair[1]).clamp(0.0, 1.0) * (b - t);
                if fill > 0.0
                    && let Some((x, width)) = columns(pair[0].time, pair[1].time)
                {
                    ink.panel([x, ya, width, b - ya], 0.0, color, fill);
                }
                ink.line([x_at(pair[0].time), ya], [x_at(pair[1].time), yb], line * 1.4, color, alpha);
            }
        };

        trace(ink, bands[0], |x| x.loudness, INK, 0.8, 0.02);
        label(ink, bands[0].0, "loudness");
        value(ink, bands[0].0, s.loudness, INK);
        trace(ink, bands[1], |x| x.grit, INK, 0.8, 0.0);
        label(ink, bands[1].0, "grit");
        value(ink, bands[1].0, s.grit, INK);
        trace(ink, bands[2], |x| x.density, INK, 0.5, 0.0);
        label(ink, bands[2].0, "density");
        value(ink, bands[2].0, s.density, INK);
        trace(ink, bands[3], |x| x.chaos, ACCENT, 1.0, 0.05);
        label(ink, bands[3].0, "chaos");
        value(ink, bands[3].0, s.chaos, ACCENT);

        // The drums, a row each, a tick per hit as tall as the hit.
        let (dt, db) = bands[4];
        let row = (db - dt) / 3.0;
        for (n, name) in ["kick", "snare", "cymbals"].into_iter().enumerate() {
            let rt = dt + row * n as f32;
            ink.line([x0, rt + row * 0.85], [x1, rt + row * 0.85], line, RULE, 1.0);
            ink.text(queue, name, [m, rt + row * 0.85], small, Weight::Regular, GRAY, 1.0);
        }
        for &(t, mark) in &self.marks {
            let (n, v) = match mark {
                Mark::Kick(v) => (0, v),
                Mark::Snare(v) => (1, v),
                Mark::Cymbals(v) => (2, v),
                _ => continue,
            };
            if t < since {
                continue;
            }
            let base = dt + row * n as f32 + row * 0.85;
            let v = v.clamp(0.0, 1.0);
            ink.line([x_at(t), base], [x_at(t), base - (0.25 + 0.5 * v) * row], line * 2.0, INK, 0.4 + 0.6 * v);
        }

        // Percussive rising from the middle, harmonic hanging below it.
        let (pt, pb) = bands[5];
        let mid = (pt + pb) * 0.5;
        ink.line([x0, mid], [x1, mid], line, RULE, 1.0);
        for pair in samples.windows(2) {
            let (xa, xb) = (x_at(pair[0].time), x_at(pair[1].time));
            let up = |x: &Sample| mid - x.percussive.clamp(0.0, 1.0) * (mid - pt);
            let down = |x: &Sample| mid + x.harmonic.clamp(0.0, 1.0) * (pb - mid);
            ink.line([xa, up(pair[0])], [xb, up(pair[1])], line * 1.2, INK, 0.7);
            ink.line([xa, down(pair[0])], [xb, down(pair[1])], line * 1.4, INK, 0.4);
        }
        label(ink, pt, "percussive");
        value(ink, pt, s.percussive, INK);
        ink.text(queue, "harmonic", [m, pb], small, Weight::Regular, GRAY, 1.0);
        ink.text(queue, &format!("{:.2}", s.harmonic), [x1 + u * 0.9, pb], small, Weight::Strong, GRAY, 1.0);

        // Impacts and the director's changes across every track, named
        // above it; the beats as ticks under it.
        let tiny = u * 0.75;
        for &(t, mark) in &self.marks {
            if t < since {
                continue;
            }
            let x = x_at(t).round();
            match mark {
                Mark::Impact => {
                    ink.line([x, top - u * 1.0], [x, bottom], line * 1.5, ACCENT, 0.9);
                    ink.text(queue, "impact", [x + u * 0.35, top - u * 0.35], tiny, Weight::Strong, ACCENT, 1.0);
                }
                Mark::Change(scene) => {
                    let mut y = top - u * 2.2;
                    while y < bottom {
                        ink.line([x, y], [x, (y + u * 0.35).min(bottom)], line, INK, 0.6);
                        y += u * 0.7;
                    }
                    ink.text(queue, &title_case(scene), [x + u * 0.35, top - u * 1.55], tiny, Weight::Strong, INK, 1.0);
                }
                Mark::Beat => ink.line([x, bottom + u * 0.35], [x, bottom + u * 0.75], line, INK, 0.45),
                _ => {}
            }
        }

        // The time axis.
        let axis = bottom + u * 1.9;
        for (t, text) in [(WINDOW, "\u{2212}8 s"), (WINDOW * 0.5, "\u{2212}4 s")] {
            ink.text(queue, text, [x_at(self.now - t), axis], tiny, Weight::Regular, DIM, 1.0);
        }
        let now = ink.width(queue, "now", tiny, Weight::Regular);
        ink.text(queue, "now", [x1 - now, axis], tiny, Weight::Regular, DIM, 1.0);
    }

    /// The spectrum, the drums and events now, and the director's record.
    fn panel(&self, ink: &mut Ink, queue: &wgpu::Queue, xs: [f32; 2], ys: [f32; 2], u: f32, line: f32, s: &Signals) {
        let [p0, p1] = xs;
        let [top, bottom] = ys;
        let width = p1 - p0;
        let small = u * 0.85;
        let tiny = u * 0.75;
        // A section: its name over a hairline; returns where its body starts.
        let section = |ink: &mut Ink, y: f32, name: &str| {
            ink.text(queue, name, [p0, y + ink.metrics(small).0], small, Weight::Regular, GRAY, 1.0);
            ink.line([p0, y + u * 1.45], [p1, y + u * 1.45], line, RULE, 1.0);
            y + u * 2.3
        };

        // The spectrum as thin bars on a log scale, its ranges marked.
        let st = section(ink, top, "spectrum");
        let sb = top + (bottom - top) * 0.3;
        let slot = width / SPECTRUM_BANDS as f32;
        ink.line([p0, sb], [p1, sb], line, RULE, 1.0);
        for (i, &v) in s.spectrum.iter().enumerate() {
            let x = (p0 + (i as f32 + 0.5) * slot).round();
            let height = v.clamp(0.0, 1.0) * (sb - st);
            ink.line([x, sb], [x, sb - height.max(line)], (slot * 0.22).max(line * 1.5), INK, 0.65);
        }
        let place = |hz: f32| p0 + width * (hz / SPECTRUM_HZ.0).ln() / (SPECTRUM_HZ.1 / SPECTRUM_HZ.0).ln();
        for (hz, text) in [(SPECTRUM_HZ.0, "30"), (250.0, "250"), (4_000.0, "4k"), (SPECTRUM_HZ.1, "16k Hz")] {
            let x = place(hz);
            ink.line([x, sb], [x, sb + u * 0.4], line, GRAY, 0.6);
            let tw = ink.width(queue, text, tiny, Weight::Regular);
            let left = (x - tw * 0.5).clamp(p0, p1 - tw);
            ink.text(queue, text, [left, sb + u * 1.4], tiny, Weight::Regular, DIM, 1.0);
        }

        // The drums now: each name lights up on its hit, over a bar.
        let mut y = section(ink, sb + u * 2.6, "drums");
        let column = width / 3.0;
        for (n, (name, v)) in [("kick", s.kick), ("snare", s.snare), ("cymbals", s.cymbals)].into_iter().enumerate() {
            let v = v.clamp(0.0, 1.0);
            let x = p0 + column * n as f32;
            let color = [GRAY[0] + (INK[0] - GRAY[0]) * v, GRAY[1] + (INK[1] - GRAY[1]) * v, GRAY[2] + (INK[2] - GRAY[2]) * v];
            ink.text(queue, name, [x, y + ink.metrics(small).0], small, if v > 0.5 { Weight::Strong } else { Weight::Regular }, color, 1.0);
            let bar = [x, y + u * 1.45, column - u * 1.2, line * 2.0];
            ink.panel(bar, 0.0, RULE, 1.0);
            ink.panel([bar[0], bar[1], bar[2] * v, bar[3]], 0.0, INK, 1.0);
        }

        // Events now: a name, a bar, the value.
        y = section(ink, y + u * 3.2, "events");
        for (name, v, color) in [("impact", s.impact, ACCENT), ("hits", s.hit, INK), ("novelty", s.novelty, INK)] {
            let v = v.clamp(0.0, 1.0);
            let base = y + ink.metrics(small).0;
            ink.text(queue, name, [p0, base], small, Weight::Regular, GRAY, 1.0);
            let text = format!("{v:.2}");
            let tw = ink.width(queue, &text, small, Weight::Strong);
            ink.text(queue, &text, [p1 - tw, base], small, Weight::Strong, if v > 0.0 { color } else { DIM }, 1.0);
            let bar = [p0 + u * 5.0, base - u * 0.32, width - u * 8.5, line * 2.0];
            ink.panel(bar, 0.0, RULE, 1.0);
            ink.panel([bar[0], bar[1], bar[2] * v, bar[3]], 0.0, color, 1.0);
            y += u * 1.7;
        }

        // Why the scene changed: when, at which moment of the music, to
        // which scene.
        y = section(ink, y + u * 1.6, "the director");
        if self.changes.is_empty() {
            ink.text(queue, "no change yet", [p0, y + ink.metrics(small).0], small, Weight::Regular, DIM, 1.0);
        }
        let when_width = ink.width(queue, "00:00  ", small, Weight::Regular);
        for (n, (when, why, scene)) in self.changes.iter().rev().enumerate() {
            let base = y + ink.metrics(small).0;
            if base > bottom + u {
                break;
            }
            let alpha = if n == 0 { 1.0 } else { 0.5 };
            ink.text(queue, &clock(*when), [p0, base], small, Weight::Regular, DIM, alpha);
            let x = ink.text(queue, why, [p0 + when_width, base], small, Weight::Strong, if why == "impact" { ACCENT } else { INK }, alpha);
            ink.text(queue, &format!("  \u{2192}  {}", title_case(scene)), [x, base], small, Weight::Regular, GRAY, alpha);
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
