//! Windows implementations of the platform traits.

mod apps;
mod console;
mod covered;
mod media;
mod motion;
mod shell;
mod tray;
mod wallpaper;
mod wasapi;

pub use console::{alert, open_console, output_attached, send_output_to};
pub use covered::desktop_covered;
pub use media::MediaWatch;
pub use motion::reduced_motion;
pub use shell::{already_running, dark_title_bar, set_starts_with_windows, starts_with_windows};
pub use tray::{HOTKEYS, Tray, TrayChoice, TrayState, pointer};
pub use wallpaper::{ScreenRect, Wallpaper};
pub use wasapi::WasapiLoopback;
