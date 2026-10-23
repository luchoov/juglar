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
/// `-1..=1`, relative levels around 1. Producers own the smoothing, so
/// consumers can use every value as it comes.
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
    /// beat is a detected onset in the low or middle range (kicks, snares),
    /// so a beat the music leaves out isn't one here.
    pub beat: f32,
    /// Beats per minute, or 0 while there is no steady beat to count.
    pub tempo: f32,
    /// Where the music is within the beat while `tempo` is known: 0 on the
    /// beat, rising to 1 just before the next, and carrying on through
    /// beats the music leaves out. 0 while the tempo is unknown.
    pub beat_phase: f32,
    /// High when the music changes character (a drop, a new section),
    /// fading over a few seconds as the change becomes the new normal.
    pub novelty: f32,
    /// Each drum on its own: 1 at a hit, falling back to 0 within a few
    /// hundredths of a second, fast enough to follow a double kick at 16
    /// hits a second. From audio, a drum the mix buries isn't heard: in
    /// loud metal, about half the hits of each.
    pub kick: f32,
    pub snare: f32,
    pub cymbals: f32,
    /// How loud the percussive part of the sound is (hits, attacks) and
    /// the harmonic part (held notes, chords, pads). The louder of the two
    /// can reach 1; the other keeps its distance below it.
    pub percussive: f32,
    pub harmonic: f32,
    /// The levels against their own last few seconds (see `Relative`).
    pub relative: Relative,
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
            tempo: 0.0,
            beat_phase: 0.0,
            novelty: 0.0,
            kick: 0.0,
            snare: 0.0,
            cymbals: 0.0,
            percussive: 0.0,
            harmonic: 0.0,
            relative: Relative::default(),
            intensity: 0.5,
            spectrum: [0.0; SPECTRUM_BANDS],
            waveform: [0.0; WAVEFORM_POINTS],
        }
    }
}

/// Levels against their own average over the last four seconds: 1 is usual
/// for this moment of this song, 1.3 stands out, 0.5 has dropped to half.
/// Unlike the absolute levels, they say whether something is *more* than it
/// was, in a quiet song as in a loud one. 0 in silence, never above 4.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Relative {
    pub bass: f32,
    pub mid: f32,
    pub treble: f32,
    pub energy: f32,
    pub percussive: f32,
    pub harmonic: f32,
}
