//! What the listener chose, kept between runs in
//! `%LOCALAPPDATA%\Juglar\settings.ini`: what to listen to, how to show
//! Juglar, the mode, the intensity, whether to show song titles, and
//! whether to open the menu first. One `key = value` a line, written again
//! whenever something changes; anything missing or unknown keeps its
//! default. The `JUGLAR_*` variables and the command line still win.

use std::path::PathBuf;

use juglar_director::Mode;
use juglar_platform::Listen;

use crate::desktop::Spread;

/// How Juglar is shown once it starts.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Show {
    #[default]
    Window,
    Fullscreen,
    Wallpaper,
}

/// The ways to show Juglar, as the menus list them.
pub(crate) const SHOWS: [(Show, &str); 3] =
    [(Show::Window, "Window"), (Show::Fullscreen, "Fullscreen"), (Show::Wallpaper, "Wallpaper")];

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Settings {
    pub(crate) listen: Listen,
    pub(crate) show: Show,
    pub(crate) spread: Spread,
    pub(crate) mode: Mode,
    /// A place in `INTENSITIES`: the music's, gentle, medium, strong.
    pub(crate) intensity: usize,
    /// Show each new song's title over the scene.
    pub(crate) intros: bool,
    /// Open the menu when Juglar starts.
    pub(crate) menu: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            listen: Listen::Everything,
            show: Show::Window,
            spread: Spread::Each,
            mode: Mode::Lively,
            intensity: 0,
            intros: true,
            menu: true,
        }
    }
}

impl Settings {
    /// The settings saved last, or the defaults.
    pub(crate) fn load() -> Self {
        path().and_then(|p| std::fs::read_to_string(p).ok()).map_or_else(Self::default, |text| Self::parse(&text))
    }

    pub(crate) fn parse(text: &str) -> Self {
        let mut settings = Self::default();
        for line in text.lines() {
            let Some((key, value)) = line.split_once('=') else { continue };
            let value = value.trim();
            match key.trim() {
                "listen" if value == "everything" || value.is_empty() => settings.listen = Listen::Everything,
                "listen" => settings.listen = Listen::App(value.to_owned()),
                "show" => {
                    settings.show = match value {
                        "fullscreen" => Show::Fullscreen,
                        "wallpaper" => Show::Wallpaper,
                        _ => Show::Window,
                    }
                }
                "displays" => {
                    settings.spread = match value {
                        "across" => Spread::Across,
                        "one" => Spread::One,
                        _ => Spread::Each,
                    }
                }
                "mode" => {
                    settings.mode = match value {
                        "focus" => Mode::Focus,
                        "relax" => Mode::Relax,
                        _ => Mode::Lively,
                    }
                }
                "intensity" => {
                    settings.intensity = match value {
                        "gentle" => 1,
                        "medium" => 2,
                        "strong" => 3,
                        _ => 0,
                    }
                }
                "song_titles" => settings.intros = value != "off",
                "menu_at_start" => settings.menu = value != "off",
                _ => {}
            }
        }
        settings
    }

    pub(crate) fn text(&self) -> String {
        let listen = match &self.listen {
            Listen::Everything => "everything",
            Listen::App(exe) => exe,
        };
        let show = match self.show {
            Show::Window => "window",
            Show::Fullscreen => "fullscreen",
            Show::Wallpaper => "wallpaper",
        };
        let displays = match self.spread {
            Spread::Each => "each",
            Spread::Across => "across",
            Spread::One => "one",
        };
        let intensity = ["music", "gentle", "medium", "strong"][self.intensity.min(3)];
        let on = |on: bool| if on { "on" } else { "off" };
        format!(
            "# Juglar's settings, written by Juglar.\nlisten = {listen}\nshow = {show}\ndisplays = {displays}\nmode = {}\nintensity = {intensity}\nsong_titles = {}\nmenu_at_start = {}\n",
            self.mode.name(),
            on(self.intros),
            on(self.menu),
        )
    }

    /// Writes the settings down; a failure only costs remembering them.
    pub(crate) fn save(&self) {
        let Some(path) = path() else { return };
        if let Some(folder) = path.parent() {
            let _ = std::fs::create_dir_all(folder);
        }
        if let Err(e) = std::fs::write(&path, self.text()) {
            eprintln!("juglar: could not save the settings: {e}");
        }
    }
}

fn path() -> Option<PathBuf> {
    std::env::var_os("LOCALAPPDATA").map(|dir| PathBuf::from(dir).join("Juglar").join("settings.ini"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_come_back_as_written() {
        let settings = Settings {
            listen: Listen::App("Spotify.exe".into()),
            show: Show::Wallpaper,
            spread: Spread::Across,
            mode: Mode::Relax,
            intensity: 2,
            intros: false,
            menu: false,
        };
        assert_eq!(Settings::parse(&settings.text()), settings);
    }

    #[test]
    fn unknown_lines_keep_the_defaults() {
        assert_eq!(Settings::parse("nonsense\nshow = sideways\ncolor = red"), Settings::default());
    }
}
