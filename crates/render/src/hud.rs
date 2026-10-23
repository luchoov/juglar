//! What Juglar says on screen: a short note when a key changes something
//! ("Mode: focus"), which fades on its own, and the help with the keys.
//! Quiet on purpose, like the icon (D55): off-white and grays on near-black.

use crate::ink::{Ink, Weight};

/// Off-white, for what matters; gray, for the rest; near-black panels.
pub(crate) const INK: [f32; 3] = [0.93, 0.93, 0.91];
pub(crate) const GRAY: [f32; 3] = [0.6, 0.6, 0.6];
pub(crate) const PANEL: [f32; 3] = [0.05, 0.05, 0.06];
/// Hairlines: baselines, rules, the empty part of a bar.
pub(crate) const RULE: [f32; 3] = rgb(0x222227);
/// Words that matter less than the gray ones.
pub(crate) const DIM: [f32; 3] = rgb(0x5c5c63);
/// The one color, for what matters most: impacts and chaos in the
/// analysis view, the chosen setting, a song's line.
pub(crate) const ACCENT: [f32; 3] = rgb(0xff5b3a);

pub(crate) const fn rgb(hex: u32) -> [f32; 3] {
    [((hex >> 16) & 255) as f32 / 255.0, ((hex >> 8) & 255) as f32 / 255.0, (hex & 255) as f32 / 255.0]
}

/// Seconds a note takes to appear, stays, and takes to fade.
const NOTE_IN: f32 = 0.12;
const NOTE_HOLD: f32 = 1.6;
const NOTE_OUT: f32 = 0.6;
/// Seconds the help takes to appear or go.
const HELP_FADE: f32 = 0.15;

#[derive(Default)]
pub(crate) struct Hud {
    /// The note showing, and how long it has shown.
    note: Option<(String, String, f32)>,
    help: Vec<(String, String)>,
    help_on: bool,
    help_amount: f32,
}

impl Hud {
    /// Shows `label: value` for a moment. A note replaces the one showing
    /// without fading in again.
    pub(crate) fn note(&mut self, label: &str, value: &str) {
        let age = match &self.note {
            Some((_, _, age)) if *age < NOTE_IN + NOTE_HOLD + NOTE_OUT => NOTE_IN,
            _ => 0.0,
        };
        self.note = Some((label.to_owned(), value.to_owned(), age));
    }

    /// Shows the keys and what they do, or (with an empty list) hides them.
    pub(crate) fn help(&mut self, lines: &[(&str, &str)]) {
        self.help_on = !lines.is_empty();
        if self.help_on {
            self.help = lines.iter().map(|&(key, does)| (key.to_owned(), does.to_owned())).collect();
        }
    }

    pub(crate) fn helping(&self) -> bool {
        self.help_on
    }

    /// Queues this frame's text into `ink`, `dt` seconds after the last.
    pub(crate) fn draw(&mut self, ink: &mut Ink, queue: &wgpu::Queue, size: [f32; 2], dt: f32) {
        if !ink.has_fonts() {
            return;
        }
        let fade = dt / HELP_FADE;
        self.help_amount = (self.help_amount + if self.help_on { fade } else { -fade }).clamp(0.0, 1.0);
        // Sized by the window's height, within what stays readable.
        let unit = (size[1] * 0.024).clamp(14.0, 34.0);
        if self.help_amount > 0.0 {
            self.draw_help(ink, queue, size, unit, smooth(self.help_amount));
        }
        if let Some((label, value, age)) = &mut self.note {
            *age += dt;
            let t = *age;
            let alpha = if t < NOTE_IN {
                t / NOTE_IN
            } else if t < NOTE_IN + NOTE_HOLD {
                1.0
            } else {
                1.0 - (t - NOTE_IN - NOTE_HOLD) / NOTE_OUT
            };
            if alpha <= 0.0 {
                self.note = None;
                return;
            }
            let alpha = smooth(alpha);
            let (label, value) = (format!("{label}  "), value.clone());
            let width = ink.width(queue, &label, unit, Weight::Regular) + ink.width(queue, &value, unit, Weight::Strong);
            let (ascent, descent) = ink.metrics(unit);
            let (pad_x, pad_y) = (unit * 0.9, unit * 0.55);
            let height = ascent + descent + 2.0 * pad_y;
            let left = (size[0] - width) * 0.5 - pad_x;
            let top = size[1] - height - unit * 2.0;
            ink.panel([left, top, width + 2.0 * pad_x, height], height * 0.5, PANEL, 0.88 * alpha);
            let pen = [left + pad_x, top + pad_y + ascent];
            let x = ink.text(queue, &label, pen, unit, Weight::Regular, GRAY, alpha);
            ink.text(queue, &value, [x, pen[1]], unit, Weight::Strong, INK, alpha);
        }
    }

    /// The keys in two columns on a panel in the middle of the screen.
    fn draw_help(&self, ink: &mut Ink, queue: &wgpu::Queue, size: [f32; 2], unit: f32, alpha: f32) {
        let text = unit * 0.85;
        let row = text * 1.75;
        let gap = text * 1.6;
        let keys = self.help.iter().map(|(key, _)| ink.width(queue, key, text, Weight::Strong)).fold(0.0, f32::max);
        let does = self.help.iter().map(|(_, does)| ink.width(queue, does, text, Weight::Regular)).fold(0.0, f32::max);
        let title = unit * 1.3;
        let pad = unit * 1.4;
        let width = keys + gap + does + 2.0 * pad;
        let height = title * 1.8 + row * self.help.len() as f32 + 2.0 * pad - (row - text);
        let left = ((size[0] - width) * 0.5).max(0.0);
        let top = ((size[1] - height) * 0.5).max(0.0);
        ink.panel([left, top, width, height], unit * 0.7, PANEL, 0.95 * alpha);
        let (ascent, _) = ink.metrics(title);
        ink.text(queue, "juglar", [left + pad, top + pad + ascent], title, Weight::Strong, INK, alpha);
        let mut y = top + pad + title * 1.8 + ink.metrics(text).0;
        for (key, does) in &self.help {
            let key_width = ink.width(queue, key, text, Weight::Strong);
            ink.text(queue, key, [left + pad + keys - key_width, y], text, Weight::Strong, INK, alpha);
            ink.text(queue, does, [left + pad + keys + gap, y], text, Weight::Regular, GRAY, alpha);
            y += row;
        }
    }
}

fn smooth(t: f32) -> f32 {
    t * t * (3.0 - 2.0 * t)
}
