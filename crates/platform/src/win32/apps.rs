//! The programs that make sound, as the volume mixer lists them, and the
//! process to hear when one of them is chosen.
//!
//! The mixer's sessions belong to processes; a browser plays from a helper
//! process its main one started, so capture is opened on the oldest
//! ancestor with the same file name, and takes in every process under it
//! (`wasapi.rs`).

use std::collections::HashMap;

use windows::Win32::Foundation::{CloseHandle, HANDLE, S_OK, STILL_ACTIVE};
use windows::Win32::Media::Audio::{
    AudioSessionStateActive, IAudioSessionControl2, IAudioSessionManager2, IMMDeviceEnumerator, MMDeviceEnumerator,
    eConsole, eRender,
};
use windows::Win32::Storage::FileSystem::{GetFileVersionInfoSizeW, GetFileVersionInfoW, VerQueryValueW};
use windows::Win32::System::Com::{CLSCTX_ALL, CoCreateInstance};
use windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW, TH32CS_SNAPPROCESS,
};
use windows::Win32::System::Threading::{
    GetCurrentProcessId, GetExitCodeProcess, OpenProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
    QueryFullProcessImageNameW,
};
use windows::core::{Interface, PCWSTR, PWSTR, w};

use crate::AudioApp;

/// A process: its id, its parent's, and its file's name.
struct Process {
    pid: u32,
    parent: u32,
    exe: String,
}

/// Every process running, from a snapshot.
fn processes() -> Vec<Process> {
    let mut all = Vec::new();
    // SAFETY: the snapshot is walked with a correctly sized entry and closed.
    unsafe {
        let Ok(snapshot) = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) else { return all };
        let mut entry = PROCESSENTRY32W { dwSize: size_of::<PROCESSENTRY32W>() as u32, ..Default::default() };
        let mut more = Process32FirstW(snapshot, &mut entry).is_ok();
        while more {
            let len = entry.szExeFile.iter().position(|&c| c == 0).unwrap_or(entry.szExeFile.len());
            all.push(Process {
                pid: entry.th32ProcessID,
                parent: entry.th32ParentProcessID,
                exe: String::from_utf16_lossy(&entry.szExeFile[..len]),
            });
            more = Process32NextW(snapshot, &mut entry).is_ok();
        }
        let _ = CloseHandle(snapshot);
    }
    all
}

/// The oldest ancestor of `pid` with the same file name, by the snapshot.
fn root(all: &[Process], pid: u32) -> u32 {
    let by_pid: HashMap<u32, &Process> = all.iter().map(|p| (p.pid, p)).collect();
    let mut at = pid;
    // Bounded: parent ids can be reused, and so form loops.
    for _ in 0..64 {
        let (Some(me), Some(parent)) = (by_pid.get(&at), by_pid.get(&at).and_then(|p| by_pid.get(&p.parent))) else {
            break;
        };
        if !parent.exe.eq_ignore_ascii_case(&me.exe) || parent.pid == at {
            break;
        }
        at = parent.pid;
    }
    at
}

/// The process to hear for the program whose file is `exe`: the root of
/// the one playing, else of any that is running.
pub(crate) fn app_root(exe: &str) -> Option<u32> {
    let all = processes();
    let sessions = sessions();
    let playing = sessions.iter().filter(|s| s.exe.eq_ignore_ascii_case(exe)).max_by_key(|s| s.playing).map(|s| s.pid);
    let any = all.iter().find(|p| p.exe.eq_ignore_ascii_case(exe)).map(|p| p.pid);
    playing.or(any).map(|pid| root(&all, pid))
}

/// Whether process `pid` is still running.
pub(crate) fn running(pid: u32) -> bool {
    // SAFETY: the handle is opened only to read the exit code, then closed.
    unsafe {
        let Ok(handle) = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) else { return false };
        let mut code = 0u32;
        let alive = GetExitCodeProcess(handle, &mut code).is_ok() && code == STILL_ACTIVE.0 as u32;
        let _ = CloseHandle(handle);
        alive
    }
}

/// One of the mixer's sessions.
struct Session {
    pid: u32,
    exe: String,
    path: String,
    playing: bool,
}

/// The default output device's sessions, but the system sounds' and any
/// Juglar's (its capture shows as one). COM must be initialized on the
/// calling thread.
fn sessions() -> Vec<Session> {
    let mut found = Vec::new();
    // SAFETY: COM calls on interfaces just obtained; the strings and
    // handles are freed or closed before returning.
    unsafe {
        let Ok(enumerator) = CoCreateInstance::<_, IMMDeviceEnumerator>(&MMDeviceEnumerator, None, CLSCTX_ALL) else {
            return found;
        };
        let Ok(device) = enumerator.GetDefaultAudioEndpoint(eRender, eConsole) else { return found };
        let Ok(manager) = device.Activate::<IAudioSessionManager2>(CLSCTX_ALL, None) else { return found };
        let Ok(list) = manager.GetSessionEnumerator() else { return found };
        let me = GetCurrentProcessId();
        for n in 0..list.GetCount().unwrap_or(0) {
            let Ok(control) = list.GetSession(n) else { continue };
            let Ok(control) = control.cast::<IAudioSessionControl2>() else { continue };
            if control.IsSystemSoundsSession() == S_OK {
                continue;
            }
            let Ok(pid) = control.GetProcessId() else { continue };
            if pid == 0 || pid == me {
                continue;
            }
            let Some(path) = image_path(pid) else { continue };
            let exe = path.rsplit(['\\', '/']).next().unwrap_or(&path).to_owned();
            if exe.eq_ignore_ascii_case("juglar.exe") {
                continue;
            }
            let playing = control.GetState().is_ok_and(|state| state == AudioSessionStateActive);
            found.push(Session { pid, exe, path, playing });
        }
    }
    found
}

/// The programs with sound open, each once, those playing first, then by
/// name. COM must be initialized on the calling thread.
pub(crate) fn audio_apps(names: &mut HashMap<String, String>) -> Vec<AudioApp> {
    let mut apps: Vec<AudioApp> = Vec::new();
    for session in sessions() {
        if let Some(app) = apps.iter_mut().find(|a| a.exe.eq_ignore_ascii_case(&session.exe)) {
            app.playing |= session.playing;
            continue;
        }
        let name = names.entry(session.path.clone()).or_insert_with(|| describe(&session.path, &session.exe)).clone();
        apps.push(AudioApp { exe: session.exe, name, playing: session.playing });
    }
    apps.sort_by(|a, b| b.playing.cmp(&a.playing).then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase())));
    apps
}

/// The full path of process `pid`'s file.
fn image_path(pid: u32) -> Option<String> {
    // SAFETY: the buffer's length is passed and updated by the call; the
    // handle is closed.
    unsafe {
        let handle: HANDLE = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
        let mut buffer = [0u16; 1024];
        let mut len = buffer.len() as u32;
        let ok = QueryFullProcessImageNameW(handle, PROCESS_NAME_WIN32, PWSTR(buffer.as_mut_ptr()), &mut len).is_ok();
        let _ = CloseHandle(handle);
        ok.then(|| String::from_utf16_lossy(&buffer[..len as usize]))
    }
}

/// A program's name for people: its file's description ("Google Chrome"),
/// or its file name without `.exe`.
fn describe(path: &str, exe: &str) -> String {
    let fallback = || exe.strip_suffix(".exe").or_else(|| exe.strip_suffix(".EXE")).unwrap_or(exe).to_owned();
    let wide: Vec<u16> = path.encode_utf16().chain([0]).collect();
    // SAFETY: the version block is read into a buffer of the size Windows
    // gave, and the values queried point inside it.
    unsafe {
        let size = GetFileVersionInfoSizeW(PCWSTR(wide.as_ptr()), None);
        if size == 0 {
            return fallback();
        }
        let mut block = vec![0u8; size as usize];
        if GetFileVersionInfoW(PCWSTR(wide.as_ptr()), None, size, block.as_mut_ptr().cast()).is_err() {
            return fallback();
        }
        let mut value = std::ptr::null_mut();
        let mut len = 0u32;
        // The first language and code page the file has strings in.
        if !VerQueryValueW(block.as_ptr().cast(), w!("\\VarFileInfo\\Translation"), &mut value, &mut len).as_bool()
            || len < 4
        {
            return fallback();
        }
        let pair = value.cast::<u16>();
        let key = format!("\\StringFileInfo\\{:04x}{:04x}\\FileDescription", *pair, *pair.add(1));
        let key: Vec<u16> = key.encode_utf16().chain([0]).collect();
        if !VerQueryValueW(block.as_ptr().cast(), PCWSTR(key.as_ptr()), &mut value, &mut len).as_bool() || len == 0 {
            return fallback();
        }
        let text = std::slice::from_raw_parts(value.cast::<u16>(), len as usize);
        let end = text.iter().position(|&c| c == 0).unwrap_or(text.len());
        let name = String::from_utf16_lossy(&text[..end]).trim().to_owned();
        if name.is_empty() || name.len() > 40 { fallback() } else { name }
    }
}
