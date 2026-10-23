//! Audio analysis for Juglar. Plain Rust, no OS APIs: everything here must
//! also build for `wasm32-unknown-unknown`.

pub mod analyzer;
pub mod fft;
pub mod level;
pub mod onset;
pub mod percussion;
pub mod ring;
pub mod scope;
pub mod smoothing;
pub mod tempo;
pub mod window;

pub use analyzer::{Analysis, Analyzer, FFT_SIZE, SPECTRUM_BANDS};
pub use level::{Level, to_dbfs};
pub use onset::Onsets;
pub use percussion::{DRUMS, Percussion};
pub use ring::{Consumer, Producer, ring};
pub use scope::{SCOPE_POINTS, SCOPE_SPAN, Scope};
pub use tempo::Tempo;
