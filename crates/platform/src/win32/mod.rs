//! Windows implementations of the platform traits.

mod motion;
mod wasapi;

pub use motion::reduced_motion;
pub use wasapi::WasapiLoopback;
