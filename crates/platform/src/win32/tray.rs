//! An icon in the notification area, with a menu: the only way to reach
//! Juglar while it is the wallpaper, since its window lies under the
//! desktop icons and never gets the keyboard.
//!
//! The icon talks to a hidden window of its own, created on the thread that
//! runs the app's event loop, so its messages arrive there with everyone
//! else's. Choices from the menu wait in a queue until the app asks.

use std::cell::RefCell;
use std::collections::VecDeque;

use windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, POINT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Shell::{
    NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NOTIFYICONDATAW, Shell_NotifyIconW,
};
use windows::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CW_USEDEFAULT, CreatePopupMenu, CreateWindowExW, DefWindowProcW, DestroyMenu, DestroyWindow,
    GetCursorPos, GetSystemMetrics, HICON, IDI_APPLICATION, IMAGE_ICON, LR_SHARED, LoadIconW, LoadImageW, MF_CHECKED, MF_DISABLED, MF_POPUP, MF_SEPARATOR, MF_STRING, MF_UNCHECKED, PostMessageW,
    RegisterClassW, SM_CXSMICON, SM_CYSMICON, SetForegroundWindow, TPM_RETURNCMD, TPM_RIGHTBUTTON, TrackPopupMenu, WM_APP, WM_LBUTTONUP, WM_NULL, WM_RBUTTONUP, WNDCLASSW, WS_EX_TOOLWINDOW, WS_OVERLAPPED,
};
use windows::core::{PCWSTR, w};

use crate::PlatformError;

/// What the listener chose from the menu.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrayChoice {
    NextScene,
    /// Keep the scene showing, or let go.
    KeepScene,
    /// One of `TrayState::modes`, by its place in the list.
    Mode(usize),
    /// One of `TrayState::intensities`, likewise.
    Intensity(usize),
    /// Leave wallpaper mode for a window.
    Window,
    Quit,
}

/// What the menu shows: the scene, and the choices with the current one
/// checked.
#[derive(Clone, Debug, Default)]
pub struct TrayState {
    /// The scene showing, as a title at the top of the menu.
    pub scene: String,
    pub keeping: bool,
    pub modes: Vec<String>,
    pub mode: usize,
    pub intensities: Vec<String>,
    pub intensity: usize,
}

/// The icon's messages to its window.
const ICON_MESSAGE: u32 = WM_APP + 1;

thread_local! {
    static CHOICES: RefCell<VecDeque<TrayChoice>> = const { RefCell::new(VecDeque::new()) };
    static STATE: RefCell<TrayState> = RefCell::new(TrayState::default());
}

pub struct Tray {
    window: HWND,
    icon: NOTIFYICONDATAW,
}

impl Tray {
    /// Adds the icon, with `tip` shown when the pointer rests on it.
    pub fn new(tip: &str) -> Result<Self, PlatformError> {
        // SAFETY: the class, window and icon structures are filled from
        // live locals and constants; failures are checked.
        unsafe {
            let instance = GetModuleHandleW(None).map_err(|e| failed("finding the program", e))?;
            let class = WNDCLASSW {
                lpfnWndProc: Some(window_proc),
                hInstance: instance.into(),
                lpszClassName: w!("JuglarTray"),
                ..Default::default()
            };
            // Registering twice (a second tray in one run) fails harmlessly.
            RegisterClassW(&class);
            let window = CreateWindowExW(
                WS_EX_TOOLWINDOW,
                w!("JuglarTray"),
                w!("Juglar"),
                WS_OVERLAPPED,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                None,
                None,
                Some(instance.into()),
                None,
            )
            .map_err(|e| failed("creating the tray's window", e))?;
            let mut icon = NOTIFYICONDATAW {
                cbSize: size_of::<NOTIFYICONDATAW>() as u32,
                hWnd: window,
                uID: 1,
                uFlags: NIF_ICON | NIF_MESSAGE | NIF_TIP,
                uCallbackMessage: ICON_MESSAGE,
                hIcon: logo(instance.into()),
                ..Default::default()
            };
            for (slot, unit) in icon.szTip.iter_mut().zip(tip.encode_utf16().take(127)) {
                *slot = unit;
            }
            if !Shell_NotifyIconW(NIM_ADD, &icon).as_bool() {
                let _ = DestroyWindow(window);
                return Err(PlatformError::new("adding the tray icon failed"));
            }
            icon.uFlags = Default::default();
            Ok(Self { window, icon })
        }
    }

    /// Updates what the menu shows.
    pub fn set_state(&self, state: TrayState) {
        STATE.with(|s| *s.borrow_mut() = state);
    }

    /// The next choice made from the menu, if any.
    pub fn take(&self) -> Option<TrayChoice> {
        CHOICES.with(|c| c.borrow_mut().pop_front())
    }
}

impl Drop for Tray {
    fn drop(&mut self) {
        // SAFETY: the icon and window were created by `new`.
        unsafe {
            let _ = Shell_NotifyIconW(NIM_DELETE, &self.icon);
            let _ = DestroyWindow(self.window);
        }
    }
}

/// Juglar's logo at the tray's size, from the icon build.rs puts in the
/// program (resource 1); Windows' generic icon if it has none.
///
/// # Safety
/// `instance` must be this program's module.
unsafe fn logo(instance: HINSTANCE) -> HICON {
    // SAFETY: resource 1 is looked up in this program; LR_SHARED leaves
    // the icon to Windows, so it is never destroyed by hand.
    unsafe {
        let (width, height) = (GetSystemMetrics(SM_CXSMICON), GetSystemMetrics(SM_CYSMICON));
        match LoadImageW(Some(instance), PCWSTR(1 as *const u16), IMAGE_ICON, width, height, LR_SHARED) {
            Ok(icon) => HICON(icon.0),
            Err(_) => LoadIconW(None, IDI_APPLICATION).unwrap_or_default(),
        }
    }
}

fn failed(what: &str, e: windows::core::Error) -> PlatformError {
    PlatformError::new(format!("{what}: {e}"))
}

/// Menu item numbers; 0 means nothing was chosen. Modes and intensities
/// count up from their own hundreds.
fn choice(item: usize) -> Option<TrayChoice> {
    match item {
        1 => Some(TrayChoice::NextScene),
        2 => Some(TrayChoice::KeepScene),
        4 => Some(TrayChoice::Window),
        5 => Some(TrayChoice::Quit),
        100..200 => Some(TrayChoice::Mode(item - 100)),
        200..300 => Some(TrayChoice::Intensity(item - 200)),
        _ => None,
    }
}

unsafe extern "system" fn window_proc(window: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if message == ICON_MESSAGE {
        let event = (lparam.0 & 0xffff) as u32;
        if event == WM_RBUTTONUP || event == WM_LBUTTONUP {
            // SAFETY: called on the window's own thread with its handle.
            unsafe { show_menu(window) };
        }
        return LRESULT(0);
    }
    // SAFETY: the default handling for everything else.
    unsafe { DefWindowProcW(window, message, wparam, lparam) }
}

unsafe fn show_menu(window: HWND) {
    let state = STATE.with(|s| s.borrow().clone());
    let wide = |text: &str| -> Vec<u16> { text.encode_utf16().chain([0]).collect() };
    let scene = wide(&state.scene);
    let modes: Vec<Vec<u16>> = state.modes.iter().map(|m| wide(m)).collect();
    let intensities: Vec<Vec<u16>> = state.intensities.iter().map(|i| wide(i)).collect();
    let check = |on: bool| if on { MF_CHECKED } else { MF_UNCHECKED };
    // SAFETY: the menus are created, shown and destroyed here (a submenu
    // goes with its menu); strings are live, nul-terminated locals.
    unsafe {
        let Ok(menu) = CreatePopupMenu() else { return };
        if !state.scene.is_empty() {
            let _ = AppendMenuW(menu, MF_STRING | MF_DISABLED, 0, PCWSTR(scene.as_ptr()));
        }
        let _ = AppendMenuW(menu, MF_STRING, 1, w!("Next scene"));
        let _ = AppendMenuW(menu, MF_STRING | check(state.keeping), 2, w!("Keep this scene"));
        let _ = AppendMenuW(menu, MF_SEPARATOR, 0, PCWSTR::null());
        for (n, mode) in modes.iter().enumerate() {
            let _ = AppendMenuW(menu, MF_STRING | check(n == state.mode), 100 + n, PCWSTR(mode.as_ptr()));
        }
        if let Ok(submenu) = CreatePopupMenu() {
            for (n, intensity) in intensities.iter().enumerate() {
                let _ = AppendMenuW(submenu, MF_STRING | check(n == state.intensity), 200 + n, PCWSTR(intensity.as_ptr()));
            }
            let _ = AppendMenuW(menu, MF_SEPARATOR, 0, PCWSTR::null());
            let _ = AppendMenuW(menu, MF_STRING | MF_POPUP, submenu.0 as usize, w!("Intensity"));
        }
        let _ = AppendMenuW(menu, MF_SEPARATOR, 0, PCWSTR::null());
        let _ = AppendMenuW(menu, MF_STRING, 4, w!("Show in a window"));
        let _ = AppendMenuW(menu, MF_STRING, 5, w!("Quit Juglar"));
        let mut at = POINT::default();
        let _ = GetCursorPos(&mut at);
        // Without this, the menu wouldn't close when clicking elsewhere.
        let _ = SetForegroundWindow(window);
        let chosen = TrackPopupMenu(menu, TPM_RETURNCMD | TPM_RIGHTBUTTON, at.x, at.y, Some(0), window, None).0 as usize;
        let _ = PostMessageW(Some(window), WM_NULL, WPARAM(0), LPARAM(0));
        let _ = DestroyMenu(menu);
        if let Some(choice) = choice(chosen) {
            CHOICES.with(|c| c.borrow_mut().push_back(choice));
        }
    }
}
