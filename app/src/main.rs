//! Capture what the system is playing, turn it into `Signals`, and draw them
//! in a window on the GPU. With `JUGLAR_DEMO=1` the signals come from a
//! synthetic song instead, and nothing is captured; with
//! `JUGLAR_FILE=song.wav`, from a WAV file played silently in step with the
//! picture. `JUGLAR_RECORD=folder` (with a file or the demo) draws frames at
//! a fixed rate as fast as it can and saves each one, for making videos.
//!
//! The release build is a Windows program, not a console one: starting it
//! opens no console. Its messages go to a log file (see `route_output`),
//! or to a console with `--console`.
//!
//! Juglar opens on a menu over the scene (`launcher.rs`), unless the
//! settings (`settings.rs`) say to start right away, or `--start`,
//! `--fullscreen` or `--wallpaper` do. What players say is playing comes
//! from `MediaWatch`; each new song gets its title on screen (`song.rs`)
//! and a new scene. Only one Juglar runs at a time: starting it again
//! shows the one running.

// The debug build keeps its console, for `cargo run`.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod desktop;
mod launcher;
mod settings;
mod song;
mod wav;

use std::sync::Arc;
use std::time::{Duration, Instant};

use juglar_director::{Director, Intensity, Mode, Moment, Style, Transition};
use juglar_dsp::{Analyzer, Consumer, FFT_SIZE, SCOPE_POINTS, Scope, ring};
use desktop::{Other, SPREADS, Spread, hotkey_help, spread_names};
use juglar_platform::{
    AudioApp, AudioCapture, Listen, Media, MediaWatch, PlatformError, ScreenRect, SystemAudio, Track, Tray, TrayChoice,
    TrayState, Wallpaper,
};
use juglar_render::{Renderer, Song, View};
use launcher::{Launcher, intensity_at};
use settings::{SHOWS, Settings};
use juglar_signals::{Relative, Signals, Synthetic};
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::{ElementState, KeyEvent, MouseButton, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::dpi::{PhysicalPosition, PhysicalSize};
use winit::keyboard::{Key, ModifiersState, NamedKey};
use winit::window::{Fullscreen, Window, WindowAttributes, WindowId};

const _: () = assert!(juglar_dsp::SPECTRUM_BANDS == juglar_signals::SPECTRUM_BANDS);
const _: () = assert!(SCOPE_POINTS == juglar_signals::WAVEFORM_POINTS);

/// Frames per second Juglar aims for. The real rate is the display's refresh
/// divided by a whole number (see `Pacer`), as close to this as possible.
const TARGET_FPS: f64 = 60.0;
/// How often to try again while the window is minimized or hidden.
const HIDDEN_RETRY: Duration = Duration::from_millis(100);
/// Intensity presets on keys 1, 2 and 3: gentle, medium, strong.
const INTENSITY_PRESETS: [f32; 3] = [0.2, 0.5, 0.9];
/// Two clicks closer than this are a double click.
const DOUBLE_CLICK: Duration = Duration::from_millis(400);
/// In fullscreen, the mouse pointer hides after this long without moving.
const POINTER_HIDES: Duration = Duration::from_secs(2);
/// From a window, a second `Esc` within this long closes Juglar.
const ESCAPE_AGAIN: Duration = Duration::from_secs(2);
/// A new song's title waits this long for its cover, so the cover doesn't
/// push the words aside once they are showing.
const COVER_WAIT: Duration = Duration::from_millis(900);
/// The modes in the order `M` goes through them, and the intensities'
/// names: the music's first, then the presets'.
const MODES: [Mode; 3] = [Mode::Lively, Mode::Focus, Mode::Relax];
const INTENSITIES: [&str; 4] = ["from the music", "gentle", "medium", "strong"];
/// The keys, as the help (`H`) shows them.
const HELP: &[(&str, &str)] = &[
    ("\u{2190} \u{2192}", "previous / next scene"),
    ("Space", "keep this scene / let the music change it"),
    ("M", "mode: lively, focus, relax"),
    ("A", "intensity from the music"),
    ("1 2 3", "intensity: gentle, medium, strong"),
    ("Tab", "what Juglar hears"),
    ("S", "settings: what to hear, how to show Juglar"),
    ("T", "the song's title again"),
    ("F11", "fullscreen (or Alt+Enter, or double click)"),
    ("W", "wallpaper: behind the desktop icons"),
    ("H", "this help"),
    ("Esc", "leave fullscreen; twice to close"),
];

fn main() {
    let file = std::env::var("JUGLAR_FILE").ok();
    // The ways of working on Juglar itself: a file, the synthetic song,
    // recording frames. They run beside a Juglar that is listening.
    let tools = file.is_some()
        || std::env::var("JUGLAR_DEMO").is_ok_and(|v| v != "0")
        || std::env::var("JUGLAR_RECORD").is_ok();
    // One Juglar at a time: starting it again shows the one running.
    #[cfg(windows)]
    if !tools && juglar_platform::already_running() {
        return;
    }
    route_output();
    let mut settings = Settings::load();
    let mut listen_note = String::new();
    let source = match std::env::var("JUGLAR_DEMO") {
        _ if file.is_some() => {
            let path = file.unwrap_or_default();
            let start = std::env::var("JUGLAR_FILE_START").ok().and_then(|v| v.parse().ok()).unwrap_or(0.0);
            match wav::read(&path) {
                Ok(wav) => {
                    println!("Playing {path} silently from {start} s.");
                    Source::File(Box::new(Playing::new(wav, start)))
                }
                Err(e) => fatal(&e.to_string()),
            }
        }
        Ok(v) if v != "0" => {
            let start = std::env::var("JUGLAR_DEMO_START").ok().and_then(|v| v.parse().ok()).unwrap_or(0.0);
            println!("Demo mode: a synthetic song, nothing is captured.");
            Source::Demo(Synthetic::starting_at(1, start))
        }
        _ => match Listening::hearing(&settings.listen, &mut listen_note) {
            Ok(listening) => Source::Audio(Box::new(listening)),
            Err(e) => fatal(&format!("Could not capture system audio: {e}")),
        },
    };

    let event_loop = EventLoop::new().expect("event loop");
    // The saved intensity unless `JUGLAR_INTENSITY` fixes one.
    let intensity = match std::env::var("JUGLAR_INTENSITY").ok().and_then(|v| v.parse::<f32>().ok()) {
        Some(value) => Intensity::Fixed(value.clamp(0.0, 1.0)),
        None => intensity_at(settings.intensity),
    };
    if std::env::var("JUGLAR_DISPLAYS").is_ok() {
        settings.spread = Spread::from_env();
    }
    let arguments: Vec<String> = std::env::args().collect();
    let argument = |name: &str| arguments.iter().any(|a| a == name);
    let mut app = App {
        source,
        director: Director::new(intensity, juglar_render::scene_characters()),
        signals: Signals::default(),
        window: None,
        renderer: None,
        pacer: Pacer::new(None, TARGET_FPS),
        display_hz: None,
        print_stats: std::env::var("JUGLAR_STATS").is_ok_and(|v| v != "0"),
        last_frame: Instant::now(),
        stats: Stats::new(),
        start_fullscreen: argument("--fullscreen"),
        start_wallpaper: argument("--wallpaper"),
        start_now: argument("--start") || tools,
        wallpaper: None,
        others: Vec::new(),
        spread: settings.spread,
        displays: Vec::new(),
        arrange: false,
        watch: false,
        tray: None,
        covered: false,
        modifiers: ModifiersState::empty(),
        last_click: None,
        pointer_moved: Instant::now(),
        escaped: None,
        alignment: Alignment::default(),
        still: false,
        recorder: Recorder::from_env(),
        done: false,
        settings,
        launcher: Launcher::default(),
        media: None,
        apps: Vec::new(),
        track: None,
        song: None,
        introduced: None,
        pending_intro: None,
        songs: 0,
        listen_note,
        startup: false,
    };
    if let Err(e) = event_loop.run_app(&mut app) {
        eprintln!("juglar: {e}");
    }
}

/// Where each frame's signals come from.
enum Source {
    /// What the system is playing.
    Audio(Box<Listening>),
    /// A synthetic song, for working on visuals in silence.
    Demo(Synthetic),
    /// A WAV file, heard but not played.
    File(Box<Playing>),
}

impl Source {
    fn update(&mut self, dt: f32, out: &mut Signals) {
        match self {
            Source::Audio(listening) => listening.update(dt, out),
            Source::Demo(song) => song.next(dt, out),
            Source::File(playing) => playing.update(dt, out),
        }
    }

    /// Beats, kicks, snares and cymbals heard since the start; the
    /// synthetic song's are known, not heard.
    fn hits(&self) -> Option<[u64; 4]> {
        match self {
            Source::Audio(listening) => Some(listening.ear.hits()),
            Source::File(playing) => Some(playing.ear.hits()),
            Source::Demo(_) => None,
        }
    }
}

/// System audio capture and its analysis.
struct Listening {
    /// Capture stops when dropped.
    capture: SystemAudio,
    /// What it hears: what was asked for, or everything when that program
    /// wasn't running.
    hearing: Listen,
    consumer: Consumer,
    block: Vec<f32>,
    ear: Ear,
}

/// A WAV file fed to the analysis as if it were playing.
struct Playing {
    samples: Vec<f32>,
    sample_rate: f32,
    /// Where in the file the next sample is, in samples.
    position: f64,
    ear: Ear,
}

impl Playing {
    fn new(wav: wav::Wav, start: f32) -> Self {
        Self {
            sample_rate: wav.sample_rate as f32,
            position: f64::from(start * wav.sample_rate as f32),
            ear: Ear::new(wav.sample_rate),
            samples: wav.samples,
        }
    }

    /// Feeds the next `dt` seconds of the file; silence after its end.
    fn update(&mut self, dt: f32, out: &mut Signals) {
        let from = (self.position as usize).min(self.samples.len());
        self.position += f64::from(dt * self.sample_rate);
        let to = (self.position as usize).min(self.samples.len());
        self.ear.hear(&self.samples[from..to], dt, out);
    }
}

/// The analysis, from samples to `Signals`.
struct Ear {
    analyzer: Analyzer,
    scope: Scope,
    recent: Vec<f32>,
}

impl Ear {
    fn new(sample_rate: u32) -> Self {
        Self { analyzer: Analyzer::new(sample_rate), scope: Scope::new(sample_rate), recent: vec![0.0; FFT_SIZE] }
    }

    /// Beats, kicks, snares and cymbals heard since the start.
    fn hits(&self) -> [u64; 4] {
        let [kick, snare, cymbals] = self.analyzer.drum_hits();
        [self.analyzer.beats(), kick, snare, cymbals]
    }
}

impl Listening {
    fn start(from: &Listen) -> Result<Self, PlatformError> {
        // One second of mono audio at up to 192 kHz. Every frame drains it,
        // so it never comes close to filling.
        let (mut producer, consumer) = ring(192_000);
        let mut mono = Vec::new();
        let capture = SystemAudio::start(
            from,
            Box::new(move |interleaved, channels| {
                downmix(interleaved, usize::from(channels), &mut mono);
                producer.push(&mono);
            }),
        )?;
        let info = capture.info();
        let what = match from {
            Listen::Everything => "everything".to_owned(),
            Listen::App(exe) => exe.clone(),
        };
        println!("Capturing {what}: {} Hz, {} channels.", info.sample_rate, info.channels);
        Ok(Self {
            capture,
            hearing: from.clone(),
            consumer,
            block: vec![0.0; info.sample_rate as usize],
            ear: Ear::new(info.sample_rate),
        })
    }

    /// Hears `wanted`, or everything when that program can't be heard
    /// (it isn't running), saying so in `note`.
    fn hearing(wanted: &Listen, note: &mut String) -> Result<Self, PlatformError> {
        note.clear();
        if *wanted != Listen::Everything {
            match Self::start(wanted) {
                Ok(listening) => return Ok(listening),
                Err(e) => {
                    eprintln!("juglar: {e}; hearing everything until it plays");
                    *note = "not playing: hearing everything for now".to_owned();
                }
            }
        }
        Self::start(&Listen::Everything)
    }

    /// Drains the new audio and analyzes it.
    fn update(&mut self, dt: f32, out: &mut Signals) {
        let n = self.consumer.pop(&mut self.block);
        self.ear.hear(&self.block[..n], dt, out);
    }
}

impl Ear {
    /// Analyzes `samples`, the last `dt` seconds of sound, into `out`.
    fn hear(&mut self, samples: &[f32], dt: f32, out: &mut Signals) {
        self.analyzer.push(samples);
        let analysis = self.analyzer.analyze(dt);
        out.time += dt;
        out.dt = dt;
        out.bass = analysis.bass;
        out.mid = analysis.mid;
        out.treble = analysis.treble;
        out.energy = analysis.energy;
        out.beat = analysis.beat;
        out.novelty = analysis.novelty;
        out.tempo = analysis.tempo;
        out.beat_phase = analysis.beat_phase;
        out.spectrum = analysis.spectrum;
        out.kick = analysis.kick;
        out.snare = analysis.snare;
        out.cymbals = analysis.cymbals;
        out.percussive = analysis.percussive;
        out.harmonic = analysis.harmonic;
        let r = analysis.relative;
        out.relative = Relative {
            bass: r.bass,
            mid: r.mid,
            treble: r.treble,
            energy: r.energy,
            percussive: r.percussive,
            harmonic: r.harmonic,
        };
        out.loudness = analysis.loudness;
        out.grit = analysis.grit;
        out.density = analysis.density;
        out.chaos = analysis.chaos;
        out.impact = analysis.impact;
        out.hit = analysis.hit;
        self.analyzer.recent(&mut self.recent);
        out.waveform = *self.scope.update(&self.recent, dt);
    }
}

/// Where the messages go when Juglar has no console of its own: to a
/// console with `--console`, else into `%LOCALAPPDATA%\Juglar\juglar.log`,
/// started over each time (the last run's is kept as `juglar.old.log`).
/// Output already redirected, by a script for example, stays where it is.
#[cfg(windows)]
fn route_output() {
    if juglar_platform::output_attached() {
        return;
    }
    if std::env::args().any(|arg| arg == "--console") {
        juglar_platform::open_console();
        return;
    }
    let Some(folder) = std::env::var_os("LOCALAPPDATA").map(|dir| std::path::Path::new(&dir).join("Juglar")) else {
        return;
    };
    let log = folder.join("juglar.log");
    let _ = std::fs::create_dir_all(&folder);
    let _ = std::fs::rename(&log, folder.join("juglar.old.log"));
    if let Ok(file) = std::fs::File::create(&log) {
        juglar_platform::send_output_to(file);
        LOGGING.store(true, std::sync::atomic::Ordering::Relaxed);
        println!("Juglar {}", env!("CARGO_PKG_VERSION"));
    }
}

#[cfg(not(windows))]
fn route_output() {}

/// Whether the messages go only to the log file, where nobody watches.
static LOGGING: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Stops Juglar over an error it can't go on after, saying why: in the log,
/// and in a message box when the log is the only other place it would go.
fn fatal(why: &str) -> ! {
    eprintln!("juglar: {why}");
    #[cfg(windows)]
    if LOGGING.load(std::sync::atomic::Ordering::Relaxed) {
        juglar_platform::alert(why);
    }
    std::process::exit(1);
}

/// Whether to keep the picture still: the OS setting, unless
/// `JUGLAR_REDUCED_MOTION` is set to 1 or 0 (until the settings file, M7).
fn reduced_motion() -> bool {
    match std::env::var("JUGLAR_REDUCED_MOTION").as_deref() {
        Ok("1") => true,
        Ok("0") => false,
        _ => juglar_platform::reduced_motion(),
    }
}

/// The target frame rate: `TARGET_FPS`, or the `JUGLAR_FPS` environment
/// variable until there is a settings file (M7). Fewer frames cost less, for
/// example while a game is running.
fn target_fps() -> f64 {
    std::env::var("JUGLAR_FPS").ok().and_then(|v| v.parse().ok()).filter(|&fps: &f64| fps >= 1.0).unwrap_or(TARGET_FPS)
}

/// Averages each interleaved frame into one mono sample, reusing `out`.
fn downmix(interleaved: &[f32], channels: usize, out: &mut Vec<f32>) {
    out.clear();
    let scale = 1.0 / channels as f32;
    out.extend(interleaved.chunks_exact(channels).map(|frame| frame.iter().sum::<f32>() * scale));
}

struct App {
    source: Source,
    director: Director,
    signals: Signals,
    window: Option<Arc<Window>>,
    renderer: Option<Renderer>,
    pacer: Pacer,
    /// The refresh rate the pacer was last snapped to (`Some(None)` when
    /// the display didn't report one).
    display_hz: Option<Option<f64>>,
    /// `JUGLAR_STATS=1` also prints the title's numbers once a second, so a
    /// script can tell whether frames are being drawn at all.
    print_stats: bool,
    last_frame: Instant,
    stats: Stats,
    /// How well the counted beat matches the detected one (stats only).
    alignment: Alignment,
    /// `--fullscreen` on the command line.
    start_fullscreen: bool,
    /// `--wallpaper` on the command line.
    start_wallpaper: bool,
    /// `--start` on the command line (or a tool's variable): no menu,
    /// shown as the settings say.
    start_now: bool,
    /// While Juglar is the wallpaper: the window's place in the desktop,
    /// the area it covers, and where it was as a window.
    wallpaper: Option<(Wallpaper, ScreenRect, ScreenRect)>,
    /// The wallpapers on the other displays (`desktop.rs`), how the
    /// displays are used, and the displays as last seen.
    others: Vec<Other>,
    spread: Spread,
    displays: Vec<ScreenRect>,
    /// Wallpaper mode was asked for (or must be laid out again); done
    /// where windows can be made.
    arrange: bool,
    /// Time to check the desktop again (once a second).
    watch: bool,
    /// The notification-area icon, while Juglar is the wallpaper.
    tray: Option<Tray>,
    /// Whether something covers the desktop (wallpaper mode only), as last
    /// checked: then nothing is drawn.
    covered: bool,
    /// Shift, Ctrl, Alt and the Windows key, as last reported.
    modifiers: ModifiersState,
    /// When the left button was last pressed, to tell double clicks.
    last_click: Option<Instant>,
    /// When the mouse pointer last moved.
    pointer_moved: Instant,
    /// When `Esc` last asked whether to close.
    escaped: Option<Instant>,
    /// Whether reduced motion is on, as last read (once a second).
    still: bool,
    /// Saving frames (`JUGLAR_RECORD`), and whether it has saved them all.
    recorder: Option<Recorder>,
    done: bool,
    settings: Settings,
    launcher: Launcher,
    /// What the players say, and the programs making sound.
    media: Option<MediaWatch>,
    apps: Vec<AudioApp>,
    /// The song as the player says it, and as Juglar shows it.
    track: Option<Track>,
    song: Option<Song>,
    /// The song whose title was shown last, and a title waiting for its
    /// cover since then.
    introduced: Option<u64>,
    pending_intro: Option<Instant>,
    /// Songs seen start since Juglar started.
    songs: u32,
    /// Why Juglar hears something else than was chosen, if it does.
    listen_note: String,
    /// Whether Juglar starts with Windows, as last read.
    startup: bool,
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let attributes = window_attributes()
            .with_inner_size(LogicalSize::new(960.0, 540.0))
            .with_min_inner_size(LogicalSize::new(320.0, 180.0));
        let window = Arc::new(event_loop.create_window(attributes).expect("window"));
        let size = window.inner_size();
        let renderer = match pollster::block_on(Renderer::new(window.clone(), size.width, size.height)) {
            Ok(renderer) => renderer,
            Err(e) => fatal(&format!("Could not start the GPU: {e}")),
        };
        println!("GPU: {}", renderer.adapter_name());
        #[cfg(windows)]
        if let Some(handle) = desktop::hwnd(&window) {
            juglar_platform::dark_title_bar(handle);
        }
        self.window = Some(window);
        if self.start_fullscreen {
            self.toggle_fullscreen();
        }
        self.follow_display();

        let mut renderer = renderer;
        renderer.set_fonts(&ui_fonts());
        // `JUGLAR_SCENE=oil` (or any scene's name) picks the first scene.
        if let Ok(name) = std::env::var("JUGLAR_SCENE") {
            match renderer.scene_names().iter().position(|&scene| scene == name) {
                Some(index) => {
                    renderer.set_scene(index);
                    self.director.chosen(index);
                }
                None => eprintln!("juglar: no scene called {name:?}; there are {:?}", renderer.scene_names()),
            }
        }
        // `JUGLAR_VIEW=analysis` starts on what Juglar hears, as `Tab` does.
        if std::env::var("JUGLAR_VIEW").is_ok_and(|v| v == "analysis") {
            renderer.set_view(View::Analysis);
        }
        // `JUGLAR_KEEP=1` keeps the first scene, as `Space` does.
        if std::env::var("JUGLAR_KEEP").is_ok_and(|v| v != "0") {
            self.director.hold(true);
        }
        self.still = reduced_motion();
        renderer.set_reduced_motion(self.still);
        if self.still {
            println!("Reduced motion is on: the picture stays still.");
        }
        // `JUGLAR_MODE=focus` (or `relax`) starts in a calm mode; else the
        // saved one.
        match std::env::var("JUGLAR_MODE").as_deref() {
            Ok("focus") => self.set_mode(Mode::Focus, &mut renderer),
            Ok("relax") => self.set_mode(Mode::Relax, &mut renderer),
            Ok("lively") => {}
            Err(_) => self.set_mode(self.settings.mode, &mut renderer),
            Ok(other) => eprintln!("juglar: no mode called {other:?}; there are lively, focus and relax"),
        }
        if self.recorder.is_some() {
            renderer.enable_capture();
        }
        self.renderer = Some(renderer);
        self.last_frame = Instant::now();
        // The tray icon is always there; its keys only as the wallpaper.
        #[cfg(windows)]
        {
            match Tray::new("Juglar") {
                Ok(tray) => self.tray = Some(tray),
                Err(e) => eprintln!("juglar: no tray icon ({e})"),
            }
            self.startup = juglar_platform::starts_with_windows();
        }
        if matches!(self.source, Source::Audio(_)) {
            let media = MediaWatch::start();
            media.prefer(self.listened_app());
            self.media = Some(media);
        }
        if self.start_wallpaper {
            self.arrange = true;
        } else if self.start_fullscreen {
            // Done above.
        } else if self.start_now || !self.settings.menu {
            self.apply_show();
        } else {
            self.open_menu(true);
        }
        self.sync_tray();
    }

    /// Leaves the desktop as it was: the window comes out of it before it
    /// is destroyed.
    fn exiting(&mut self, _event_loop: &ActiveEventLoop) {
        self.others.clear();
        self.wallpaper = None;
        self.tray = None;
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, id: WindowId, event: WindowEvent) {
        // The other displays' wallpapers only need to follow their size.
        if self.window.as_ref().is_some_and(|w| w.id() != id) {
            if let WindowEvent::Resized(size) = event {
                self.resize_other(id, size);
            }
            return;
        }
        // The menu takes the keys and the mouse while it is open.
        if self.launcher.open {
            match event {
                WindowEvent::CloseRequested => event_loop.exit(),
                WindowEvent::KeyboardInput { event: KeyEvent { logical_key, state: ElementState::Pressed, .. }, .. } => {
                    self.menu_key(&logical_key)
                }
                WindowEvent::CursorMoved { position, .. } => self.menu_pointer([position.x as f32, position.y as f32]),
                WindowEvent::MouseInput { state: ElementState::Pressed, button: MouseButton::Left, .. } => self.menu_click(),
                WindowEvent::Resized(size) => {
                    if let Some(renderer) = &mut self.renderer {
                        renderer.resize(size.width, size.height);
                    }
                }
                WindowEvent::Moved(_) | WindowEvent::ScaleFactorChanged { .. } => self.follow_display(),
                WindowEvent::RedrawRequested => self.frame(),
                _ => {}
            }
            return;
        }
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::KeyboardInput {
                event: KeyEvent { logical_key: Key::Named(NamedKey::Escape), state: ElementState::Pressed, repeat: false, .. },
                ..
            } => self.escape(event_loop),
            WindowEvent::KeyboardInput { event: KeyEvent { logical_key, state: ElementState::Pressed, repeat: false, .. }, .. } => {
                self.key(&logical_key)
            }
            WindowEvent::ModifiersChanged(modifiers) => self.modifiers = modifiers.state(),
            WindowEvent::MouseInput { state: ElementState::Pressed, button: MouseButton::Left, .. } => {
                let now = Instant::now();
                if self.last_click.is_some_and(|last| now - last < DOUBLE_CLICK) {
                    self.toggle_fullscreen();
                    self.last_click = None;
                } else {
                    self.last_click = Some(now);
                }
            }
            WindowEvent::CursorMoved { .. } => {
                self.pointer_moved = Instant::now();
                if let Some(window) = &self.window {
                    window.set_cursor_visible(true);
                }
            }
            WindowEvent::Resized(size) => {
                if let Some(renderer) = &mut self.renderer {
                    renderer.resize(size.width, size.height);
                }
            }
            // The window may have reached a display with another refresh rate.
            WindowEvent::Moved(_) | WindowEvent::ScaleFactorChanged { .. } => self.follow_display(),
            WindowEvent::RedrawRequested => self.frame(),
            _ => {}
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        while let Some(choice) = self.tray.as_ref().and_then(Tray::take) {
            match choice {
                TrayChoice::PreviousScene => self.step_scene(true),
                TrayChoice::NextScene => self.step_scene(false),
                TrayChoice::KeepScene => self.toggle_keep(),
                TrayChoice::Mode(n) => self.choose_mode(MODES[n.min(MODES.len() - 1)]),
                TrayChoice::NextMode => self.choose_mode(self.director.mode().next()),
                TrayChoice::Intensity(0) => self.choose_intensity(Intensity::Natural),
                TrayChoice::Intensity(n) => {
                    self.choose_intensity(Intensity::Fixed(INTENSITY_PRESETS[(n - 1).min(INTENSITY_PRESETS.len() - 1)]))
                }
                TrayChoice::NextIntensity => self.choose_intensity(self.next_intensity()),
                TrayChoice::Displays(n) => {
                    self.spread = SPREADS[n.min(SPREADS.len() - 1)].0;
                    self.settings.spread = self.spread;
                    self.settings.save();
                    self.arrange = self.wallpaper.is_some();
                }
                TrayChoice::Listen(n) => {
                    if let Some((listen, _)) = self.sources().get(n) {
                        self.listen_to(listen.clone());
                        self.settings.save();
                    }
                }
                TrayChoice::Show(n) => {
                    self.settings.show = SHOWS[n.min(SHOWS.len() - 1)].0;
                    self.settings.save();
                    if self.launcher.open {
                        self.close_menu();
                    } else {
                        self.apply_show();
                    }
                }
                TrayChoice::Intros => {
                    self.set_intros(!self.settings.intros);
                    self.settings.save();
                }
                TrayChoice::Startup => self.toggle_startup(),
                TrayChoice::Settings => self.open_menu(false),
                TrayChoice::Open => {
                    if self.wallpaper.is_some() {
                        self.open_menu(false);
                    } else if let Some(window) = &self.window {
                        window.set_minimized(false);
                        window.focus_window();
                    }
                }
                TrayChoice::Help => self.toggle_help(),
                TrayChoice::Window => self.leave_wallpaper(),
                TrayChoice::Quit => event_loop.exit(),
            }
            self.refresh_menu();
        }
        if self.arrange {
            self.arrange = false;
            self.arrange_wallpaper(event_loop);
        }
        if self.watch {
            self.watch = false;
            self.watch_desktop(event_loop);
        }
        if self.done {
            event_loop.exit();
            return;
        }
        let Some(window) = &self.window else { return };
        // Recording draws the next frame as soon as the last is saved.
        if self.recorder.is_some() {
            window.request_redraw();
            event_loop.set_control_flow(ControlFlow::Poll);
        } else if Instant::now() >= self.pacer.next {
            window.request_redraw();
            event_loop.set_control_flow(ControlFlow::Wait);
        } else {
            // Sleep until the next frame is due; the OS wakes us on time.
            event_loop.set_control_flow(ControlFlow::WaitUntil(self.pacer.next));
        }
    }
}

impl App {
    /// Snaps the frame rate to the refresh rate of the display the window is
    /// on. Called at start and whenever the window moves, since a window
    /// dragged from a 144 Hz display to a 75 Hz one must change its rhythm.
    fn follow_display(&mut self) {
        let Some(window) = &self.window else { return };
        let refresh_hz = window.current_monitor().and_then(|m| m.refresh_rate_millihertz()).map(|mhz| f64::from(mhz) / 1000.0);
        if self.display_hz == Some(refresh_hz) {
            return;
        }
        self.display_hz = Some(refresh_hz);
        self.pacer = Pacer::new(refresh_hz, target_fps());
        match refresh_hz {
            Some(hz) => println!("Display: {hz:.0} Hz, drawing at {:.1} fps.", self.pacer.fps()),
            None => println!("Display refresh rate unknown, drawing at {:.1} fps.", self.pacer.fps()),
        }
    }

    fn is_fullscreen(&self) -> bool {
        self.window.as_ref().is_some_and(|w| w.fullscreen().is_some())
    }

    /// Fills the display the window is on, without borders, or goes back to
    /// a window. Borderless rather than exclusive fullscreen: the display
    /// keeps its mode, so switching is instant and other windows can still
    /// appear on top.
    fn toggle_fullscreen(&mut self) {
        let Some(window) = &self.window else { return };
        if window.fullscreen().is_some() {
            window.set_fullscreen(None);
            window.set_cursor_visible(true);
        } else {
            window.set_fullscreen(Some(Fullscreen::Borderless(window.current_monitor())));
            self.pointer_moved = Instant::now();
        }
    }

    fn set_mode(&mut self, mode: Mode, renderer: &mut Renderer) {
        self.director.set_mode(mode);
        renderer.set_calm(mode.calm());
    }

    /// Tab switches between the scene and the analysis view; the left and
    /// right arrows go through the scenes, and space keeps the one showing;
    /// 1, 2 and 3 fix the intensity at gentle, medium or strong, and A lets
    /// the music set it; M goes through the modes (lively, focus, relax);
    /// F11 and Alt+Enter switch fullscreen, W makes Juglar the wallpaper;
    /// H, ? or F1 show the keys. Each change is noted on screen.
    fn key(&mut self, key: &Key) {
        match key {
            Key::Named(NamedKey::F11) => self.toggle_fullscreen(),
            Key::Named(NamedKey::Enter) if self.modifiers.alt_key() => self.toggle_fullscreen(),
            Key::Named(step @ (NamedKey::ArrowLeft | NamedKey::ArrowRight)) => self.step_scene(*step == NamedKey::ArrowLeft),
            Key::Named(NamedKey::Space) => self.toggle_keep(),
            Key::Named(NamedKey::Tab) => {
                if let Some(renderer) = &mut self.renderer {
                    let (next, name) = match renderer.view() {
                        View::Scene => (View::Analysis, "what Juglar hears"),
                        View::Analysis => (View::Scene, "the scene"),
                    };
                    renderer.set_view(next);
                    renderer.note("View", name);
                }
            }
            Key::Named(NamedKey::F1) => self.toggle_help(),
            Key::Character(c) if c.eq_ignore_ascii_case("h") || c == "?" => self.toggle_help(),
            Key::Character(c) if c.eq_ignore_ascii_case("w") => self.arrange = self.wallpaper.is_none(),
            Key::Character(c) if c.eq_ignore_ascii_case("s") => self.open_menu(false),
            Key::Character(c) if c.eq_ignore_ascii_case("t") => self.title_again(),
            Key::Character(c) if c.eq_ignore_ascii_case("m") => self.choose_mode(self.director.mode().next()),
            Key::Character(c) if c.eq_ignore_ascii_case("a") => self.choose_intensity(Intensity::Natural),
            Key::Character(c) => {
                if let Some(&preset) = c.parse::<usize>().ok().and_then(|n| INTENSITY_PRESETS.get(n.wrapping_sub(1))) {
                    self.choose_intensity(Intensity::Fixed(preset));
                }
            }
            _ => {}
        }
    }

    /// `Esc` hides the help, then leaves fullscreen; from a window it asks
    /// first, and a second press closes Juglar, so a stray touch doesn't.
    fn escape(&mut self, event_loop: &ActiveEventLoop) {
        let now = Instant::now();
        if self.renderer.as_ref().is_some_and(Renderer::helping) {
            self.toggle_help();
        } else if self.is_fullscreen() {
            self.toggle_fullscreen();
        } else if self.escaped.is_some_and(|then| now - then < ESCAPE_AGAIN) {
            event_loop.exit();
        } else {
            self.escaped = Some(now);
            if let Some(renderer) = &mut self.renderer {
                renderer.note("Esc again", "to close Juglar");
            }
        }
    }

    /// Shows the keys, or hides them; as the wallpaper, the keys that work
    /// from anywhere, on the display under the pointer.
    fn toggle_help(&mut self) {
        let wallpaper = self.wallpaper.is_some();
        let target = self.under_pointer();
        if let Some((renderer, _)) = self.screen(target) {
            let lines = if wallpaper { hotkey_help() } else { HELP.to_vec() };
            renderer.help(if renderer.helping() { &[] } else { &lines });
        }
    }

    /// To the previous or next scene, through a transition: on the display
    /// under the pointer, when Juglar is the wallpaper on several.
    fn step_scene(&mut self, back: bool) {
        let still = self.still;
        let target = self.under_pointer();
        let Some((renderer, director)) = self.screen(target) else { return };
        let count = renderer.scene_names().len();
        let next = if back { renderer.scene() + count - 1 } else { renderer.scene() + 1 } % count;
        // With reduced motion, a slow fade through patches instead of an
        // edge sweeping across.
        let transition = if still {
            Transition { style: Style::Seep, seconds: 3.0, per_beat: 0.0 }
        } else {
            Transition::asked()
        };
        renderer.begin_transition(next, transition);
        director.chosen(next);
        let kept = if director.holding() { ", kept" } else { "" };
        renderer.note("Scene", &format!("{}{kept}", title(renderer.scene_names()[next])));
        self.sync_tray();
    }

    /// Keeps the scene showing (no automatic changes), or lets go.
    fn toggle_keep(&mut self) {
        let target = self.under_pointer();
        if let Some((renderer, director)) = self.screen(target) {
            let holding = !director.holding();
            director.hold(holding);
            let value = if holding { format!("{}, kept", App::scene_title(renderer)) } else { "changes with the music".to_owned() };
            renderer.note("Scene", &value);
        }
        self.sync_tray();
    }

    fn choose_mode(&mut self, mode: Mode) {
        let menu = self.launcher.open;
        if let Some(mut renderer) = self.renderer.take() {
            self.set_mode(mode, &mut renderer);
            if !menu {
                renderer.note("Mode", mode.name());
            }
            self.renderer = Some(renderer);
        }
        self.mode_everywhere(mode);
        self.settings.mode = mode;
        self.settings.save();
        self.sync_tray();
    }

    fn choose_intensity(&mut self, intensity: Intensity) {
        self.director.set_intensity(intensity, self.signals.intensity);
        let index = intensity_index(intensity);
        let name = INTENSITIES[index];
        if let Some(renderer) = &mut self.renderer
            && !self.launcher.open
        {
            renderer.note("Intensity", name);
        }
        self.intensity_everywhere(intensity, name);
        self.settings.intensity = index;
        self.settings.save();
        self.sync_tray();
    }

    /// Listens to `listen` from now on (everything, or one program), and
    /// remembers it.
    fn listen_to(&mut self, listen: Listen) {
        self.settings.listen = listen;
        self.relisten();
        if let Some(media) = &self.media {
            media.prefer(self.listened_app());
        }
        self.sync_tray();
    }

    /// The program chosen to listen to, if one is.
    fn listened_app(&self) -> Option<&str> {
        match &self.settings.listen {
            Listen::Everything => None,
            Listen::App(exe) => Some(exe),
        }
    }

    /// Starts capture again for what the settings ask.
    fn relisten(&mut self) {
        let Source::Audio(listening) = &mut self.source else { return };
        match Listening::hearing(&self.settings.listen, &mut self.listen_note) {
            Ok(new) => **listening = new,
            Err(e) => eprintln!("juglar: could not capture audio: {e}"),
        }
    }

    /// Every two seconds: capture goes on when the program heard closes,
    /// and goes back to it when it opens again.
    fn check_listening(&mut self) {
        let Source::Audio(listening) = &self.source else { return };
        let wanted = &self.settings.listen;
        let waiting = listening.hearing != *wanted
            && matches!(wanted, Listen::App(exe) if self.apps.iter().any(|a| a.exe.eq_ignore_ascii_case(exe)));
        if !listening.capture.alive() || waiting {
            self.relisten();
            self.refresh_menu();
        }
    }

    /// Shows each new song's title, or stops.
    fn set_intros(&mut self, on: bool) {
        self.settings.intros = on;
        if !on {
            for renderer in self.renderers() {
                renderer.end_intro();
            }
        }
        self.sync_tray();
    }

    /// Starts with Windows (as the menu's settings say), or stops.
    fn toggle_startup(&mut self) {
        #[cfg(windows)]
        {
            let on = !juglar_platform::starts_with_windows();
            if let Err(e) = juglar_platform::set_starts_with_windows(on, "--start") {
                eprintln!("juglar: could not change starting with Windows: {e}");
            }
            self.startup = juglar_platform::starts_with_windows();
        }
        self.sync_tray();
    }

    /// Every display's renderer, the main window's first.
    fn renderers(&mut self) -> impl Iterator<Item = &mut Renderer> {
        self.renderer.iter_mut().chain(self.others.iter_mut().map(|o| &mut o.renderer))
    }

    /// Follows what the players say: a new song gets its title and a new
    /// scene; its cover goes to every display.
    fn follow_media(&mut self) {
        let Some(media) = &self.media else { return };
        let mut changed = false;
        for news in media.news() {
            changed = true;
            match news {
                Media::Apps(apps) => self.apps = apps,
                Media::Track(track) => {
                    let new = track.as_ref().map(|t| t.id) != self.track.as_ref().map(|t| t.id);
                    if new {
                        for renderer in self.renderers() {
                            renderer.set_cover(None);
                        }
                    }
                    self.song = track.as_ref().map(|t| song::song(t, &self.apps));
                    self.track = track;
                    let tip = self.song.as_ref().map_or("Juglar".to_owned(), |s| format!("Juglar \u{b7} {}", song::line(s)));
                    if let Some(tray) = &self.tray {
                        tray.set_tip(&tip);
                    }
                    // A song not yet introduced, once it plays.
                    if let Some(track) = &self.track
                        && track.playing
                        && self.introduced != Some(track.id)
                        && self.pending_intro.is_none()
                    {
                        self.pending_intro = Some(Instant::now());
                    }
                }
                Media::Cover(cover) => {
                    if self.track.as_ref().is_some_and(|t| t.id == cover.id) {
                        for renderer in self.renderers() {
                            renderer.set_cover(Some((cover.width, cover.height, &cover.rgba)));
                        }
                        // The title waited for this.
                        if let Some(since) = &mut self.pending_intro {
                            *since -= COVER_WAIT;
                        }
                    }
                }
            }
        }
        if let Some(since) = self.pending_intro
            && since.elapsed() >= COVER_WAIT
            && !self.launcher.open
        {
            self.pending_intro = None;
            self.introduce();
        }
        if changed {
            self.sync_tray();
            self.refresh_menu();
        }
    }

    /// A new song: its title on every display, and a new scene, but for
    /// the song playing when Juglar started.
    fn introduce(&mut self) {
        let Some(track) = &self.track else { return };
        self.introduced = Some(track.id);
        self.songs += 1;
        let fresh = self.songs > 1;
        let intros = self.settings.intros;
        let Some(song) = self.song.clone() else { return };
        if intros {
            for renderer in self.renderers() {
                renderer.intro(song.clone());
            }
        }
        let still = self.still;
        if fresh
            && !still
            && let Some(renderer) = &mut self.renderer
            && let Some(change) = self.director.new_song(renderer.scene())
        {
            renderer.begin_transition(change.scene, change.transition);
            renderer.director_changed(moment_name(change.moment), change.scene);
        }
        if fresh && !still {
            for other in &mut self.others {
                if let Some(change) = other.director.new_song(other.renderer.scene()) {
                    other.renderer.begin_transition(change.scene, change.transition);
                    other.renderer.director_changed(moment_name(change.moment), change.scene);
                }
            }
        }
        self.sync_tray();
    }

    /// `T`: the song's title again.
    fn title_again(&mut self) {
        match self.song.clone() {
            Some(song) => {
                for renderer in self.renderers() {
                    renderer.intro(song.clone());
                }
            }
            None => {
                if let Some(renderer) = &mut self.renderer {
                    renderer.note("Song", "no player says what is playing");
                }
            }
        }
    }

    /// Shows the song, the scene, and every choice in the tray's menu.
    fn sync_tray(&self) {
        if let Some(tray) = &self.tray
            && let Some(state) = self.tray_state()
        {
            tray.set_state(state);
        }
    }

    /// What the tray's menu shows.
    fn tray_state(&self) -> Option<TrayState> {
        let renderer = self.renderer.as_ref()?;
        let mode = self.director.mode();
        let (displays, display) = spread_names(&self.displays, self.spread);
        let sources = self.sources();
        Some(TrayState {
            song: self.song.as_ref().map(|s| format!("\u{266a}  {}", song::line(s))).unwrap_or_default(),
            scene: title(renderer.scene_names()[renderer.scene()]),
            keeping: self.director.holding(),
            modes: MODES.iter().map(|m| title(m.name())).collect(),
            mode: MODES.iter().position(|&m| m == mode).unwrap_or(0),
            intensities: INTENSITIES.iter().map(|i| title(i)).collect(),
            intensity: intensity_index(self.director.intensity()),
            displays: if self.wallpaper.is_some() { displays } else { Vec::new() },
            display,
            source: self.source_index(&sources),
            sources: sources.into_iter().map(|(_, name)| name).collect(),
            shows: SHOWS.iter().map(|&(_, name)| name.to_owned()).collect(),
            show: SHOWS.iter().position(|&(s, _)| s == self.showing()).unwrap_or(0),
            intros: self.settings.intros,
            startup: self.startup,
        })
    }

    fn frame(&mut self) {
        let now = Instant::now();
        let mut dt = (now - self.last_frame).as_secs_f32();
        self.last_frame = now;
        if let Some(recorder) = &self.recorder {
            dt = recorder.dt();
        }

        self.follow_media();
        // Signals keep updating even when nothing can be drawn, so the ring
        // buffer keeps draining and the envelopes stay current.
        let work = Instant::now();
        self.source.update(dt, &mut self.signals);
        if self.print_stats {
            self.alignment.frame(&self.signals);
        }
        // Nothing to change before there is a picture, and no automatic
        // changes with reduced motion: the picture stays as it is.
        let (scene, busy) = match &self.renderer {
            Some(renderer) => (renderer.scene(), renderer.in_transition() || self.still),
            None => (0, true),
        };
        // The other displays decide from what was heard, before this
        // director writes into it.
        let heard = (!self.others.is_empty()).then(|| self.signals.clone());
        let elsewhere: Vec<usize> = self.others.iter().map(|o| o.renderer.scene()).collect();
        self.director.elsewhere(&elsewhere);
        let change = self.director.update(&mut self.signals, scene, busy);
        let analysis_time = work.elapsed();

        let Some(renderer) = &mut self.renderer else { return };
        if let Some(change) = change {
            renderer.begin_transition(change.scene, change.transition);
            renderer.director_changed(moment_name(change.moment), change.scene);
            if self.print_stats {
                let name = renderer.scene_names()[change.scene];
                let feel = self.director.feel();
                println!(
                    "Director: {:?} at {:.2} s → {name}, {:?} (pulse {:.2}, brightness {:.2}, swell {:.2}, tension {:.2})",
                    change.moment,
                    self.signals.time,
                    change.transition.style,
                    feel.pulse,
                    feel.brightness,
                    feel.swell,
                    feel.tension
                );
            }
        }
        // A minimized window can't be seen, but DirectX 12 still accepts its
        // frames, so drawing them would cost as much as drawing visible ones.
        let minimized = self.window.as_ref().and_then(|w| w.is_minimized()).unwrap_or(false);
        let work = Instant::now();
        let drawn = !minimized && !self.covered && renderer.render(&self.signals);
        if let Some(heard) = &heard {
            self.draw_others(heard);
        }
        let Some(renderer) = &mut self.renderer else { return };
        let draw_time = work.elapsed();
        self.pacer.frame_done(now, drawn);
        if drawn
            && let Some(recorder) = &mut self.recorder
            && let Some(frame) = renderer.take_capture()
        {
            self.done = recorder.save(&frame);
        }
        if let Some(ms) = renderer.take_gpu_ms() {
            self.stats.gpu_ms.push(ms);
        }
        if drawn {
            self.stats.frame(dt, analysis_time, draw_time);
        }
        let report = self.stats.report(now, self.source.hits());
        if report.is_some() {
            self.sync_tray();
            if self.stats.seconds % 2 == 0 {
                self.check_listening();
            }
        }
        let Some(renderer) = &mut self.renderer else { return };
        if let Some(stats) = report {
            let intensity = match self.director.intensity() {
                Intensity::Natural => "natural",
                Intensity::Fixed(_) => "intensity",
            };
            let kept = if self.director.holding() { " (kept)" } else { "" };
            let mode = match self.director.mode() {
                Mode::Lively => String::new(),
                calm => format!(" · {}", calm.name()),
            };
            let tempo = match self.signals.tempo {
                0.0 => String::new(),
                bpm => format!(" · {bpm:.0} bpm"),
            };
            let title = format!(
                "{stats}{tempo}{mode} · {intensity} {:.2} · {}{kept}",
                self.signals.intensity,
                renderer.scene_names()[renderer.scene()]
            );
            // Once a second: cheap enough to follow the OS setting live.
            self.still = reduced_motion();
            renderer.set_reduced_motion(self.still);
            for other in &mut self.others {
                other.renderer.set_reduced_motion(self.still);
            }
            self.watch = self.wallpaper.is_some();
            if let Some(window) = &self.window
                && window.fullscreen().is_some()
                && now - self.pointer_moved > POINTER_HIDES
            {
                window.set_cursor_visible(false);
            }
            if self.print_stats {
                // Also what the director hears, to tune it by.
                let feel = self.director.feel();
                println!(
                    "{title} · pulse {:.2} loudness {:.2} swell {:.2} brightness {:.2} rise {:+.2} tension {:.2}{}",
                    feel.pulse,
                    feel.loudness,
                    feel.swell,
                    feel.brightness,
                    feel.rise,
                    feel.tension,
                    self.alignment.report()
                );
            }
            // The numbers are for tuning; otherwise the title names the scene.
            if let Some(window) = &self.window {
                let scene = self::title(renderer.scene_names()[renderer.scene()]);
                window.set_title(&if self.print_stats { title } else { format!("Juglar \u{b7} {scene}") });
            }
        }
    }
}

/// Saves frames for `JUGLAR_RECORD`: a fixed rate (`JUGLAR_RECORD_FPS`, 30
/// by default) for `JUGLAR_RECORD_SECONDS` (10), as numbered PPM images.
struct Recorder {
    folder: std::path::PathBuf,
    fps: f32,
    frames: u32,
    saved: u32,
}

impl Recorder {
    fn from_env() -> Option<Self> {
        let folder = std::env::var("JUGLAR_RECORD").ok()?;
        let number = |name: &str, default: f32| std::env::var(name).ok().and_then(|v| v.parse().ok()).unwrap_or(default);
        let fps = number("JUGLAR_RECORD_FPS", 30.0).max(1.0);
        let frames = (number("JUGLAR_RECORD_SECONDS", 10.0) * fps).round() as u32;
        std::fs::create_dir_all(&folder).ok()?;
        println!("Recording {frames} frames at {fps} fps into {folder}.");
        Some(Self { folder: folder.into(), fps, frames, saved: 0 })
    }

    fn dt(&self) -> f32 {
        1.0 / self.fps
    }

    /// Saves one frame; returns whether that was the last.
    fn save(&mut self, frame: &juglar_render::Capture) -> bool {
        use std::io::Write;
        self.saved += 1;
        let path = self.folder.join(format!("{:05}.ppm", self.saved));
        let header = format!("P6\n{} {}\n255\n", frame.width, frame.height);
        let written = std::fs::File::create(&path).and_then(|mut file| {
            file.write_all(header.as_bytes())?;
            file.write_all(&frame.rgb)
        });
        if let Err(e) = written {
            eprintln!("juglar: {}: {e}", path.display());
            return true;
        }
        self.saved >= self.frames
    }
}

/// A moment of the music as the analysis view names it.
fn moment_name(moment: Moment) -> &'static str {
    match moment {
        Moment::Drop => "drop",
        Moment::Rise => "rise",
        Moment::Fall => "breakdown",
        Moment::Turn => "turn",
        Moment::Impact => "impact",
        Moment::Long => "time",
        Moment::Song => "new song",
    }
}

/// Juglar's windows: titled, with the logo from the icon build.rs puts in
/// the program (resource 1).
fn window_attributes() -> WindowAttributes {
    let attributes = Window::default_attributes().with_title("Juglar");
    #[cfg(windows)]
    let attributes = {
        use winit::platform::windows::{IconExtWindows, WindowAttributesExtWindows};
        use winit::window::Icon;
        attributes.with_window_icon(Icon::from_resource(1, None).ok()).with_taskbar_icon(Icon::from_resource(1, None).ok())
    };
    attributes
}

/// A name as a title: its first letter capitalized.
fn title(name: &str) -> String {
    let mut chars = name.chars();
    chars.next().map(|first| first.to_uppercase().chain(chars).collect()).unwrap_or_default()
}

/// Where an intensity is in `INTENSITIES`: the music's, or a preset's.
fn intensity_index(intensity: Intensity) -> usize {
    match intensity {
        Intensity::Natural => 0,
        Intensity::Fixed(value) => {
            INTENSITY_PRESETS.iter().position(|&preset| (preset - value).abs() < 1e-3).map_or(0, |n| n + 1)
        }
    }
}

/// The fonts for text on screen: Windows' own Segoe UI, regular and
/// semibold (bold where there is no semibold).
fn ui_fonts() -> Vec<Vec<u8>> {
    let folder = std::env::var_os("WINDIR").map(|dir| std::path::Path::new(&dir).join("Fonts")).unwrap_or_default();
    let read = |names: &[&str]| names.iter().find_map(|name| std::fs::read(folder.join(name)).ok());
    [read(&["segoeui.ttf", "arial.ttf"]), read(&["seguisb.ttf", "segoeuib.ttf", "arialbd.ttf"])].into_iter().flatten().collect()
}

fn rect(position: PhysicalPosition<i32>, size: PhysicalSize<u32>) -> ScreenRect {
    ScreenRect { left: position.x, top: position.y, width: size.width, height: size.height }
}

/// Decides when each frame starts.
///
/// The display refreshes at a fixed rate and shows a new frame only on a
/// refresh. If frames arrived at an unrelated rate, some would stay on screen
/// for one refresh and others for two, and motion would stutter. So Juglar
/// draws on every refresh, or every second, third… one: whichever whole
/// number lands closest to the target rate.
///
/// Vsync alone can't be trusted to set the pace: GPU drivers let users force
/// it off (common on gaming PCs), and then a frame loop with no timer of its
/// own runs as fast as it can, at a thousand frames per second or more.
struct Pacer {
    period: Duration,
    /// When the next frame is due.
    next: Instant,
}

impl Pacer {
    fn new(refresh_hz: Option<f64>, target_fps: f64) -> Self {
        let fps = match refresh_hz {
            Some(hz) => hz / (hz / target_fps).round().max(1.0),
            None => target_fps,
        };
        Self { period: Duration::from_secs_f64(1.0 / fps), next: Instant::now() }
    }

    fn fps(&self) -> f64 {
        1.0 / self.period.as_secs_f64()
    }

    fn frame_done(&mut self, started: Instant, drawn: bool) {
        self.next = if drawn {
            // Advance from the previous deadline, not from now, so small
            // delays don't add up into a slower rate. After a long stall,
            // start over instead of rushing to catch up.
            let next = self.next + self.period;
            if next < started { started + self.period } else { next }
        } else {
            Instant::now() + HIDDEN_RETRY
        };
    }
}

/// Per-second numbers for the window title, until the HUD exists (M7).
/// Of the beats detected while the tempo is known, how many fall within a
/// tenth of a beat of where the count puts the beat: whether the tempo
/// tracker counts along with the music.
#[derive(Default)]
struct Alignment {
    last_beat: f32,
    on: u32,
    all: u32,
}

impl Alignment {
    fn frame(&mut self, s: &Signals) {
        if s.beat > 0.5 && s.beat > self.last_beat + 0.3 && s.tempo > 0.0 {
            self.all += 1;
            // The phase rises from 0 at the beat, and the detector reports a
            // beat a little after it happens.
            if s.beat_phase.min(1.0 - s.beat_phase) < 0.1 {
                self.on += 1;
            }
        }
        self.last_beat = s.beat;
    }

    /// The last second's count, and a fresh start.
    fn report(&mut self) -> String {
        let text = if self.all > 0 { format!(" · on the count {}/{}", self.on, self.all) } else { String::new() };
        (self.on, self.all) = (0, 0);
        text
    }
}

struct Stats {
    /// Seconds reported since the start.
    seconds: u64,
    since: Instant,
    frames: u32,
    shortest: f32,
    longest: f32,
    analysis: Duration,
    /// Wall-clock time spent recording, submitting and presenting frames.
    draw: Duration,
    gpu_ms: Vec<f32>,
    /// The beat, kick, snare and cymbal counts when this second started.
    hits: Option<[u64; 4]>,
}

impl Stats {
    fn new() -> Self {
        Self { seconds: 0, since: Instant::now(), frames: 0, shortest: f32::MAX, longest: 0.0, analysis: Duration::ZERO, draw: Duration::ZERO, gpu_ms: Vec::new(), hits: None }
    }

    fn frame(&mut self, dt: f32, analysis: Duration, draw: Duration) {
        self.frames += 1;
        self.shortest = self.shortest.min(dt);
        self.longest = self.longest.max(dt);
        self.analysis += analysis;
        self.draw += draw;
    }

    /// Once a second, the title summarizing it.
    fn report(&mut self, now: Instant, hits: Option<[u64; 4]>) -> Option<String> {
        let elapsed = (now - self.since).as_secs_f32();
        if elapsed < 1.0 {
            return None;
        }
        let mut title = format!("Juglar · {:.0} fps", self.frames as f32 / elapsed);
        if self.frames > 0 {
            title += &format!(
                " · frame {:.1}–{:.1} ms · dsp {:.3} ms · draw {:.2} ms",
                self.shortest * 1000.0,
                self.longest * 1000.0,
                self.analysis.as_secs_f32() * 1000.0 / self.frames as f32,
                self.draw.as_secs_f32() * 1000.0 / self.frames as f32,
            );
        }
        if !self.gpu_ms.is_empty() {
            let gpu = self.gpu_ms.iter().sum::<f32>() / self.gpu_ms.len() as f32;
            title += &format!(" · gpu {gpu:.3} ms");
        }
        if let (Some(start), Some(end)) = (self.hits, hits) {
            let [beats, kick, snare, cymbals] = std::array::from_fn(|i| end[i].saturating_sub(start[i]) as f32 / elapsed);
            title += &format!(" · beats {beats:.1}/s · kick {kick:.1} snare {snare:.1} cymbals {cymbals:.1}/s");
        }
        let seconds = self.seconds + 1;
        *self = Self::new();
        self.seconds = seconds;
        self.since = now;
        self.hits = hits;
        Some(title)
    }
}
