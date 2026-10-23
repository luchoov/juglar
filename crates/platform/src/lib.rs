//! Per-OS services for Juglar, behind OS-agnostic traits.
//!
//! The core never calls an operating-system API. Each OS module implements the
//! traits here, and `SystemAudio` names the implementation for the current OS.

use std::fmt;

#[cfg(windows)]
mod win32;

#[cfg(windows)]
pub use win32::WasapiLoopback as SystemAudio;

/// Whether the user asked the OS for less motion on screen. Cheap enough to
/// ask every second, so a change applies while Juglar runs.
#[cfg(windows)]
pub use win32::reduced_motion;

/// Wallpaper mode (drawing behind the desktop icons), the tray icon, and
/// whether anything covers the desktop. Windows only for now.
#[cfg(windows)]
pub use win32::{HOTKEYS, ScreenRect, Tray, TrayChoice, TrayState, Wallpaper, desktop_covered, pointer};

/// What the media players say is playing, and which programs make sound
/// (`MediaWatch`); the window's dark title bar; starting with Windows; one
/// Juglar at a time.
#[cfg(windows)]
pub use win32::{MediaWatch, already_running, dark_title_bar, set_starts_with_windows, starts_with_windows};

/// Where the messages go when the program has no console: whether they
/// already lead somewhere, a console to show them in, or a log file; and a
/// message box for the errors Juglar can't go on after.
#[cfg(windows)]
pub use win32::{alert, open_console, output_attached, send_output_to};

/// Without a known OS preference, motion is allowed.
#[cfg(not(windows))]
pub fn reduced_motion() -> bool {
    false
}

/// Receives interleaved `f32` frames and the channel count on the capture
/// thread. It must not block or allocate: it runs at the audio engine's pace.
pub type SampleCallback = Box<dyn FnMut(&[f32], u16) + Send + 'static>;

/// Format of the samples delivered to a `SampleCallback`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StreamInfo {
    pub sample_rate: u32,
    pub channels: u16,
}

/// What to listen to.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum Listen {
    /// Everything the speakers play.
    #[default]
    Everything,
    /// One program, by the name of its file (`Spotify.exe`), with the
    /// processes it started: a game or a call stays out of the analysis.
    App(String),
}

/// Captures what the system, or one program, is playing. Capture stops
/// when the value drops.
pub trait AudioCapture: Sized {
    fn start(from: &Listen, on_samples: SampleCallback) -> Result<Self, PlatformError>;
    fn info(&self) -> StreamInfo;
    /// Whether it still hears what it was asked to: a program's capture
    /// ends with the program.
    fn alive(&self) -> bool;
}

/// A program that has opened sound, as Windows' volume mixer lists it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AudioApp {
    /// Its file's name, `Spotify.exe`: what `Listen::App` takes.
    pub exe: String,
    /// Its name for people, from the file's description: "Spotify".
    pub name: String,
    /// Whether it is playing something right now.
    pub playing: bool,
}

/// The song a media player says it is playing.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Track {
    pub title: String,
    pub artist: String,
    pub album: String,
    /// The player, as Windows names it (`Spotify.exe`, `Chrome`, `MSEdge`).
    pub player: String,
    pub playing: bool,
    /// Counts up with each new song, so a cover can be matched to its song.
    pub id: u64,
}

/// A song's cover, straight (not premultiplied) 8-bit RGBA, rows top to
/// bottom.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Cover {
    /// The `Track::id` it belongs to.
    pub id: u64,
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

/// News from `MediaWatch`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Media {
    /// The song changed, started or stopped playing; `None` when no player
    /// says anything.
    Track(Option<Track>),
    Cover(Cover),
    /// The programs making sound changed.
    Apps(Vec<AudioApp>),
}

#[derive(Debug)]
pub struct PlatformError(String);

impl PlatformError {
    pub fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }
}

impl fmt::Display for PlatformError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for PlatformError {}
