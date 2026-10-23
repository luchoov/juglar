//! What is playing, from the players themselves: Windows' media sessions
//! (the "global system media transport controls", GSMTC), which Spotify,
//! the browsers (YouTube among them) and most players fill in for the
//! volume flyout. No account or web service: the title, artist, album and
//! cover come from the program playing.
//!
//! A thread of its own asks twice a second, and every two seconds lists
//! the programs making sound (`apps.rs`). It sends only what changed, so
//! the app reads a quiet channel once a frame.

use std::collections::HashMap;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use windows::Graphics::Imaging::{
    BitmapAlphaMode, BitmapDecoder, BitmapInterpolationMode, BitmapPixelFormat, BitmapTransform, ColorManagementMode,
    ExifOrientationMode,
};
use windows::Media::Control::{
    GlobalSystemMediaTransportControlsSession as Session, GlobalSystemMediaTransportControlsSessionManager as Manager,
    GlobalSystemMediaTransportControlsSessionPlaybackStatus as Status,
};
use windows::Win32::System::Com::{COINIT_MULTITHREADED, CoInitializeEx, CoUninitialize};

use super::apps::audio_apps;
use crate::{AudioApp, Cover, Media, Track};

/// How often the players are asked.
const TICK: Duration = Duration::from_millis(500);
/// The programs making sound are listed every this many ticks.
const APPS_EVERY: u32 = 4;
/// Covers are this many pixels a side at most.
const COVER: u32 = 320;
/// What browsers say instead of a title when they won't tell it (private
/// windows, ads): no song.
const PLACEHOLDERS: [&str; 1] = ["A site is playing media"];
/// Ticks after a new song at which its cover is read: players often set
/// the cover a moment after the title, and Spotify sometimes still has the
/// last song's at first.
const COVER_TICKS: [u32; 3] = [0, 2, 5];

/// Watches the players and the programs making sound. Stops when dropped.
pub struct MediaWatch {
    news: Receiver<Media>,
    prefer: Arc<Mutex<Option<String>>>,
    stop: Sender<()>,
}

impl MediaWatch {
    pub fn start() -> Self {
        let (news_tx, news) = mpsc::channel();
        let (stop, stop_rx) = mpsc::channel();
        let prefer = Arc::new(Mutex::new(None));
        let thread_prefer = prefer.clone();
        let started = thread::Builder::new()
            .name("juglar-media".into())
            .spawn(move || watch(news_tx, stop_rx, thread_prefer));
        if let Err(e) = started {
            eprintln!("juglar: no song titles: {e}");
        }
        Self { news, prefer, stop }
    }

    /// What changed since the last call, oldest first.
    pub fn news(&self) -> Vec<Media> {
        self.news.try_iter().collect()
    }

    /// Prefers the player whose file is `exe` (the program Juglar listens
    /// to) when several have something to say.
    pub fn prefer(&self, exe: Option<&str>) {
        if let Ok(mut prefer) = self.prefer.lock() {
            *prefer = exe.map(str::to_owned);
        }
    }
}

impl Drop for MediaWatch {
    fn drop(&mut self) {
        let _ = self.stop.send(());
    }
}

fn watch(news: Sender<Media>, stop: Receiver<()>, prefer: Arc<Mutex<Option<String>>>) {
    // SAFETY: paired with CoUninitialize at the end; this thread owns its
    // apartment and every object is dropped before it ends.
    if unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) }.is_err() {
        return;
    }
    let mut manager: Option<Manager> = None;
    let mut last: Option<Track> = None;
    let mut apps: Vec<AudioApp> = Vec::new();
    let mut names = HashMap::new();
    let mut ids = 0u64;
    // Ticks since the song changed, and the cover last sent for it.
    let mut since_song = 0u32;
    let mut cover_sent: Option<u64> = None;
    let mut tick = 0u32;
    loop {
        if tick.is_multiple_of(APPS_EVERY) {
            let now = audio_apps(&mut names);
            if now != apps {
                apps = now;
                if news.send(Media::Apps(apps.clone())).is_err() {
                    break;
                }
            }
        }
        // Asked for again after a failure, every ten ticks.
        if manager.is_none() && tick.is_multiple_of(10) {
            manager = Manager::RequestAsync().and_then(|op| op.join()).ok();
        }
        let wanted = prefer.lock().ok().and_then(|p| p.clone());
        let chosen = manager.as_ref().and_then(|m| choose(m, wanted.as_deref()));
        let now = chosen.as_ref().and_then(read);
        let same_song = |a: &Track, b: &Track| a.title == b.title && a.artist == b.artist && a.player == b.player;
        let track = match (&last, now) {
            (Some(old), Some(mut new)) if same_song(old, &new) => {
                new.id = old.id;
                Some(new)
            }
            (_, Some(mut new)) => {
                ids += 1;
                new.id = ids;
                since_song = 0;
                cover_sent = None;
                Some(new)
            }
            (_, None) => None,
        };
        if track != last {
            last = track.clone();
            if news.send(Media::Track(track)).is_err() {
                break;
            }
        }
        if let (Some(track), Some(session)) = (&last, &chosen)
            && COVER_TICKS.contains(&since_song)
            && let Some(cover) = cover(session, track.id)
        {
            let hash = digest(&cover.rgba);
            if cover_sent != Some(hash) {
                cover_sent = Some(hash);
                if news.send(Media::Cover(cover)).is_err() {
                    break;
                }
            }
        }
        since_song = since_song.saturating_add(1);
        tick = tick.wrapping_add(1);
        match stop.recv_timeout(TICK) {
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            _ => break,
        }
    }
    drop(manager);
    // SAFETY: every WinRT object of this thread is gone.
    unsafe { CoUninitialize() };
}

/// The session to show: the preferred player's, else one playing, else
/// the one Windows calls current.
fn choose(manager: &Manager, prefer: Option<&str>) -> Option<Session> {
    let sessions: Vec<Session> = manager.GetSessions().ok()?.into_iter().collect();
    let playing = |s: &Session| s.GetPlaybackInfo().and_then(|i| i.PlaybackStatus()).is_ok_and(|st| st == Status::Playing);
    let named = |s: &Session| {
        let (Some(exe), Ok(id)) = (prefer, s.SourceAppUserModelId()) else { return false };
        let stem = exe.trim_end_matches(".exe").trim_end_matches(".EXE").to_lowercase();
        id.to_string().to_lowercase().contains(&stem)
    };
    let pick = |test: &dyn Fn(&Session) -> bool| sessions.iter().find(|s| test(s)).cloned();
    pick(&|s| named(s) && playing(s))
        .or_else(|| pick(&named))
        .or_else(|| pick(&playing))
        .or_else(|| manager.GetCurrentSession().ok())
}

/// What a session says it is playing; `None` without a title.
fn read(session: &Session) -> Option<Track> {
    let properties = session.TryGetMediaPropertiesAsync().ok()?.join().ok()?;
    let title = properties.Title().ok()?.to_string().trim().to_owned();
    if title.is_empty() || PLACEHOLDERS.contains(&title.as_str()) {
        return None;
    }
    let artist = properties.Artist().map(|a| a.to_string()).unwrap_or_default();
    let artist = if artist.trim().is_empty() {
        properties.AlbumArtist().map(|a| a.to_string()).unwrap_or_default()
    } else {
        artist
    };
    let status = session.GetPlaybackInfo().and_then(|i| i.PlaybackStatus()).ok();
    Some(Track {
        title,
        artist: artist.trim().to_owned(),
        album: properties.AlbumTitle().map(|a| a.to_string().trim().to_owned()).unwrap_or_default(),
        player: session.SourceAppUserModelId().map(|id| id.to_string()).unwrap_or_default(),
        playing: status == Some(Status::Playing),
        id: 0,
    })
}

/// The session's cover, decoded and scaled to at most `COVER` a side.
fn cover(session: &Session, id: u64) -> Option<Cover> {
    let properties = session.TryGetMediaPropertiesAsync().ok()?.join().ok()?;
    let stream = properties.Thumbnail().ok()?.OpenReadAsync().ok()?.join().ok()?;
    let decoder = BitmapDecoder::CreateAsync(&stream).ok()?.join().ok()?;
    let (width, height) = (decoder.OrientedPixelWidth().ok()?, decoder.OrientedPixelHeight().ok()?);
    if width == 0 || height == 0 {
        return None;
    }
    let scale = (COVER as f32 / width.max(height) as f32).min(1.0);
    let (w, h) = (((width as f32 * scale).round() as u32).max(1), ((height as f32 * scale).round() as u32).max(1));
    let transform = BitmapTransform::new().ok()?;
    transform.SetScaledWidth(w).ok()?;
    transform.SetScaledHeight(h).ok()?;
    transform.SetInterpolationMode(BitmapInterpolationMode::Fant).ok()?;
    let pixels = decoder
        .GetPixelDataTransformedAsync(
            BitmapPixelFormat::Rgba8,
            BitmapAlphaMode::Straight,
            &transform,
            ExifOrientationMode::RespectExifOrientation,
            ColorManagementMode::DoNotColorManage,
        )
        .ok()?
        .join()
        .ok()?;
    let rgba = pixels.DetachPixelData().ok()?.to_vec();
    (rgba.len() == (w * h * 4) as usize).then_some(Cover { id, width: w, height: h, rgba })
}

/// A cheap fingerprint of a picture, to tell a new cover from the same one.
fn digest(bytes: &[u8]) -> u64 {
    // FNV-1a over every 7th byte: covers that differ, differ widely.
    bytes.iter().step_by(7).fold(0xcbf2_9ce4_8422_2325, |h, &b| (h ^ u64::from(b)).wrapping_mul(0x100_0000_01b3))
}
