//! The menu Juglar opens with (and `S` or the tray's "Settings…" opens
//! later): what to listen to, how to show Juglar, the mode, the intensity,
//! song titles, and whether to open this menu next time. Choices that
//! change the picture apply at once, so the scene behind shows them; how
//! Juglar is shown applies when the menu closes ("Start", "Done", `Esc`).
//! The renderer draws it (`juglar_render::Menu`); this decides what is in
//! it and what a key or a click does.

use juglar_director::Intensity;
use juglar_platform::Listen;
use juglar_render::{Hit, Menu, Row};
use winit::keyboard::{Key, NamedKey};

use crate::desktop::SPREADS;
use crate::settings::{SHOWS, Show};
use crate::{App, INTENSITIES, INTENSITY_PRESETS, MODES, title};

/// A row of the menu.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Setting {
    Listen,
    Show,
    Displays,
    Mode,
    Intensity,
    Titles,
    AtStart,
}

/// The menu's state between frames.
#[derive(Default)]
pub(crate) struct Launcher {
    pub(crate) open: bool,
    /// The menu Juglar opened with: its button starts the show.
    pub(crate) first: bool,
    /// The row the keyboard is on (one past the rows: the button).
    focus: usize,
    hover: Option<Hit>,
    /// Where the pointer was last seen, in window pixels.
    pointer: [f32; 2],
}

impl App {
    /// What can be listened to, as the menus name it: everything, then each
    /// program making sound, and the one chosen even when it is closed.
    pub(crate) fn sources(&self) -> Vec<(Listen, String)> {
        let mut sources = vec![(Listen::Everything, "Everything".to_owned())];
        for app in &self.apps {
            sources.push((Listen::App(app.exe.clone()), app.name.clone()));
        }
        if let Listen::App(exe) = &self.settings.listen
            && !self.apps.iter().any(|a| a.exe.eq_ignore_ascii_case(exe))
        {
            let name = exe.strip_suffix(".exe").or_else(|| exe.strip_suffix(".EXE")).unwrap_or(exe);
            sources.push((self.settings.listen.clone(), title(name)));
        }
        sources
    }

    /// Where the chosen source is in `sources`.
    pub(crate) fn source_index(&self, sources: &[(Listen, String)]) -> usize {
        sources.iter().position(|(listen, _)| match (listen, &self.settings.listen) {
            (Listen::App(a), Listen::App(b)) => a.eq_ignore_ascii_case(b),
            (a, b) => a == b,
        })
        .unwrap_or(0)
    }

    fn rows(&self) -> Vec<(Setting, Row)> {
        let row = |label: &str, options: Vec<String>, chosen: usize| Row {
            label: label.to_owned(),
            options,
            chosen,
            note: String::new(),
        };
        let sources = self.sources();
        let mut listen = row("Listen to", sources.iter().map(|(_, name)| name.clone()).collect(), self.source_index(&sources));
        listen.note = self.listen_note.clone();
        let mut rows = vec![
            (Setting::Listen, listen),
            (Setting::Show, row("Show as", SHOWS.iter().map(|&(_, n)| n.to_owned()).collect(), show_index(self.settings.show))),
        ];
        let displays = self.window.as_ref().map_or(1, |w| w.available_monitors().count());
        if self.settings.show == Show::Wallpaper && displays > 1 {
            let chosen = SPREADS.iter().position(|&(s, _)| s == self.settings.spread).unwrap_or(0);
            rows.push((Setting::Displays, row("Displays", SPREADS.iter().map(|&(_, n)| n.to_owned()).collect(), chosen)));
        }
        let mode = MODES.iter().position(|&m| m == self.director.mode()).unwrap_or(0);
        rows.push((Setting::Mode, row("Mode", MODES.iter().map(|m| title(m.name())).collect(), mode)));
        rows.push((Setting::Intensity, row("Intensity", INTENSITIES.iter().map(|i| title(i)).collect(), self.settings.intensity)));
        let on_off = |on: bool| usize::from(!on);
        rows.push((Setting::Titles, row("Song titles", vec!["Show".into(), "Hide".into()], on_off(self.settings.intros))));
        rows.push((Setting::AtStart, row("At start", vec!["Open this menu".into(), "Start right away".into()], on_off(self.settings.menu))));
        rows
    }

    /// Opens the menu: `first` when Juglar starts, else as its settings.
    pub(crate) fn open_menu(&mut self, first: bool) {
        // The settings start from how Juglar is shown now; from the
        // wallpaper, back to a window, where the menu can be used.
        if !first {
            self.settings.show = self.showing();
            if self.wallpaper.is_some() {
                self.leave_wallpaper();
            }
        }
        if let Some(window) = &self.window {
            window.set_minimized(false);
            window.focus_window();
        }
        self.launcher = Launcher { open: true, first, focus: if first { usize::MAX } else { 0 }, ..Launcher::default() };
        self.refresh_menu();
    }

    /// Closes the menu and shows Juglar as chosen; the first time, the
    /// song playing gets its title.
    pub(crate) fn close_menu(&mut self) {
        let first = self.launcher.first;
        self.launcher = Launcher::default();
        if let Some(renderer) = &mut self.renderer {
            renderer.set_menu(None);
        }
        if let Some(window) = &self.window {
            window.set_cursor(winit::window::CursorIcon::Default);
        }
        self.apply_show();
        if first {
            self.introduced = None;
            self.pending_intro = self.track.as_ref().filter(|t| t.playing).map(|_| std::time::Instant::now());
        }
    }

    /// Draws the menu again from the current state, if it is open.
    pub(crate) fn refresh_menu(&mut self) {
        if !self.launcher.open {
            return;
        }
        let rows: Vec<Row> = self.rows().into_iter().map(|(_, row)| row).collect();
        // The first menu starts on its button: Enter starts at once.
        let focus = if self.launcher.focus == usize::MAX { rows.len() } else { self.launcher.focus.min(rows.len()) };
        self.launcher.focus = focus;
        let song = self.song.as_ref().map(|song| {
            let detail = match (song.artist.is_empty(), song.source.is_empty()) {
                (false, false) => format!("{}  \u{b7}  {}", song.artist, song.source),
                (false, true) => song.artist.clone(),
                (true, _) => song.source.clone(),
            };
            (song.title.clone(), detail)
        });
        let first = self.launcher.first;
        let menu = Menu {
            rows,
            focus,
            button: if first { "Start".into() } else { "Done".into() },
            keys: "\u{2191}\u{2193} move   \u{2190}\u{2192} choose   Enter to go".into(),
            song: song.or_else(|| Some(("Nothing playing".into(), "Play something and Juglar will hear it".into()))),
            version: format!("v{}", env!("CARGO_PKG_VERSION")),
            hover: self.launcher.hover,
        };
        if let Some(renderer) = &mut self.renderer {
            renderer.set_menu(Some(menu));
        }
    }

    /// A key while the menu is open: the arrows move and choose, Enter
    /// and `Esc` close it. Every other key is ignored.
    pub(crate) fn menu_key(&mut self, key: &Key) {
        let rows = self.rows();
        let focus = self.launcher.focus;
        match key {
            Key::Named(NamedKey::ArrowUp) => self.launcher.focus = focus.saturating_sub(1),
            Key::Named(NamedKey::ArrowDown) | Key::Named(NamedKey::Tab) => self.launcher.focus = (focus + 1).min(rows.len()),
            Key::Named(step @ (NamedKey::ArrowLeft | NamedKey::ArrowRight)) => {
                if let Some((setting, row)) = rows.get(focus) {
                    let count = row.options.len().max(1);
                    let next = if *step == NamedKey::ArrowLeft { row.chosen + count - 1 } else { row.chosen + 1 } % count;
                    self.choose_setting(*setting, next);
                }
            }
            Key::Named(NamedKey::Enter | NamedKey::Escape) => {
                self.close_menu();
                return;
            }
            Key::Character(c) if c.eq_ignore_ascii_case("s") && !self.launcher.first => {
                self.close_menu();
                return;
            }
            _ => {}
        }
        self.refresh_menu();
    }

    /// The pointer moved to `point` (window pixels) over the menu.
    pub(crate) fn menu_pointer(&mut self, point: [f32; 2]) {
        self.launcher.pointer = point;
        let hover = self.renderer.as_ref().and_then(|r| r.menu_hit(point));
        if hover != self.launcher.hover {
            self.launcher.hover = hover;
            if let Some(window) = &self.window {
                let icon = if hover.is_some() { winit::window::CursorIcon::Pointer } else { winit::window::CursorIcon::Default };
                window.set_cursor(icon);
            }
            self.refresh_menu();
        }
    }

    /// A click where the pointer is over the menu.
    pub(crate) fn menu_click(&mut self) {
        match self.renderer.as_ref().and_then(|r| r.menu_hit(self.launcher.pointer)) {
            Some(Hit::Button) => self.close_menu(),
            Some(Hit::Choice(r, n)) => {
                if let Some(&(setting, _)) = self.rows().get(r) {
                    self.launcher.focus = r;
                    self.choose_setting(setting, n);
                    self.refresh_menu();
                }
            }
            None => {}
        }
    }

    /// Takes choice `n` of `setting`, and remembers it.
    fn choose_setting(&mut self, setting: Setting, n: usize) {
        match setting {
            Setting::Listen => {
                let sources = self.sources();
                if let Some((listen, _)) = sources.get(n) {
                    self.listen_to(listen.clone());
                }
            }
            Setting::Show => self.settings.show = SHOWS[n.min(SHOWS.len() - 1)].0,
            Setting::Displays => {
                self.settings.spread = SPREADS[n.min(SPREADS.len() - 1)].0;
                self.spread = self.settings.spread;
            }
            Setting::Mode => self.choose_mode(MODES[n.min(MODES.len() - 1)]),
            Setting::Intensity => self.choose_intensity(intensity_at(n)),
            Setting::Titles => self.set_intros(n == 0),
            Setting::AtStart => self.settings.menu = n == 0,
        }
        self.settings.save();
    }

    /// How Juglar is shown right now.
    pub(crate) fn showing(&self) -> Show {
        if self.wallpaper.is_some() {
            Show::Wallpaper
        } else if self.is_fullscreen() {
            Show::Fullscreen
        } else {
            Show::Window
        }
    }

    /// Shows Juglar as the settings say: a window, fullscreen, or the
    /// wallpaper.
    pub(crate) fn apply_show(&mut self) {
        match self.settings.show {
            Show::Window | Show::Fullscreen => {
                if self.wallpaper.is_some() {
                    self.leave_wallpaper();
                }
                if self.is_fullscreen() != (self.settings.show == Show::Fullscreen) {
                    self.toggle_fullscreen();
                }
            }
            Show::Wallpaper => {
                if self.wallpaper.is_none() || self.spread != self.settings.spread {
                    self.spread = self.settings.spread;
                    self.arrange = true;
                }
            }
        }
        self.sync_tray();
    }
}

fn show_index(show: Show) -> usize {
    SHOWS.iter().position(|&(s, _)| s == show).unwrap_or(0)
}

/// The intensity at place `n` of `INTENSITIES`.
pub(crate) fn intensity_at(n: usize) -> Intensity {
    match n {
        0 => Intensity::Natural,
        n => Intensity::Fixed(INTENSITY_PRESETS[(n - 1).min(INTENSITY_PRESETS.len() - 1)]),
    }
}
