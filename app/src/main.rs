//! Capture what the system is playing, turn it into `Signals`, and draw them
//! in a window on the GPU. With `JUGLAR_DEMO=1` the signals come from a
//! synthetic song instead, and nothing is captured.

use std::sync::Arc;
use std::time::{Duration, Instant};

use juglar_director::{Director, Intensity, Mode, Style, Transition};
use juglar_dsp::{Analyzer, Consumer, FFT_SIZE, SCOPE_POINTS, Scope, ring};
use juglar_platform::{
    AudioCapture, PlatformError, ScreenRect, SystemAudio, Tray, TrayChoice, TrayState, Wallpaper, desktop_covered,
};
use juglar_render::{Renderer, View};
use juglar_signals::{Relative, Signals, Synthetic};
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::{ElementState, KeyEvent, MouseButton, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::dpi::{PhysicalPosition, PhysicalSize};
use winit::keyboard::{Key, ModifiersState, NamedKey};
use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
use winit::window::{Fullscreen, Window, WindowId};

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

fn main() {
    let source = match std::env::var("JUGLAR_DEMO") {
        Ok(v) if v != "0" => {
            let start = std::env::var("JUGLAR_DEMO_START").ok().and_then(|v| v.parse().ok()).unwrap_or(0.0);
            println!("Demo mode: a synthetic song, nothing is captured.");
            Source::Demo(Synthetic::starting_at(1, start))
        }
        _ => match Listening::start() {
            Ok(listening) => Source::Audio(Box::new(listening)),
            Err(e) => {
                eprintln!("juglar: could not capture system audio: {e}");
                std::process::exit(1);
            }
        },
    };

    let event_loop = EventLoop::new().expect("event loop");
    // Natural intensity unless `JUGLAR_INTENSITY` fixes one.
    let intensity = match std::env::var("JUGLAR_INTENSITY").ok().and_then(|v| v.parse::<f32>().ok()) {
        Some(value) => Intensity::Fixed(value.clamp(0.0, 1.0)),
        None => Intensity::Natural,
    };
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
        start_fullscreen: std::env::args().any(|arg| arg == "--fullscreen"),
        start_wallpaper: std::env::args().any(|arg| arg == "--wallpaper"),
        wallpaper: None,
        tray: None,
        covered: false,
        modifiers: ModifiersState::empty(),
        last_click: None,
        pointer_moved: Instant::now(),
        alignment: Alignment::default(),
        still: false,
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
}

impl Source {
    fn update(&mut self, dt: f32, out: &mut Signals) {
        match self {
            Source::Audio(listening) => listening.update(dt, out),
            Source::Demo(song) => song.next(dt, out),
        }
    }

    /// Beats, kicks, snares and cymbals heard since the start; the
    /// synthetic song's are known, not heard.
    fn hits(&self) -> Option<[u64; 4]> {
        match self {
            Source::Audio(listening) => {
                let [kick, snare, cymbals] = listening.analyzer.drum_hits();
                Some([listening.analyzer.beats(), kick, snare, cymbals])
            }
            Source::Demo(_) => None,
        }
    }
}

/// System audio capture and its analysis.
struct Listening {
    /// Kept alive for as long as the app runs; capture stops when dropped.
    _capture: SystemAudio,
    consumer: Consumer,
    analyzer: Analyzer,
    scope: Scope,
    block: Vec<f32>,
    recent: Vec<f32>,
}

impl Listening {
    fn start() -> Result<Self, PlatformError> {
        // One second of mono audio at up to 192 kHz. Every frame drains it,
        // so it never comes close to filling.
        let (mut producer, consumer) = ring(192_000);
        let mut mono = Vec::new();
        let capture = SystemAudio::start(Box::new(move |interleaved, channels| {
            downmix(interleaved, usize::from(channels), &mut mono);
            producer.push(&mono);
        }))?;
        let info = capture.info();
        println!("Capturing {} Hz, {} channels.", info.sample_rate, info.channels);
        Ok(Self {
            _capture: capture,
            consumer,
            analyzer: Analyzer::new(info.sample_rate),
            scope: Scope::new(info.sample_rate),
            block: vec![0.0; info.sample_rate as usize],
            recent: vec![0.0; FFT_SIZE],
        })
    }

    /// Drains the new audio and analyzes it.
    fn update(&mut self, dt: f32, out: &mut Signals) {
        let n = self.consumer.pop(&mut self.block);
        self.analyzer.push(&self.block[..n]);
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
    /// While Juglar is the wallpaper: the window's place in the desktop,
    /// the display it covers, and where it was as a window.
    wallpaper: Option<(Wallpaper, ScreenRect, ScreenRect)>,
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
    /// Whether reduced motion is on, as last read (once a second).
    still: bool,
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let attributes = Window::default_attributes()
            .with_title("Juglar")
            .with_inner_size(LogicalSize::new(960.0, 540.0))
            .with_min_inner_size(LogicalSize::new(320.0, 180.0));
        let window = Arc::new(event_loop.create_window(attributes).expect("window"));
        let size = window.inner_size();
        let renderer = match pollster::block_on(Renderer::new(window.clone(), size.width, size.height)) {
            Ok(renderer) => renderer,
            Err(e) => {
                eprintln!("juglar: could not start the GPU: {e}");
                event_loop.exit();
                return;
            }
        };
        println!("GPU: {}", renderer.adapter_name());
        self.window = Some(window);
        if self.start_fullscreen {
            self.toggle_fullscreen();
        }
        self.follow_display();

        let mut renderer = renderer;
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
        self.still = reduced_motion();
        renderer.set_reduced_motion(self.still);
        if self.still {
            println!("Reduced motion is on: the picture stays still.");
        }
        // `JUGLAR_MODE=focus` (or `relax`) starts in a calm mode.
        match std::env::var("JUGLAR_MODE").as_deref() {
            Ok("focus") => self.set_mode(Mode::Focus, &mut renderer),
            Ok("relax") => self.set_mode(Mode::Relax, &mut renderer),
            Ok("lively") | Err(_) => {}
            Ok(other) => eprintln!("juglar: no mode called {other:?}; there are lively, focus and relax"),
        }
        self.renderer = Some(renderer);
        self.last_frame = Instant::now();
        if self.start_wallpaper {
            self.enter_wallpaper();
        }
    }

    /// Leaves the desktop as it was: the window comes out of it before it
    /// is destroyed.
    fn exiting(&mut self, _event_loop: &ActiveEventLoop) {
        self.wallpaper = None;
        self.tray = None;
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            // Escape leaves fullscreen first, and closes from a window.
            WindowEvent::KeyboardInput {
                event: KeyEvent { logical_key: Key::Named(NamedKey::Escape), state: ElementState::Pressed, .. },
                ..
            } => {
                if self.is_fullscreen() {
                    self.toggle_fullscreen();
                } else {
                    event_loop.exit();
                }
            }
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
                TrayChoice::NextScene => self.key(&Key::Named(NamedKey::ArrowRight)),
                TrayChoice::KeepScene => self.key(&Key::Named(NamedKey::Space)),
                TrayChoice::NextMode => self.key(&Key::Character("m".into())),
                TrayChoice::Window => self.leave_wallpaper(),
                TrayChoice::Quit => event_loop.exit(),
            }
        }
        let Some(window) = &self.window else { return };
        if Instant::now() >= self.pacer.next {
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

    /// Moves the window behind the desktop icons, over the display it is
    /// on, with an icon in the notification area to control it.
    fn enter_wallpaper(&mut self) {
        if self.wallpaper.is_some() {
            return;
        }
        if self.is_fullscreen() {
            self.toggle_fullscreen();
        }
        let Some(window) = &self.window else { return };
        let Some(monitor) = window.current_monitor() else { return };
        let display = rect(monitor.position(), monitor.size());
        let windowed = rect(window.outer_position().unwrap_or_default(), window.outer_size());
        let Ok(handle) = window.window_handle() else { return };
        let RawWindowHandle::Win32(handle) = handle.as_raw() else { return };
        match Wallpaper::attach(handle.hwnd.get(), display) {
            Ok(wallpaper) => {
                self.wallpaper = Some((wallpaper, display, windowed));
                if self.tray.is_none() {
                    match Tray::new("Juglar") {
                        Ok(tray) => self.tray = Some(tray),
                        Err(e) => eprintln!("juglar: no tray icon ({e}); quit from the Task Manager"),
                    }
                }
                println!("Wallpaper mode: drawing behind the desktop icons. The tray icon has the controls.");
            }
            Err(e) => eprintln!("juglar: could not become the wallpaper: {e}"),
        }
    }

    /// Back to an ordinary window, where it was before.
    fn leave_wallpaper(&mut self) {
        if let Some((wallpaper, _, windowed)) = self.wallpaper.take() {
            wallpaper.restore(windowed);
        }
        self.tray = None;
        self.covered = false;
        if let Some(window) = &self.window {
            window.focus_window();
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
    /// F11 and Alt+Enter switch fullscreen, W makes Juglar the wallpaper.
    fn key(&mut self, key: &Key) {
        match key {
            Key::Named(NamedKey::F11) => self.toggle_fullscreen(),
            Key::Named(NamedKey::Enter) if self.modifiers.alt_key() => self.toggle_fullscreen(),
            Key::Named(step @ (NamedKey::ArrowLeft | NamedKey::ArrowRight)) => {
                if let Some(renderer) = &mut self.renderer {
                    let count = renderer.scene_names().len();
                    let back = *step == NamedKey::ArrowLeft;
                    let next = if back { renderer.scene() + count - 1 } else { renderer.scene() + 1 };
                    // With reduced motion, a slow fade through patches
                    // instead of an edge sweeping across.
                    let transition = if self.still {
                        Transition { style: Style::Seep, seconds: 3.0, per_beat: 0.0 }
                    } else {
                        Transition::asked()
                    };
                    renderer.begin_transition(next, transition);
                    self.director.chosen(renderer.scene());
                }
            }
            Key::Named(NamedKey::Space) => {
                let holding = !self.director.holding();
                self.director.hold(holding);
            }
            Key::Named(NamedKey::Tab) => {
                if let Some(renderer) = &mut self.renderer {
                    let next = if renderer.view() == View::Scene { View::Analysis } else { View::Scene };
                    renderer.set_view(next);
                }
            }
            Key::Character(c) if c.eq_ignore_ascii_case("w") => self.enter_wallpaper(),
            Key::Character(c) if c.eq_ignore_ascii_case("m") => {
                if let Some(mut renderer) = self.renderer.take() {
                    self.set_mode(self.director.mode().next(), &mut renderer);
                    self.renderer = Some(renderer);
                }
            }
            Key::Character(c) if c.eq_ignore_ascii_case("a") => {
                self.director.set_intensity(Intensity::Natural, self.signals.intensity);
            }
            Key::Character(c) => {
                if let Some(&preset) = c.parse::<usize>().ok().and_then(|n| INTENSITY_PRESETS.get(n.wrapping_sub(1))) {
                    self.director.set_intensity(Intensity::Fixed(preset), self.signals.intensity);
                }
            }
            _ => {}
        }
    }

    fn frame(&mut self) {
        let now = Instant::now();
        let dt = (now - self.last_frame).as_secs_f32();
        self.last_frame = now;

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
        let change = self.director.update(&mut self.signals, scene, busy);
        let analysis_time = work.elapsed();

        let Some(renderer) = &mut self.renderer else { return };
        if let Some(change) = change {
            renderer.begin_transition(change.scene, change.transition);
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
        let draw_time = work.elapsed();
        self.pacer.frame_done(now, drawn);
        if let Some(ms) = renderer.take_gpu_ms() {
            self.stats.gpu_ms.push(ms);
        }
        if drawn {
            self.stats.frame(dt, analysis_time, draw_time);
        }
        if let Some(stats) = self.stats.report(now, self.source.hits()) {
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
            if let Some((_, display, _)) = &self.wallpaper {
                self.covered = desktop_covered(*display);
            }
            if let Some(tray) = &self.tray {
                tray.set_state(TrayState { keeping: self.director.holding(), mode: self.director.mode().name().into() });
            }
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
            if let Some(window) = &self.window {
                window.set_title(&title);
            }
        }
    }
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
        Self { since: Instant::now(), frames: 0, shortest: f32::MAX, longest: 0.0, analysis: Duration::ZERO, draw: Duration::ZERO, gpu_ms: Vec::new(), hits: None }
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
            let [beats, kick, snare, cymbals] = std::array::from_fn(|i| (end[i] - start[i]) as f32 / elapsed);
            title += &format!(" · beats {beats:.1}/s · kick {kick:.1} snare {snare:.1} cymbals {cymbals:.1}/s");
        }
        *self = Self::new();
        self.since = now;
        self.hits = hits;
        Some(title)
    }
}
