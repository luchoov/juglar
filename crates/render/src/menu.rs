//! The menu Juglar opens with, and its settings later: a panel over the
//! scene, which keeps playing dimmed behind it. A row for each setting, its
//! choices in a line, the chosen one marked by the accent; what is playing
//! at the top; one button. The app decides what the rows are and what a
//! click does: this draws them, and says what lies under a point (`hit`).

use crate::hud::{ACCENT, DIM, GRAY, INK, PANEL, RULE};
use crate::ink::{Ink, Weight};

/// Seconds the menu takes to appear or go.
const FADE: f32 = 0.18;

/// What the menu shows.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Menu {
    pub rows: Vec<Row>,
    /// The row the keyboard is on; `rows.len()` is the button.
    pub focus: usize,
    /// The button's word: "Start", "Done".
    pub button: String,
    /// The keys, said beside the button.
    pub keys: String,
    /// What is playing: the title, and the artist and where it plays
    /// from; drawn with the cover when there is one.
    pub song: Option<(String, String)>,
    /// Beside the name at the top: the version.
    pub version: String,
    /// What the pointer is over.
    pub hover: Option<Hit>,
}

/// A setting: its name, its choices, the one chosen, and a remark.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Row {
    pub label: String,
    pub options: Vec<String>,
    pub chosen: usize,
    /// Said quietly after the choices: "waiting for Spotify".
    pub note: String,
}

/// What lies under a point of the menu.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hit {
    /// A row's choice: the row, the choice.
    Choice(usize, usize),
    Button,
}

#[derive(Default)]
pub(crate) struct MenuView {
    menu: Option<Menu>,
    /// Showing, or going.
    on: bool,
    amount: f32,
    /// Where each thing that can be clicked was drawn last.
    hits: Vec<([f32; 4], Hit)>,
}

impl MenuView {
    pub(crate) fn set(&mut self, menu: Option<Menu>) {
        self.on = menu.is_some();
        if menu.is_some() {
            self.menu = menu;
        }
    }

    pub(crate) fn showing(&self) -> bool {
        self.on
    }

    /// What lies at `point`, in pixels from the top left.
    pub(crate) fn hit(&self, point: [f32; 2]) -> Option<Hit> {
        if !self.on {
            return None;
        }
        self.hits.iter().find(|(r, _)| point[0] >= r[0] && point[1] >= r[1] && point[0] < r[0] + r[2] && point[1] < r[1] + r[3]).map(|&(_, hit)| hit)
    }

    pub(crate) fn draw(&mut self, ink: &mut Ink, queue: &wgpu::Queue, size: [f32; 2], dt: f32) {
        let step = dt / FADE;
        self.amount = (self.amount + if self.on { step } else { -step }).clamp(0.0, 1.0);
        self.hits.clear();
        if self.amount <= 0.0 {
            self.menu = None;
            return;
        }
        let Some(menu) = &self.menu else { return };
        if !ink.has_fonts() {
            return;
        }
        let alpha = smooth(self.amount);
        let [w, h] = size;
        let u = (h * 0.024).min(w * 0.017).clamp(12.0, 21.0).round();
        let text = u;
        let small = (u * 0.82).round();
        let (ascent, descent) = ink.metrics(text);
        let line_height = u * 2.3;
        let pad = u * 1.9;
        let label_width = u * 9.0;
        let panel_width = (w - 2.0 * u).min(u * 46.0);
        let left = ((w - panel_width) * 0.5).round();
        let (x0, x1) = (left + pad + label_width, left + panel_width - pad);
        let gap = u * 1.4;

        // Each row's choices laid out in lines that fit, before drawing,
        // for the panel's height.
        let layouts: Vec<Vec<(f32, usize, f32)>> = menu
            .rows
            .iter()
            .map(|row| {
                let mut placed = Vec::new();
                let (mut x, mut line) = (x0, 0.0);
                for (n, option) in row.options.iter().enumerate() {
                    let weight = if n == row.chosen { Weight::Strong } else { Weight::Regular };
                    let width = ink.width(queue, option, text, weight);
                    if x > x0 && x + width > x1 {
                        (x, line) = (x0, line + 1.0);
                    }
                    placed.push((x, n, line));
                    x += width + gap;
                }
                placed
            })
            .collect();
        let row_lines: Vec<f32> = layouts.iter().map(|l| l.last().map_or(0.0, |&(_, _, line)| line) + 1.0).collect();
        let title_size = (u * 1.45).round();
        let song_height = if menu.song.is_some() { u * 3.0 } else { 0.0 };
        let song_block = song_height + u * 2.2;
        let rows_height: f32 = row_lines.iter().map(|lines| lines * line_height).sum();
        let button_height = (u * 2.5).round();
        let height = pad + title_size + song_block + u * 0.8 + rows_height + u * 1.4 + button_height + pad;
        let top = ((h - height) * 0.5).max(0.0).round();

        // The scene dims behind; the panel, and a soft shadow under it.
        ink.panel([0.0, 0.0, w, h], 0.0, [0.0, 0.0, 0.0], 0.45 * alpha);
        let shadow = u * 2.0;
        ink.soft_panel([left - shadow, top - shadow * 0.6, panel_width + 2.0 * shadow, height + 2.0 * shadow], shadow, shadow, [0.0; 3], 0.5 * alpha);
        ink.panel([left, top, panel_width, height], u * 0.8, PANEL, 0.94 * alpha);

        // The name, and the version at the right.
        let mut y = top + pad + ink.metrics(title_size).0;
        ink.text(queue, "juglar", [left + pad, y], title_size, Weight::Strong, INK, alpha);
        let version_width = ink.width(queue, &menu.version, small, Weight::Regular);
        ink.text(queue, &menu.version, [x1 - version_width, y], small, Weight::Regular, DIM, alpha);
        y += ink.metrics(title_size).1 + u * 1.1;

        // What is playing, with its cover.
        if let Some((title, detail)) = &menu.song {
            let side = song_height;
            let mut x = left + pad;
            if let Some(aspect) = ink.picture_aspect() {
                let (pw, ph) = if aspect >= 1.0 { (side, side / aspect) } else { (side * aspect, side) };
                ink.picture([x, y + (side - ph) * 0.5, pw, ph], u * 0.25, alpha);
                x += side + u * 1.0;
            }
            let room = x1 - x;
            let title = ink.clip(queue, title, text, Weight::Strong, room);
            let detail = ink.clip(queue, detail, small, Weight::Regular, room);
            let middle = y + side * 0.5;
            ink.text(queue, &title, [x, middle - u * 0.2], text, Weight::Strong, INK, alpha);
            ink.text(queue, &detail, [x, middle + ink.metrics(small).0 + u * 0.15], small, Weight::Regular, GRAY, alpha);
            y += side;
        } else {
            y -= u * 0.6;
        }
        y += u * 1.1;
        ink.line([left + pad, y], [x1, y], 1.0, RULE, alpha);
        y += u * 0.8;

        // The rows.
        for (r, (row, placed)) in menu.rows.iter().zip(&layouts).enumerate() {
            let focused = menu.focus == r;
            let base = y + (line_height - ascent - descent) * 0.5 + ascent;
            if focused {
                let bar = u * 1.3;
                ink.panel([left + pad * 0.45, base - ascent * 0.5 - bar * 0.5, 2.0, bar], 1.0, ACCENT, alpha);
            }
            ink.text(queue, &row.label, [left + pad, base], text, Weight::Regular, if focused { INK } else { GRAY }, alpha);
            let mut end = x0;
            for &(x, n, line) in placed {
                let option = &row.options[n];
                let chosen = n == row.chosen;
                let hovered = menu.hover == Some(Hit::Choice(r, n));
                let weight = if chosen { Weight::Strong } else { Weight::Regular };
                let color = if chosen || hovered { INK } else { GRAY };
                let at = base + line * line_height;
                let right = ink.text(queue, option, [x, at], text, weight, color, alpha);
                if chosen {
                    ink.line([x, at + descent + u * 0.15], [right, at + descent + u * 0.15], 2.0, ACCENT, alpha);
                } else if hovered {
                    ink.line([x, at + descent + u * 0.15], [right, at + descent + u * 0.15], 1.0, GRAY, alpha);
                }
                self.hits.push(([x - gap * 0.5, at - ascent - u * 0.5, right - x + gap, line_height], Hit::Choice(r, n)));
                end = right;
            }
            if !row.note.is_empty() {
                let last = placed.last().map_or(0.0, |&(_, _, line)| line);
                let x = end + gap;
                let note = ink.clip(queue, &row.note, small, Weight::Regular, x1 - x);
                ink.text(queue, &note, [x, base + last * line_height], small, Weight::Regular, DIM, alpha);
            }
            y += row_lines[r] * line_height;
        }

        // The button at the right, the keys at the left.
        y += u * 1.4;
        let label_width = ink.width(queue, &menu.button, text, Weight::Strong);
        let button_width = (label_width + u * 3.4).round();
        let button = [x1 - button_width, y, button_width, button_height];
        let lit = menu.focus == menu.rows.len() || menu.hover == Some(Hit::Button);
        if lit {
            let ring = 2.0;
            ink.panel([button[0] - ring * 2.0, button[1] - ring * 2.0, button[2] + ring * 4.0, button[3] + ring * 4.0], button_height, INK, 0.9 * alpha);
            ink.panel([button[0] - ring, button[1] - ring, button[2] + ring * 2.0, button[3] + ring * 2.0], button_height, PANEL, alpha);
        }
        ink.panel(button, button_height * 0.5, ACCENT, alpha);
        let base = y + (button_height - ascent - descent) * 0.5 + ascent;
        ink.text(queue, &menu.button, [button[0] + (button_width - label_width) * 0.5, base], text, Weight::Strong, PANEL, alpha);
        self.hits.push((button, Hit::Button));
        let keys = ink.clip(queue, &menu.keys, small, Weight::Regular, button[0] - left - pad - u);
        ink.text(queue, &keys, [left + pad, base], small, Weight::Regular, DIM, alpha);
    }
}

fn smooth(t: f32) -> f32 {
    t * t * (3.0 - 2.0 * t)
}
