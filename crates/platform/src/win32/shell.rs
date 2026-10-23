//! Small ways of fitting into Windows: a dark title bar, starting with
//! Windows, and one Juglar at a time.

use windows::Win32::Foundation::{ERROR_ALREADY_EXISTS, GetLastError, HWND, LPARAM, WPARAM};
use windows::Win32::Graphics::Dwm::{DWMWA_USE_IMMERSIVE_DARK_MODE, DwmSetWindowAttribute};
use windows::Win32::System::Registry::{
    HKEY, HKEY_CURRENT_USER, KEY_QUERY_VALUE, KEY_SET_VALUE, REG_SZ, RegCloseKey, RegDeleteValueW, RegOpenKeyExW,
    RegQueryValueExW, RegSetValueExW,
};
use windows::Win32::System::Threading::CreateMutexW;
use windows::Win32::UI::WindowsAndMessaging::{FindWindowW, PostMessageW};
use windows::core::{BOOL, w};

use super::tray::{CLASS, SHOW_MESSAGE};

/// The user's Run key, and Juglar's value in it: the same the installer
/// writes (D60), so either can turn it off.
const RUN: windows::core::PCWSTR = w!("Software\\Microsoft\\Windows\\CurrentVersion\\Run");
const VALUE: windows::core::PCWSTR = w!("Juglar");

/// Gives the window `hwnd` a dark title bar, like Juglar's dark pictures.
pub fn dark_title_bar(hwnd: isize) {
    let dark = BOOL(1);
    // SAFETY: the attribute takes a BOOL, passed with its size; a window
    // or a Windows that doesn't know the attribute just refuses it.
    unsafe {
        let _ = DwmSetWindowAttribute(
            HWND(hwnd as *mut _),
            DWMWA_USE_IMMERSIVE_DARK_MODE,
            (&raw const dark).cast(),
            size_of::<BOOL>() as u32,
        );
    }
}

/// Whether Juglar starts with Windows: whether its value is in the Run key.
pub fn starts_with_windows() -> bool {
    // SAFETY: the key is opened to read one value's size, then closed.
    unsafe {
        let mut key = HKEY::default();
        if RegOpenKeyExW(HKEY_CURRENT_USER, RUN, None, KEY_QUERY_VALUE, &mut key).is_err() {
            return false;
        }
        let found = RegQueryValueExW(key, VALUE, None, None, None, None).is_ok();
        let _ = RegCloseKey(key);
        found
    }
}

/// Starts this program with Windows, with `arguments`, or stops it.
pub fn set_starts_with_windows(on: bool, arguments: &str) -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let command = format!("\"{}\" {arguments}", exe.display());
    let data: Vec<u8> = command.encode_utf16().chain([0]).flat_map(u16::to_le_bytes).collect();
    // SAFETY: the key is opened to write or delete one value, then closed;
    // the data is a nul-terminated UTF-16 string, as REG_SZ asks.
    unsafe {
        let mut key = HKEY::default();
        RegOpenKeyExW(HKEY_CURRENT_USER, RUN, None, KEY_SET_VALUE, &mut key).ok().map_err(|e| e.to_string())?;
        let result = if on {
            RegSetValueExW(key, VALUE, None, REG_SZ, Some(&data))
        } else {
            match RegDeleteValueW(key, VALUE) {
                e if e.is_err() && !starts_with_windows() => windows::Win32::Foundation::ERROR_SUCCESS,
                e => e,
            }
        };
        let _ = RegCloseKey(key);
        result.ok().map_err(|e| e.to_string())
    }
}

/// Whether another Juglar is running already; if so, it is asked to show
/// itself, and this one should end.
pub fn already_running() -> bool {
    // SAFETY: the mutex is created once and deliberately never closed: it
    // marks this process as the running Juglar until it ends.
    unsafe {
        let created = CreateMutexW(None, false, w!("Local\\Juglar.SingleInstance"));
        if created.is_err() || GetLastError() != ERROR_ALREADY_EXISTS {
            return false;
        }
        if let Ok(window) = FindWindowW(CLASS, None) {
            let _ = PostMessageW(Some(window), SHOW_MESSAGE, WPARAM(0), LPARAM(0));
        }
        true
    }
}
