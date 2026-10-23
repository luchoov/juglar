//! Wallpaper mode: Juglar's window drawn behind the desktop icons.
//!
//! Windows has no API for this. Every live-wallpaper program takes the same
//! undocumented path, found by watching what Explorer does for its own
//! wallpaper slideshow: message 0x052C to the desktop's root window
//! (`Progman`) makes Explorer split the desktop into the icons and a layer
//! under them (a `WorkerW` window), and a window placed between the two is
//! drawn as the wallpaper. Where that layer lives changed in Windows 11 24H2:
//!
//! - **Before 24H2**, the icons (`SHELLDLL_DefView`) sit in a top-level
//!   `WorkerW`, and the wallpaper layer is the next top-level `WorkerW`.
//!   Juglar's window becomes a child of that one.
//! - **From 24H2** ("raised desktop"), the icons and the `WorkerW` are both
//!   children of `Progman`. Juglar's window becomes a child of `Progman`
//!   too, placed just under the icons, with Explorer's `WorkerW` pushed
//!   under it. It must be a layered window, and a layered child window only
//!   works in a program whose manifest declares Windows 8 or later (the
//!   app embeds one: `app/juglar.manifest`).
//!
//! Message parameters (0xD, 1) are those Lively Wallpaper and others settled
//! on for 24H2: Explorer counts requests, and (0xD, 0) releases the layer.

use std::ffi::c_void;
use std::thread::sleep;
use std::time::Duration;

use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, POINT, WPARAM};
use windows::Win32::Graphics::Gdi::MapWindowPoints;
use windows::Win32::UI::WindowsAndMessaging::{
    FindWindowExW, FindWindowW, GWL_EXSTYLE, GWL_STYLE, GetParent, GetWindowLongPtrW, HWND_BOTTOM, HWND_TOP, LWA_ALPHA,
    SMTO_NORMAL, SWP_FRAMECHANGED, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_SHOWWINDOW, SendMessageTimeoutW,
    SetLayeredWindowAttributes, SetParent, SetWindowLongPtrW, SetWindowPos, WS_CAPTION, WS_CHILD, WS_EX_LAYERED,
    WS_POPUP, WS_THICKFRAME, WS_VISIBLE, EnumWindows,
};
use windows::core::{BOOL, PCWSTR, w};

use crate::PlatformError;

/// Asks Explorer for the layer between the wallpaper and the icons.
const SPLIT_DESKTOP: u32 = 0x052C;

/// A rectangle on the screen, in physical pixels.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ScreenRect {
    pub left: i32,
    pub top: i32,
    pub width: u32,
    pub height: u32,
}

/// Where the desktop's layers are.
enum Layout {
    /// 24H2 and later: icons and wallpaper layer inside `Progman`.
    Raised { icons: HWND, layer: Option<HWND> },
    /// Earlier: the wallpaper layer is a top-level `WorkerW`.
    Split { layer: HWND },
}

/// A window turned into the wallpaper. Turning it back (`restore`, or
/// dropping this) makes it an ordinary window again.
pub struct Wallpaper {
    window: HWND,
    style: isize,
    ex_style: isize,
}

impl Wallpaper {
    /// Puts `window` (a Win32 window handle) behind the desktop icons,
    /// covering `rect`.
    pub fn attach(window: isize, rect: ScreenRect) -> Result<Self, PlatformError> {
        let window = HWND(window as *mut c_void);
        // SAFETY: every call below takes window handles by value and
        // pointers to live locals; a stale handle makes a call fail, which
        // is checked, not undefined behavior.
        unsafe {
            let progman = FindWindowW(w!("Progman"), PCWSTR::null()).map_err(|e| failed("finding the desktop", e))?;
            let mut answer = 0usize;
            SendMessageTimeoutW(progman, SPLIT_DESKTOP, WPARAM(0xD), LPARAM(1), SMTO_NORMAL, 1000, Some(&mut answer));
            // Explorer builds the layer within a moment; give it two seconds.
            let mut found = layout(progman);
            for _ in 0..40 {
                if matches!(found, Some(Layout::Split { .. }) | Some(Layout::Raised { layer: Some(_), .. })) {
                    break;
                }
                sleep(Duration::from_millis(50));
                found = layout(progman);
            }
            let layout = found.ok_or_else(|| PlatformError::new("the desktop has no layer for a wallpaper"))?;

            let style = GetWindowLongPtrW(window, GWL_STYLE);
            let ex_style = GetWindowLongPtrW(window, GWL_EXSTYLE);
            let child = (style & !((WS_POPUP | WS_CAPTION | WS_THICKFRAME).0 as isize)) | (WS_CHILD | WS_VISIBLE).0 as isize;
            let parent = match layout {
                Layout::Raised { icons, layer } => {
                    SetWindowLongPtrW(window, GWL_EXSTYLE, ex_style | WS_EX_LAYERED.0 as isize);
                    SetLayeredWindowAttributes(window, COLORREF(0), 255, LWA_ALPHA)
                        .map_err(|e| failed("making the window layered", e))?;
                    SetWindowLongPtrW(window, GWL_STYLE, child);
                    SetParent(window, Some(progman)).map_err(|e| failed("moving the window into the desktop", e))?;
                    place(window, progman, icons, rect)?;
                    if let Some(layer) = layer {
                        // Explorer's own layer goes under Juglar's window.
                        let _ = SetWindowPos(layer, Some(HWND_BOTTOM), 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE);
                    }
                    progman
                }
                Layout::Split { layer } => {
                    SetWindowLongPtrW(window, GWL_STYLE, child);
                    SetParent(window, Some(layer)).map_err(|e| failed("moving the window into the desktop", e))?;
                    place(window, layer, HWND_TOP, rect)?;
                    layer
                }
            };
            // SetParent can report success and leave the window where it was.
            if GetParent(window).ok() != Some(parent) {
                SetWindowLongPtrW(window, GWL_STYLE, style);
                SetWindowLongPtrW(window, GWL_EXSTYLE, ex_style);
                return Err(PlatformError::new("the window did not move into the desktop"));
            }
            Ok(Self { window, style, ex_style })
        }
    }

    /// Makes the window an ordinary window again, covering `rect`.
    pub fn restore(self, rect: ScreenRect) {
        self.detach(Some(rect));
    }

    fn detach(&self, rect: Option<ScreenRect>) {
        // SAFETY: as in `attach`.
        unsafe {
            let _ = SetParent(self.window, None);
            SetWindowLongPtrW(self.window, GWL_STYLE, self.style);
            SetWindowLongPtrW(self.window, GWL_EXSTYLE, self.ex_style);
            let (x, y, cx, cy, flags) = match rect {
                Some(r) => (r.left, r.top, r.width as i32, r.height as i32, SWP_FRAMECHANGED | SWP_SHOWWINDOW),
                None => (0, 0, 0, 0, SWP_FRAMECHANGED | SWP_NOMOVE | SWP_NOSIZE),
            };
            let _ = SetWindowPos(self.window, None, x, y, cx, cy, flags);
        }
    }
}

impl Drop for Wallpaper {
    /// Leaves the desktop as it was, even if Juglar quits from wallpaper
    /// mode: the window is taken out before it is destroyed.
    fn drop(&mut self) {
        self.detach(None);
    }
}

fn failed(what: &str, e: windows::core::Error) -> PlatformError {
    PlatformError::new(format!("{what}: {e}"))
}

/// Finds the desktop's layers as Explorer has them now.
fn layout(progman: HWND) -> Option<Layout> {
    // SAFETY: as in `Wallpaper::attach`.
    unsafe {
        if let Ok(icons) = FindWindowExW(Some(progman), None, w!("SHELLDLL_DefView"), PCWSTR::null()) {
            let layer = FindWindowExW(Some(progman), None, w!("WorkerW"), PCWSTR::null()).ok();
            return Some(Layout::Raised { icons, layer });
        }
        // Before 24H2: the top-level window holding the icons, and the
        // `WorkerW` right after it.
        let mut holder = HWND::default();
        unsafe extern "system" fn find_icons(window: HWND, holder: LPARAM) -> BOOL {
            // SAFETY: `holder` is the address of the local `HWND` passed to
            // `EnumWindows` below, alive for the whole enumeration.
            unsafe {
                if FindWindowExW(Some(window), None, w!("SHELLDLL_DefView"), PCWSTR::null()).is_ok() {
                    *(holder.0 as *mut HWND) = window;
                    return BOOL(0);
                }
            }
            BOOL(1)
        }
        let _ = EnumWindows(Some(find_icons), LPARAM(&raw mut holder as isize));
        if holder.is_invalid() {
            return None;
        }
        let layer = FindWindowExW(None, Some(holder), w!("WorkerW"), PCWSTR::null()).ok()?;
        Some(Layout::Split { layer })
    }
}

/// Moves `window` (a child of `parent`) over `rect` of the screen, just
/// under `after` among its parent's children (`HWND_TOP`: above them all).
unsafe fn place(window: HWND, parent: HWND, after: HWND, rect: ScreenRect) -> Result<(), PlatformError> {
    // The parent may not start at the screen's corner: Progman spans every
    // display, starting at the leftmost and topmost.
    let mut corner = [POINT { x: rect.left, y: rect.top }];
    // SAFETY: as in `Wallpaper::attach`.
    unsafe {
        MapWindowPoints(None, Some(parent), &mut corner);
        let flags = SWP_NOACTIVATE | SWP_SHOWWINDOW | SWP_FRAMECHANGED;
        SetWindowPos(window, Some(after), corner[0].x, corner[0].y, rect.width as i32, rect.height as i32, flags)
            .map_err(|e| failed("placing the window on the desktop", e))
    }
}
