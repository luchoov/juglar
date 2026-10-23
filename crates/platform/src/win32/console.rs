//! Where `println!` and `eprintln!` go in the release build, which is a
//! Windows program rather than a console one, so that starting it opens no
//! console window. Rust's standard output asks Windows for the handle on
//! every write, so pointing the handle somewhere else redirects them all.

use std::os::windows::io::IntoRawHandle;

use windows::Win32::Foundation::HANDLE;
use windows::Win32::System::Console::{
    ATTACH_PARENT_PROCESS, AllocConsole, AttachConsole, GetStdHandle, STD_ERROR_HANDLE, STD_OUTPUT_HANDLE, SetStdHandle,
};

/// Whether standard output already leads somewhere: a console (the debug
/// build is a console program) or a file or pipe it was redirected to.
pub fn output_attached() -> bool {
    // SAFETY: GetStdHandle only reads the process's handle table.
    unsafe { GetStdHandle(STD_OUTPUT_HANDLE) }.is_ok_and(|handle| !handle.is_invalid() && !handle.0.is_null())
}

/// Shows the messages in a console: the one Juglar was started from, or
/// a new one.
pub fn open_console() {
    // SAFETY: both only change this process's console; a failed attach
    // (not started from a console) leaves nothing to undo.
    unsafe {
        if AttachConsole(ATTACH_PARENT_PROCESS).is_err() {
            let _ = AllocConsole();
        }
    }
}

/// Sends standard output and standard error into `file`, for good.
pub fn send_output_to(file: std::fs::File) {
    let handle = HANDLE(file.into_raw_handle());
    // SAFETY: the handle is an open file that is never closed (it was
    // taken out of the `File`), so it stays valid for every later write.
    unsafe {
        let _ = SetStdHandle(STD_OUTPUT_HANDLE, handle);
        let _ = SetStdHandle(STD_ERROR_HANDLE, handle);
    }
}

/// Tells the user why Juglar can't go on, in a message box: without a
/// console, the log is the only other place it would appear.
pub fn alert(text: &str) {
    use windows::Win32::UI::WindowsAndMessaging::{MB_ICONERROR, MB_OK, MessageBoxW};
    use windows::core::{HSTRING, w};
    // SAFETY: both strings outlive the call, and there is no owner window.
    unsafe {
        MessageBoxW(None, &HSTRING::from(text), w!("Juglar"), MB_OK | MB_ICONERROR);
    }
}
