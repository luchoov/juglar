//! System audio capture through WASAPI loopback.
//!
//! Loopback opens the default *render* device for capture, so we receive
//! exactly what the speakers play, whatever app produces it. The capture
//! thread sleeps on an event that the audio engine signals once per period
//! (about 10 ms), so it costs nothing between packets.
//!
//! COM objects are not `Send`: everything WASAPI is created, used and released
//! on the capture thread. The handle on the calling side only holds the stop
//! flag and the join handle.

use std::ptr;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::thread::{self, JoinHandle};

use windows::Win32::Foundation::{CloseHandle, HANDLE, WAIT_OBJECT_0};
use windows::Win32::Media::Audio::{
    AUDCLNT_BUFFERFLAGS_SILENT, AUDCLNT_SHAREMODE_SHARED, AUDCLNT_STREAMFLAGS_EVENTCALLBACK,
    AUDCLNT_STREAMFLAGS_LOOPBACK, IAudioCaptureClient, IAudioClient, IMMDeviceEnumerator,
    MMDeviceEnumerator, WAVEFORMATEX, WAVEFORMATEXTENSIBLE, eConsole, eRender,
};
use windows::Win32::Media::KernelStreaming::WAVE_FORMAT_EXTENSIBLE;
use windows::Win32::Media::Multimedia::{KSDATAFORMAT_SUBTYPE_IEEE_FLOAT, WAVE_FORMAT_IEEE_FLOAT};
use windows::Win32::System::Com::{
    CLSCTX_ALL, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx, CoTaskMemFree,
    CoUninitialize,
};
use windows::Win32::System::Threading::{CreateEventW, WaitForSingleObject};
use windows::core::PCWSTR;

use crate::{AudioCapture, PlatformError, SampleCallback, StreamInfo};

/// Engine buffer size in 100 ns units. Generous on purpose: the engine keeps
/// up to 100 ms for us if the capture thread is ever descheduled.
const BUFFER_DURATION_HNS: i64 = 1_000_000;

/// How long to wait for a packet before checking the stop flag. Loopback
/// sends no packets at all while nothing is playing, so this also bounds how
/// long stopping can take.
const WAIT_TIMEOUT_MS: u32 = 100;

pub struct WasapiLoopback {
    info: StreamInfo,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl AudioCapture for WasapiLoopback {
    fn start(on_samples: SampleCallback) -> Result<Self, PlatformError> {
        let stop = Arc::new(AtomicBool::new(false));
        let (ready_tx, ready_rx) = mpsc::sync_channel(1);
        let thread_stop = stop.clone();
        let thread = thread::Builder::new()
            .name("juglar-capture".into())
            .spawn(move || capture_thread(on_samples, thread_stop, ready_tx))
            .map_err(|e| PlatformError::new(format!("spawning the capture thread: {e}")))?;

        match ready_rx.recv() {
            Ok(Ok(info)) => Ok(Self { info, stop, thread: Some(thread) }),
            Ok(Err(e)) => {
                let _ = thread.join();
                Err(e)
            }
            Err(_) => {
                let _ = thread.join();
                Err(PlatformError::new("the capture thread exited before starting"))
            }
        }
    }

    fn info(&self) -> StreamInfo {
        self.info
    }
}

impl Drop for WasapiLoopback {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

type Ready = mpsc::SyncSender<Result<StreamInfo, PlatformError>>;

fn capture_thread(on_samples: SampleCallback, stop: Arc<AtomicBool>, ready: Ready) {
    // SAFETY: paired with CoUninitialize below; this thread owns its COM apartment.
    if let Err(e) = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) }.ok() {
        let _ = ready.send(Err(win_err("initializing COM", e)));
        return;
    }
    if let Err(e) = run(on_samples, &stop, &ready) {
        // Before `ready` was answered this reaches `start`; afterwards the
        // receiver is gone, so the error can only be reported here.
        if let Err(mpsc::SendError(Err(e))) = ready.send(Err(e)) {
            eprintln!("audio capture stopped: {e}");
        }
    }
    // SAFETY: every COM object from `run` has been dropped by now.
    unsafe { CoUninitialize() };
}

fn run(mut on_samples: SampleCallback, stop: &AtomicBool, ready: &Ready) -> Result<(), PlatformError> {
    // SAFETY: plain WASAPI calls on this thread's COM apartment; every pointer
    // passed is valid for the duration of the call.
    unsafe {
        let enumerator: IMMDeviceEnumerator = CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)
            .map_err(|e| win_err("creating the device enumerator", e))?;
        let device = enumerator
            .GetDefaultAudioEndpoint(eRender, eConsole)
            .map_err(|e| win_err("finding the default output device", e))?;
        let client: IAudioClient =
            device.Activate(CLSCTX_ALL, None).map_err(|e| win_err("opening the audio client", e))?;

        let format = MixFormat(client.GetMixFormat().map_err(|e| win_err("reading the mix format", e))?);
        let info = format.float_stream_info()?;

        client
            .Initialize(
                AUDCLNT_SHAREMODE_SHARED,
                AUDCLNT_STREAMFLAGS_LOOPBACK | AUDCLNT_STREAMFLAGS_EVENTCALLBACK,
                BUFFER_DURATION_HNS,
                0,
                format.0,
                None,
            )
            .map_err(|e| win_err("initializing loopback capture", e))?;

        let event = Event(
            CreateEventW(None, false, false, PCWSTR::null()).map_err(|e| win_err("creating the capture event", e))?,
        );
        client.SetEventHandle(event.0).map_err(|e| win_err("registering the capture event", e))?;
        let capture: IAudioCaptureClient =
            client.GetService().map_err(|e| win_err("opening the capture client", e))?;
        client.Start().map_err(|e| win_err("starting capture", e))?;

        let _ = ready.send(Ok(info));

        let channels = usize::from(info.channels);
        // Reused for silent packets so the hot loop never allocates after warm-up.
        let mut silence: Vec<f32> = Vec::new();

        while !stop.load(Ordering::Relaxed) {
            if WaitForSingleObject(event.0, WAIT_TIMEOUT_MS) != WAIT_OBJECT_0 {
                continue;
            }
            loop {
                let packet = capture.GetNextPacketSize().map_err(|e| win_err("polling for packets", e))?;
                if packet == 0 {
                    break;
                }
                let mut data: *mut u8 = ptr::null_mut();
                let mut frames = 0u32;
                let mut flags = 0u32;
                capture
                    .GetBuffer(&mut data, &mut frames, &mut flags, None, None)
                    .map_err(|e| win_err("reading a packet", e))?;

                let len = frames as usize * channels;
                if flags & AUDCLNT_BUFFERFLAGS_SILENT.0 as u32 != 0 || data.is_null() {
                    silence.clear();
                    silence.resize(len, 0.0);
                    on_samples(&silence, info.channels);
                } else {
                    // The mix format is 32-bit float, checked above.
                    on_samples(std::slice::from_raw_parts(data.cast::<f32>(), len), info.channels);
                }
                capture.ReleaseBuffer(frames).map_err(|e| win_err("releasing a packet", e))?;
            }
        }

        let _ = client.Stop();
        Ok(())
    }
}

/// The engine's mix format, freed with `CoTaskMemFree` on drop.
struct MixFormat(*mut WAVEFORMATEX);

impl MixFormat {
    /// Shared-mode mix formats are 32-bit float on every modern Windows; we
    /// rely on that instead of converting.
    fn float_stream_info(&self) -> Result<StreamInfo, PlatformError> {
        // SAFETY: GetMixFormat returned a valid WAVEFORMATEX, extended to
        // WAVEFORMATEXTENSIBLE when the tag says so. The structs are packed,
        // so fields are copied out instead of referenced.
        let (tag, bits, sample_rate, channels, is_float) = unsafe {
            let base = ptr::read_unaligned(self.0);
            let tag = u32::from(base.wFormatTag);
            let is_float = match tag {
                WAVE_FORMAT_IEEE_FLOAT => true,
                WAVE_FORMAT_EXTENSIBLE => {
                    let ext = ptr::read_unaligned(self.0.cast::<WAVEFORMATEXTENSIBLE>());
                    let sub_format = ext.SubFormat;
                    sub_format == KSDATAFORMAT_SUBTYPE_IEEE_FLOAT
                }
                _ => false,
            };
            (tag, base.wBitsPerSample, base.nSamplesPerSec, base.nChannels, is_float)
        };
        if !is_float || bits != 32 {
            return Err(PlatformError::new(format!(
                "unsupported mix format (tag {tag:#x}, {bits} bits); expected 32-bit float"
            )));
        }
        Ok(StreamInfo { sample_rate, channels })
    }
}

impl Drop for MixFormat {
    fn drop(&mut self) {
        // SAFETY: allocated by the audio engine with CoTaskMemAlloc.
        unsafe { CoTaskMemFree(Some(self.0 as *const _)) };
    }
}

struct Event(HANDLE);

impl Drop for Event {
    fn drop(&mut self) {
        // SAFETY: created by CreateEventW and owned only by this value.
        let _ = unsafe { CloseHandle(self.0) };
    }
}

fn win_err(context: &str, e: windows::core::Error) -> PlatformError {
    PlatformError::new(format!("{context}: {e}"))
}
