//! An icon in the notification area, with a menu, and keys that work from
//! anywhere (`HOTKEYS`) while Juglar is the wallpaper, since its window
//! then lies under the desktop icons and never gets the keyboard.
//!
//! The icon talks to a hidden window of its own, created on the thread that
//! runs the app's event loop, so its messages arrive there with everyone
//! else's; so do the keys, registered to that window. Choices wait in a
//! queue until the app asks. The menu follows Windows' dark mode, and the
//! icon comes back when Explorer restarts.

use std::cell::{Cell, RefCell};
use std::collections::VecDeque;

use windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, POINT, WPARAM};
use windows::Win32::System::LibraryLoader::{GetModuleHandleW, GetProcAddress, LoadLibraryW};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    MOD_ALT, MOD_CONTROL, MOD_NOREPEAT, RegisterHotKey, UnregisterHotKey, VIRTUAL_KEY, VK_H, VK_I, VK_K, VK_LEFT, VK_M,
    VK_RIGHT, VK_W,
};
use windows::Win32::UI::Shell::{
    NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NIM_MODIFY, NOTIFYICONDATAW, Shell_NotifyIconW,
};
use windows::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CW_USEDEFAULT, CreatePopupMenu, CreateWindowExW, DefWindowProcW, DestroyMenu, DestroyWindow,
    GetCursorPos, GetSystemMetrics, HICON, HMENU, IDI_APPLICATION, IMAGE_ICON, LR_SHARED, LoadIconW, LoadImageW,
    MF_CHECKED, MF_DISABLED, MF_POPUP, MF_SEPARATOR, MF_STRING, MF_UNCHECKED, MENU_ITEM_FLAGS, PostMessageW,
    RegisterClassW, RegisterWindowMessageW, SM_CXSMICON, SM_CYSMICON, SetForegroundWindow, SetMenuDefaultItem,
    TPM_RETURNCMD, TPM_RIGHTBUTTON, TrackPopupMenu, WM_APP, WM_HOTKEY, WM_LBUTTONUP, WM_NULL, WM_RBUTTONUP, WNDCLASSW,
    WS_EX_TOOLWINDOW, WS_OVERLAPPED,
};
use windows::core::{PCSTR, PCWSTR, w};

use crate::PlatformError;

/// What the listener chose from the menu, or with a key.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrayChoice {
    PreviousScene,
    NextScene,
    /// Keep the scene showing, or let go.
    KeepScene,
    /// One of `TrayState::modes`, by its place in the list.
    Mode(usize),
    /// One of `TrayState::intensities`, likewise.
    Intensity(usize),
    /// One of `TrayState::displays`, likewise.
    Displays(usize),
    /// One of `TrayState::sources`, likewise: what to listen to.
    Listen(usize),
    /// One of `TrayState::shows`, likewise: a window, fullscreen, the
    /// wallpaper.
    Show(usize),
    /// Show each new song's title, or don't.
    Intros,
    /// Start with Windows, or don't.
    Startup,
    NextMode,
    NextIntensity,
    /// Show or hide the keys.
    Help,
    /// Leave wallpaper mode for a window.
    Window,
    /// Open the settings.
    Settings,
    /// Bring Juglar's window to the front (a click on the icon, or Juglar
    /// started a second time).
    Open,
    Quit,
}

/// What the menu shows: the song and the scene, and the choices with the
/// current one checked.
#[derive(Clone, Debug, Default)]
pub struct TrayState {
    /// The song playing, as a title at the top of the menu; may be empty.
    pub song: String,
    /// The scene showing, likewise.
    pub scene: String,
    pub keeping: bool,
    pub modes: Vec<String>,
    pub mode: usize,
    pub intensities: Vec<String>,
    pub intensity: usize,
    /// How to use several displays; empty with only one.
    pub displays: Vec<String>,
    pub display: usize,
    pub sources: Vec<String>,
    pub source: usize,
    pub shows: Vec<String>,
    pub show: usize,
    pub intros: bool,
    pub startup: bool,
}

/// The keys that work from anywhere while Juglar is the wallpaper, with
/// what each does. Ctrl+Alt with keys no common layout puts under AltGr
/// (which Windows reports as Ctrl+Alt): no digits, no E or Q.
pub const HOTKEYS: [(&str, TrayChoice); 7] = [
    ("Ctrl+Alt+\u{2190}", TrayChoice::PreviousScene),
    ("Ctrl+Alt+\u{2192}", TrayChoice::NextScene),
    ("Ctrl+Alt+K", TrayChoice::KeepScene),
    ("Ctrl+Alt+M", TrayChoice::NextMode),
    ("Ctrl+Alt+I", TrayChoice::NextIntensity),
    ("Ctrl+Alt+H", TrayChoice::Help),
    ("Ctrl+Alt+W", TrayChoice::Window),
];

/// Each of `HOTKEYS`' virtual-key codes, in the same order.
const HOTKEY_CODES: [VIRTUAL_KEY; 7] = [VK_LEFT, VK_RIGHT, VK_K, VK_M, VK_I, VK_H, VK_W];

/// The icon's messages to its window.
const ICON_MESSAGE: u32 = WM_APP + 1;
/// Another Juglar, started while this one runs, asks it to show itself.
pub(crate) const SHOW_MESSAGE: u32 = WM_APP + 2;
/// The tray window's class, by which a second Juglar finds the first.
pub(crate) const CLASS: PCWSTR = w!("JuglarTray");

thread_local! {
    static CHOICES: RefCell<VecDeque<TrayChoice>> = const { RefCell::new(VecDeque::new()) };
    static STATE: RefCell<TrayState> = RefCell::new(TrayState::default());
    /// The icon as added, to add it again when Explorer restarts.
    static ICON: RefCell<Option<NOTIFYICONDATAW>> = const { RefCell::new(None) };
    /// The message Explorer broadcasts when it starts (again).
    static TASKBAR_CREATED: Cell<u32> = const { Cell::new(0) };
    /// Whether a left click opens the menu (as the wallpaper, there is no
    /// window to bring forward) rather than the window.
    static MENU_ON_CLICK: Cell<bool> = const { Cell::new(false) };
}

pub struct Tray {
    window: HWND,
    hotkeys: bool,
}

impl Tray {
    /// Adds the icon, with `tip` shown when the pointer rests on it.
    pub fn new(tip: &str) -> Result<Self, PlatformError> {
        dark_menus();
        // SAFETY: the class, window and icon structures are filled from
        // live locals and constants; failures are checked.
        unsafe {
            let instance = GetModuleHandleW(None).map_err(|e| failed("finding the program", e))?;
            let class = WNDCLASSW {
                lpfnWndProc: Some(window_proc),
                hInstance: instance.into(),
                lpszClassName: CLASS,
                ..Default::default()
            };
            // Registering twice (a second tray in one run) fails harmlessly.
            RegisterClassW(&class);
            let window = CreateWindowExW(
                WS_EX_TOOLWINDOW,
                CLASS,
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
            TASKBAR_CREATED.set(RegisterWindowMessageW(w!("TaskbarCreated")));
            let mut icon = NOTIFYICONDATAW {
                cbSize: size_of::<NOTIFYICONDATAW>() as u32,
                hWnd: window,
                uID: 1,
                uFlags: NIF_ICON | NIF_MESSAGE | NIF_TIP,
                uCallbackMessage: ICON_MESSAGE,
                hIcon: logo(instance.into()),
                ..Default::default()
            };
            write_tip(&mut icon, tip);
            if !Shell_NotifyIconW(NIM_ADD, &icon).as_bool() {
                let _ = DestroyWindow(window);
                return Err(PlatformError::new("adding the tray icon failed"));
            }
            ICON.with(|i| *i.borrow_mut() = Some(icon));
            Ok(Self { window, hotkeys: false })
        }
    }

    /// Updates what the menu shows.
    pub fn set_state(&self, state: TrayState) {
        STATE.with(|s| *s.borrow_mut() = state);
    }

    /// Changes the text shown when the pointer rests on the icon.
    pub fn set_tip(&self, tip: &str) {
        ICON.with(|i| {
            if let Some(icon) = i.borrow_mut().as_mut() {
                write_tip(icon, tip);
                // SAFETY: modifies the icon `new` added, by its window and id.
                unsafe {
                    let _ = Shell_NotifyIconW(NIM_MODIFY, icon);
                }
            }
        });
    }

    /// Turns the keys that work from anywhere on (as the wallpaper) or off,
    /// and with them whether a left click opens the menu.
    pub fn set_hotkeys(&mut self, on: bool) {
        MENU_ON_CLICK.set(on);
        if on == self.hotkeys {
            return;
        }
        self.hotkeys = on;
        // SAFETY: the keys are registered to and removed from this tray's
        // own window.
        unsafe {
            for (n, (&code, (name, _))) in HOTKEY_CODES.iter().zip(&HOTKEYS).enumerate() {
                let id = n as i32 + 1;
                if !on {
                    let _ = UnregisterHotKey(Some(self.window), id);
                // A key another program holds is left out; the menu still works.
                } else if let Err(e) = RegisterHotKey(Some(self.window), id, MOD_CONTROL | MOD_ALT | MOD_NOREPEAT, code.0.into())
                {
                    eprintln!("juglar: {name} is taken by another program ({e})");
                }
            }
        }
    }

    /// The next choice made from the menu, if any.
    pub fn take(&self) -> Option<TrayChoice> {
        CHOICES.with(|c| c.borrow_mut().pop_front())
    }
}

impl Drop for Tray {
    fn drop(&mut self) {
        self.set_hotkeys(false);
        // SAFETY: the icon and window were created by `new`.
        unsafe {
            if let Some(icon) = ICON.with(|i| i.borrow_mut().take()) {
                let _ = Shell_NotifyIconW(NIM_DELETE, &icon);
            }
            let _ = DestroyWindow(self.window);
        }
    }
}

fn write_tip(icon: &mut NOTIFYICONDATAW, tip: &str) {
    icon.szTip = [0; 128];
    for (slot, unit) in icon.szTip.iter_mut().zip(tip.encode_utf16().take(127)) {
        *slot = unit;
    }
}

/// Lets the menus follow Windows' dark mode, through uxtheme's unnamed
/// functions (by number, as Explorer and Notepad do): Windows draws
/// popup menus light otherwise. Without them, menus stay light.
fn dark_menus() {
    type SetPreferredAppMode = unsafe extern "system" fn(i32) -> i32;
    type FlushMenuThemes = unsafe extern "system" fn();
    // SAFETY: the functions are looked up by their documented-by-use
    // numbers in Windows 10 1903 and later and called with their known
    // signatures; on older Windows the lookup fails and nothing is called.
    unsafe {
        let Ok(uxtheme) = LoadLibraryW(w!("uxtheme.dll")) else { return };
        // 1: follow the system's app mode.
        if let Some(set) = GetProcAddress(uxtheme, PCSTR(135 as *const u8)) {
            std::mem::transmute::<unsafe extern "system" fn() -> isize, SetPreferredAppMode>(set)(1);
        }
        if let Some(flush) = GetProcAddress(uxtheme, PCSTR(136 as *const u8)) {
            std::mem::transmute::<unsafe extern "system" fn() -> isize, FlushMenuThemes>(flush)();
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

/// Menu item numbers; 0 means nothing was chosen. Each list counts up from
/// its own hundred.
mod item {
    pub const NEXT: usize = 1;
    pub const KEEP: usize = 2;
    pub const QUIT: usize = 5;
    pub const PREVIOUS: usize = 6;
    pub const INTROS: usize = 7;
    pub const STARTUP: usize = 8;
    pub const SETTINGS: usize = 9;
    pub const MODES: usize = 100;
    pub const INTENSITIES: usize = 200;
    pub const DISPLAYS: usize = 300;
    pub const SOURCES: usize = 400;
    pub const SHOWS: usize = 500;
}

fn choice(chosen: usize) -> Option<TrayChoice> {
    match chosen {
        item::NEXT => Some(TrayChoice::NextScene),
        item::PREVIOUS => Some(TrayChoice::PreviousScene),
        item::KEEP => Some(TrayChoice::KeepScene),
        item::INTROS => Some(TrayChoice::Intros),
        item::STARTUP => Some(TrayChoice::Startup),
        item::SETTINGS => Some(TrayChoice::Settings),
        item::QUIT => Some(TrayChoice::Quit),
        100..200 => Some(TrayChoice::Mode(chosen - item::MODES)),
        200..300 => Some(TrayChoice::Intensity(chosen - item::INTENSITIES)),
        300..400 => Some(TrayChoice::Displays(chosen - item::DISPLAYS)),
        400..500 => Some(TrayChoice::Listen(chosen - item::SOURCES)),
        500..600 => Some(TrayChoice::Show(chosen - item::SHOWS)),
        _ => None,
    }
}

unsafe extern "system" fn window_proc(window: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    let push = |choice| CHOICES.with(|c| c.borrow_mut().push_back(choice));
    match message {
        WM_HOTKEY => {
            if let Some((_, choice)) = HOTKEYS.get(wparam.0.wrapping_sub(1)) {
                push(*choice);
            }
            LRESULT(0)
        }
        ICON_MESSAGE => {
            let event = (lparam.0 & 0xffff) as u32;
            if event == WM_RBUTTONUP || (event == WM_LBUTTONUP && MENU_ON_CLICK.get()) {
                // SAFETY: called on the window's own thread with its handle.
                unsafe { show_menu(window) };
            } else if event == WM_LBUTTONUP {
                push(TrayChoice::Open);
            }
            LRESULT(0)
        }
        SHOW_MESSAGE => {
            push(TrayChoice::Open);
            LRESULT(0)
        }
        _ if message != 0 && message == TASKBAR_CREATED.get() => {
            // Explorer restarted, and the icon went with it.
            ICON.with(|i| {
                if let Some(icon) = i.borrow().as_ref() {
                    // SAFETY: the same icon data `new` added.
                    unsafe {
                        let _ = Shell_NotifyIconW(NIM_ADD, icon);
                    }
                }
            });
            LRESULT(0)
        }
        // SAFETY: the default handling for everything else.
        _ => unsafe { DefWindowProcW(window, message, wparam, lparam) },
    }
}

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain([0]).collect()
}

fn check(on: bool) -> MENU_ITEM_FLAGS {
    if on { MF_CHECKED } else { MF_UNCHECKED }
}

/// Adds a submenu called `title` listing `names` (numbered from `first`),
/// with `chosen` checked; nothing for an empty list.
///
/// # Safety
/// `menu` must be a live menu.
unsafe fn submenu(menu: HMENU, title: &str, names: &[String], chosen: usize, first: usize) {
    if names.is_empty() {
        return;
    }
    // SAFETY: the submenu goes with `menu` when it is destroyed; strings
    // are live, nul-terminated locals for each call.
    unsafe {
        let Ok(sub) = CreatePopupMenu() else { return };
        for (n, name) in names.iter().enumerate() {
            let name = wide(name);
            let _ = AppendMenuW(sub, MF_STRING | check(n == chosen), first + n, PCWSTR(name.as_ptr()));
        }
        let title = wide(title);
        let _ = AppendMenuW(menu, MF_STRING | MF_POPUP, sub.0 as usize, PCWSTR(title.as_ptr()));
    }
}

unsafe fn show_menu(window: HWND) {
    let state = STATE.with(|s| s.borrow().clone());
    // SAFETY: the menu is created, shown and destroyed here (submenus go
    // with it); strings are live, nul-terminated locals.
    unsafe {
        let Ok(menu) = CreatePopupMenu() else { return };
        let mut headed = false;
        for title in [&state.song, &state.scene] {
            if !title.is_empty() {
                let title = wide(title);
                let _ = AppendMenuW(menu, MF_STRING | MF_DISABLED, 0, PCWSTR(title.as_ptr()));
                headed = true;
            }
        }
        if headed {
            let _ = AppendMenuW(menu, MF_SEPARATOR, 0, PCWSTR::null());
        }
        let _ = AppendMenuW(menu, MF_STRING, item::PREVIOUS, w!("Previous scene"));
        let _ = AppendMenuW(menu, MF_STRING, item::NEXT, w!("Next scene"));
        let _ = AppendMenuW(menu, MF_STRING | check(state.keeping), item::KEEP, w!("Keep this scene"));
        let _ = AppendMenuW(menu, MF_SEPARATOR, 0, PCWSTR::null());
        submenu(menu, "Listen to", &state.sources, state.source, item::SOURCES);
        submenu(menu, "Show as", &state.shows, state.show, item::SHOWS);
        submenu(menu, "Displays", &state.displays, state.display, item::DISPLAYS);
        submenu(menu, "Mode", &state.modes, state.mode, item::MODES);
        submenu(menu, "Intensity", &state.intensities, state.intensity, item::INTENSITIES);
        let _ = AppendMenuW(menu, MF_SEPARATOR, 0, PCWSTR::null());
        let _ = AppendMenuW(menu, MF_STRING | check(state.intros), item::INTROS, w!("Song titles"));
        let _ = AppendMenuW(menu, MF_STRING | check(state.startup), item::STARTUP, w!("Start with Windows"));
        let _ = AppendMenuW(menu, MF_SEPARATOR, 0, PCWSTR::null());
        let _ = AppendMenuW(menu, MF_STRING, item::SETTINGS, w!("Settings\u{2026}"));
        let _ = AppendMenuW(menu, MF_STRING, item::QUIT, w!("Quit Juglar"));
        let _ = SetMenuDefaultItem(menu, item::SETTINGS as u32, 0);
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

/// Where the mouse pointer is on the screen, in physical pixels.
pub fn pointer() -> Option<(i32, i32)> {
    let mut at = POINT::default();
    // SAFETY: writes one POINT to a live local.
    unsafe { GetCursorPos(&mut at) }.ok().map(|()| (at.x, at.y))
}
