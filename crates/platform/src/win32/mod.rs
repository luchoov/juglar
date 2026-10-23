//! Windows implementations of the platform traits.

mod console;
mod covered;
mod motion;
mod tray;
mod wallpaper;
mod wasapi;

pub use console::{alert, open_console, output_attached, send_output_to};
pub use covered::desktop_covered;
pub use motion::reduced_motion;
pub use tray::{HOTKEYS, Tray, TrayChoice, TrayState, pointer};
pub use wallpaper::{ScreenRect, Wallpaper};
pub use wasapi::WasapiLoopback;
