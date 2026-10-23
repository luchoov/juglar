//! Whether the desktop can be seen: a wallpaper nobody can see should cost
//! nothing.

use windows::Win32::Foundation::RECT;
use windows::Win32::Graphics::Gdi::{MONITOR_DEFAULTTONULL, MonitorFromRect, MonitorFromWindow};
use windows::Win32::UI::Shell::{
    QUNS_BUSY, QUNS_PRESENTATION_MODE, QUNS_RUNNING_D3D_FULL_SCREEN, SHQueryUserNotificationState,
};
use windows::Win32::UI::WindowsAndMessaging::{GetClassNameW, GetForegroundWindow, GetWindowRect, IsZoomed};

use super::wallpaper::ScreenRect;

/// `true` when something covers the desktop over `rect` (a display): a
/// program in fullscreen (a game, a video, a presentation), or a maximized
/// or display-sized window in front on that display.
pub fn desktop_covered(rect: ScreenRect) -> bool {
    // SAFETY: the calls take values and pointers to live locals.
    unsafe {
        if let Ok(state) = SHQueryUserNotificationState()
            && (state == QUNS_BUSY || state == QUNS_RUNNING_D3D_FULL_SCREEN || state == QUNS_PRESENTATION_MODE)
        {
            return true;
        }
        let front = GetForegroundWindow();
        if front.is_invalid() {
            return false;
        }
        // The desktop itself in front (after Windows+D, or a click on it).
        let mut class = [0u16; 32];
        let len = GetClassNameW(front, &mut class) as usize;
        let class = String::from_utf16_lossy(&class[..len]);
        if class == "Progman" || class == "WorkerW" {
            return false;
        }
        let area = RECT { left: rect.left, top: rect.top, right: rect.left + rect.width as i32, bottom: rect.top + rect.height as i32 };
        if MonitorFromWindow(front, MONITOR_DEFAULTTONULL) != MonitorFromRect(&area, MONITOR_DEFAULTTONULL) {
            return false;
        }
        if IsZoomed(front).as_bool() {
            return true;
        }
        let mut window = RECT::default();
        GetWindowRect(front, &mut window).is_ok()
            && window.left <= area.left
            && window.top <= area.top
            && window.right >= area.right
            && window.bottom >= area.bottom
    }
}
