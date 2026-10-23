//! Wallpaper mode on every display (D58): one window behind the desktop
//! icons on each display, each with its own scene, or one window across
//! them all, or only the display Juglar's window was on. The main window
//! becomes one of the wallpapers; the others are made for the occasion and
//! dropped when Juglar goes back to a window. The displays are checked once
//! a second, and the wallpapers laid out again when one comes or goes.

use std::sync::Arc;

use juglar_director::{Director, Intensity, Mode};
use juglar_platform::{HOTKEYS, ScreenRect, Tray, TrayChoice, Wallpaper, desktop_covered, pointer};
use juglar_render::Renderer;
use juglar_signals::Signals;
use winit::dpi::{PhysicalPosition, PhysicalSize};
use winit::event_loop::ActiveEventLoop;
use winit::monitor::MonitorHandle;
use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
use winit::window::{Window, WindowId};

use crate::{App, rect, title, ui_fonts, window_attributes};

/// How wallpaper mode uses several displays.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Spread {
    /// A window and a scene on each display.
    Each,
    /// One window across all of them.
    Across,
    /// Only the display the window was on.
    One,
}

/// The choices as the tray's menu lists them.
pub(crate) const SPREADS: [(Spread, &str); 3] =
    [(Spread::Each, "A scene on each"), (Spread::Across, "One scene across all"), (Spread::One, "Only this display")];

impl Spread {
    /// `JUGLAR_DISPLAYS=across` (or `each`, `one`); a scene on each by
    /// default.
    pub(crate) fn from_env() -> Self {
        match std::env::var("JUGLAR_DISPLAYS").as_deref() {
            Ok("across") => Spread::Across,
            Ok("one") => Spread::One,
            _ => Spread::Each,
        }
    }
}

/// A wallpaper on another display than the main window's. Fields drop in
/// order: the window leaves the desktop before it is destroyed.
pub(crate) struct Other {
    /// Held for its drop, which takes the window out of the desktop.
    _wallpaper: Wallpaper,
    pub(crate) renderer: Renderer,
    pub(crate) director: Director,
    window: Arc<Window>,
    pub(crate) display: ScreenRect,
    covered: bool,
}

impl Other {
    pub(crate) fn id(&self) -> WindowId {
        self.window.id()
    }
}

/// The help while Juglar is the wallpaper: the keys that work from anywhere.
pub(crate) fn hotkey_help() -> Vec<(&'static str, &'static str)> {
    HOTKEYS
        .iter()
        .map(|&(keys, choice)| {
            let does = match choice {
                TrayChoice::PreviousScene => "previous scene, on the display under the pointer",
                TrayChoice::NextScene => "next scene, likewise",
                TrayChoice::KeepScene => "keep that scene / let the music change it",
                TrayChoice::NextMode => "mode: lively, focus, relax",
                TrayChoice::NextIntensity => "intensity: the music's, gentle, medium, strong",
                TrayChoice::Help => "this help",
                TrayChoice::Window => "back to a window",
                _ => "",
            };
            (keys, does)
        })
        .collect()
}

/// The displays' choices for the tray, and the one taken; none with a
/// single display.
pub(crate) fn spread_names(displays: &[ScreenRect], spread: Spread) -> (Vec<String>, usize) {
    if displays.len() < 2 {
        return (Vec::new(), 0);
    }
    let names = SPREADS.iter().map(|&(_, name)| name.to_owned()).collect();
    (names, SPREADS.iter().position(|&(s, _)| s == spread).unwrap_or(0))
}

/// Every display's rectangle, the primary first.
fn displays(event_loop: &ActiveEventLoop) -> Vec<ScreenRect> {
    let primary = event_loop.primary_monitor();
    let mut monitors: Vec<MonitorHandle> = event_loop.available_monitors().collect();
    monitors.sort_by_key(|m| Some(m) != primary.as_ref());
    monitors.iter().map(|m| rect(m.position(), m.size())).collect()
}

/// The smallest rectangle around all of `rects`.
fn union(rects: &[ScreenRect]) -> ScreenRect {
    let left = rects.iter().map(|r| r.left).min().unwrap_or(0);
    let top = rects.iter().map(|r| r.top).min().unwrap_or(0);
    let right = rects.iter().map(|r| r.left + r.width as i32).max().unwrap_or(0);
    let bottom = rects.iter().map(|r| r.top + r.height as i32).max().unwrap_or(0);
    ScreenRect { left, top, width: (right - left) as u32, height: (bottom - top) as u32 }
}

fn contains(r: &ScreenRect, (x, y): (i32, i32)) -> bool {
    x >= r.left && y >= r.top && x < r.left + r.width as i32 && y < r.top + r.height as i32
}

fn hwnd(window: &Window) -> Option<isize> {
    match window.window_handle().ok()?.as_raw() {
        RawWindowHandle::Win32(handle) => Some(handle.hwnd.get()),
        _ => None,
    }
}

impl App {
    /// Lays the wallpapers out over the displays as `spread` asks, from
    /// scratch: the first time, and whenever the displays or the choice
    /// change.
    pub(crate) fn arrange_wallpaper(&mut self, event_loop: &ActiveEventLoop) {
        if self.is_fullscreen() {
            self.toggle_fullscreen();
        }
        // Out of the desktop first, so the window's own place is known.
        self.others.clear();
        let windowed = match self.wallpaper.take() {
            Some((wallpaper, _, windowed)) => {
                wallpaper.restore(windowed);
                windowed
            }
            None => match &self.window {
                Some(window) => rect(window.outer_position().unwrap_or_default(), window.outer_size()),
                None => return,
            },
        };
        self.displays = displays(event_loop);
        let Some(window) = self.window.clone() else { return };
        // The window's own display, or the primary one if it is gone.
        let center = (windowed.left + windowed.width as i32 / 2, windowed.top + windowed.height as i32 / 2);
        let own = self.displays.iter().position(|d| contains(d, center)).unwrap_or(0);
        let Some(&home) = self.displays.get(own) else { return };
        let main = match self.spread {
            Spread::Across => union(&self.displays),
            Spread::Each | Spread::One => home,
        };
        let Some(handle) = hwnd(&window) else { return };
        match Wallpaper::attach(handle, main) {
            Ok(wallpaper) => self.wallpaper = Some((wallpaper, main, windowed)),
            Err(e) => {
                eprintln!("juglar: could not become the wallpaper: {e}");
                return;
            }
        }
        if self.spread == Spread::Each {
            let rest: Vec<ScreenRect> = self.displays.iter().enumerate().filter(|&(n, _)| n != own).map(|(_, &d)| d).collect();
            for display in rest {
                match self.other(event_loop, display) {
                    Ok(other) => self.others.push(other),
                    Err(e) => eprintln!("juglar: no wallpaper on the display at {}, {}: {e}", display.left, display.top),
                }
            }
        }
        if self.tray.is_none() {
            match Tray::new("Juglar") {
                Ok(tray) => self.tray = Some(tray),
                Err(e) => eprintln!("juglar: no tray icon ({e}); quit from the Task Manager"),
            }
        }
        println!(
            "Wallpaper mode on {} of {} displays: drawing behind the desktop icons. The tray icon and Ctrl+Alt keys have the controls.",
            1 + self.others.len(),
            self.displays.len()
        );
        self.covered = false;
        self.sync_tray();
    }

    /// A wallpaper of its own for `display`, starting from the main
    /// window's settings.
    fn other(&self, event_loop: &ActiveEventLoop, display: ScreenRect) -> Result<Other, String> {
        let attributes = window_attributes()
            .with_visible(false)
            .with_decorations(false)
            .with_position(PhysicalPosition::new(display.left, display.top))
            .with_inner_size(PhysicalSize::new(display.width, display.height));
        let window = Arc::new(event_loop.create_window(attributes).map_err(|e| e.to_string())?);
        let mut renderer =
            pollster::block_on(Renderer::new(window.clone(), display.width, display.height)).map_err(|e| e.to_string())?;
        renderer.set_fonts(&ui_fonts());
        renderer.set_reduced_motion(self.still);
        let mut director = Director::new(self.director.intensity(), juglar_render::scene_characters());
        let mode = self.director.mode();
        director.set_mode(mode);
        renderer.set_calm(mode.calm());
        director.hold(self.director.holding());
        // Start where no other display is.
        let taken = self.scenes();
        if let Some(free) = (0..renderer.scene_names().len()).find(|n| !taken.contains(n)) {
            renderer.set_scene(free);
            director.chosen(free);
        }
        let handle = hwnd(&window).ok_or("no window handle")?;
        let wallpaper = Wallpaper::attach(handle, display).map_err(|e| e.to_string())?;
        Ok(Other { _wallpaper: wallpaper, renderer, director, window, display, covered: false })
    }

    /// Back to an ordinary window, where it was before.
    pub(crate) fn leave_wallpaper(&mut self) {
        self.others.clear();
        if let Some((wallpaper, _, windowed)) = self.wallpaper.take() {
            wallpaper.restore(windowed);
        }
        self.tray = None;
        self.covered = false;
        if let Some(window) = &self.window {
            window.focus_window();
        }
    }

    /// The scene each display shows, the main window's first.
    pub(crate) fn scenes(&self) -> Vec<usize> {
        self.renderer.iter().map(Renderer::scene).chain(self.others.iter().map(|o| o.renderer.scene())).collect()
    }

    /// Which wallpaper the pointer is over: `None` for the main window's.
    pub(crate) fn under_pointer(&self) -> Option<usize> {
        let at = pointer()?;
        self.others.iter().position(|o| contains(&o.display, at))
    }

    /// The renderer and director of a display: `None` is the main window.
    pub(crate) fn screen(&mut self, which: Option<usize>) -> Option<(&mut Renderer, &mut Director)> {
        match which {
            Some(n) => self.others.get_mut(n).map(|o| (&mut o.renderer, &mut o.director)),
            None => self.renderer.as_mut().map(|r| (r, &mut self.director)),
        }
    }

    /// Once a second, while Juglar is the wallpaper: whether each display's
    /// desktop can be seen, and whether the displays changed.
    pub(crate) fn watch_desktop(&mut self, event_loop: &ActiveEventLoop) {
        let Some((_, area, _)) = &self.wallpaper else { return };
        // Across all displays, it is covered only once every one is.
        self.covered = match self.spread {
            Spread::Across => self.displays.iter().all(|&d| desktop_covered(d)),
            Spread::Each | Spread::One => desktop_covered(*area),
        };
        for other in &mut self.others {
            other.covered = desktop_covered(other.display);
        }
        if displays(event_loop) != self.displays {
            println!("The displays changed; laying the wallpapers out again.");
            self.arrange_wallpaper(event_loop);
        }
    }

    /// Changes, then draws, the other displays' wallpapers, each from its
    /// own copy of what was heard.
    pub(crate) fn draw_others(&mut self, heard: &Signals) {
        let scenes = self.scenes();
        for (n, other) in self.others.iter_mut().enumerate() {
            let elsewhere: Vec<usize> = scenes.iter().enumerate().filter(|&(m, _)| m != n + 1).map(|(_, &s)| s).collect();
            other.director.elsewhere(&elsewhere);
            let mut signals = heard.clone();
            let busy = other.renderer.in_transition() || self.still;
            if let Some(change) = other.director.update(&mut signals, other.renderer.scene(), busy) {
                other.renderer.begin_transition(change.scene, change.transition);
                other.renderer.director_changed(crate::moment_name(change.moment), change.scene);
            }
            if !other.covered {
                other.renderer.render(&signals);
            }
        }
    }

    /// Follows a resize of one of the other wallpapers' windows.
    pub(crate) fn resize_other(&mut self, id: WindowId, size: PhysicalSize<u32>) {
        if let Some(other) = self.others.iter_mut().find(|o| o.id() == id) {
            other.renderer.resize(size.width, size.height);
        }
    }


    /// The next intensity: the music's, then gentle, medium and strong.
    pub(crate) fn next_intensity(&self) -> Intensity {
        let presets = crate::INTENSITY_PRESETS;
        match self.director.intensity() {
            Intensity::Natural => Intensity::Fixed(presets[0]),
            Intensity::Fixed(value) => match presets.iter().position(|&p| (p - value).abs() < 1e-3) {
                Some(n) if n + 1 < presets.len() => Intensity::Fixed(presets[n + 1]),
                _ => Intensity::Natural,
            },
        }
    }

    /// Sets a mode on every display.
    pub(crate) fn mode_everywhere(&mut self, mode: Mode) {
        for other in &mut self.others {
            other.director.set_mode(mode);
            other.renderer.set_calm(mode.calm());
            other.renderer.note("Mode", mode.name());
        }
    }

    /// Sets an intensity on every display, likewise.
    pub(crate) fn intensity_everywhere(&mut self, intensity: Intensity, name: &str) {
        for other in &mut self.others {
            other.director.set_intensity(intensity, self.signals.intensity);
            other.renderer.note("Intensity", name);
        }
    }

    /// The name of the scene a display shows, as a title.
    pub(crate) fn scene_title(renderer: &Renderer) -> String {
        title(renderer.scene_names()[renderer.scene()])
    }
}
