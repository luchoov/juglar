//! Windows implementations of the platform traits.

mod covered;
mod motion;
mod tray;
mod wallpaper;
mod wasapi;

pub use covered::desktop_covered;
pub use motion::reduced_motion;
pub use tray::{Tray, TrayChoice, TrayState};
pub use wallpaper::{ScreenRect, Wallpaper};
pub use wasapi::WasapiLoopback;
