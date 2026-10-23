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

/// Wallpaper mode (drawing behind the desktop icons), the tray icon that
/// controls it, and whether anything covers the desktop. Windows only for
/// now.
#[cfg(windows)]
pub use win32::{ScreenRect, Tray, TrayChoice, TrayState, Wallpaper, desktop_covered};

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

/// Captures what the system is playing. Capture stops when the value drops.
pub trait AudioCapture: Sized {
    fn start(on_samples: SampleCallback) -> Result<Self, PlatformError>;
    fn info(&self) -> StreamInfo;
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
