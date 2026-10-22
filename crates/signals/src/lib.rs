//! Everything the visuals are allowed to know about the outside world.
//!
//! Visuals never see audio. They see `Signals`: a few hundred numbers, already
//! normalized and smoothed. Audio analysis is one producer of them; the
//! synthetic song in `synthetic` is another, and later time, scroll or mouse
//! input on the web will be more. Plain Rust, no OS APIs: this must also
//! build for `wasm32-unknown-unknown`.

pub mod synthetic;

pub use synthetic::Synthetic;

/// Log-spaced spectrum bands from low to high.
pub const SPECTRUM_BANDS: usize = 32;
/// Points in the waveform shape.
pub const WAVEFORM_POINTS: usize = 256;

/// One frame's worth of signals. Levels are in `0..=1`, the waveform in
/// `-1..=1`. Producers own the smoothing, so consumers can use every value
/// as it comes.
#[derive(Clone, Debug)]
pub struct Signals {
    /// Seconds since start.
    pub time: f32,
    /// Seconds since the previous frame.
    pub dt: f32,
    pub bass: f32,
    pub mid: f32,
    pub treble: f32,
    /// Overall loudness.
    pub energy: f32,
    /// 1 on a beat, falling back to 0 before the next one. From audio, a
    /// beat is a detected onset in the low or middle range (kicks, snares);
    /// the tempo isn't tracked yet (Milestone 5.5).
    pub beat: f32,
    /// High when the music changes character (a drop, a new section),
    /// fading over a few seconds as the change becomes the new normal.
    pub novelty: f32,
    /// How lively the visuals may be: 0 is calm, 1 is strong. Set by the
    /// user, or derived from the music in auto mode (Milestone 6).
    pub intensity: f32,
    pub spectrum: [f32; SPECTRUM_BANDS],
    /// A stable snapshot of the wave, as an oscilloscope shows it.
    pub waveform: [f32; WAVEFORM_POINTS],
}

impl Default for Signals {
    fn default() -> Self {
        Self {
            time: 0.0,
            dt: 0.0,
            bass: 0.0,
            mid: 0.0,
            treble: 0.0,
            energy: 0.0,
            beat: 0.0,
            novelty: 0.0,
            intensity: 0.5,
            spectrum: [0.0; SPECTRUM_BANDS],
            waveform: [0.0; WAVEFORM_POINTS],
        }
    }
}
