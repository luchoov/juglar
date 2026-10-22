//! Audio analysis for Juglar. Plain Rust, no OS APIs: everything here must
//! also build for `wasm32-unknown-unknown`.

pub mod level;
pub mod ring;

pub use level::{Level, to_dbfs};
pub use ring::{Consumer, Producer, ring};
