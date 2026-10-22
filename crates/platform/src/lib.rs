//! Per-OS services for Juglar, behind OS-agnostic traits.
//!
//! The core never calls an operating-system API. Each OS module implements the
//! traits here, and `SystemAudio` names the implementation for the current OS.

use std::fmt;

#[cfg(windows)]
mod win32;

#[cfg(windows)]
pub use win32::WasapiLoopback as SystemAudio;

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
