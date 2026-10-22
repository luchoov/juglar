//! Capture what the system is playing, turn it into `Signals`, and draw them
//! in a window on the GPU. With `JUGLAR_DEMO=1` the signals come from a
//! synthetic song instead, and nothing is captured.

use std::sync::Arc;
use std::time::{Duration, Instant};

use juglar_dsp::{Analyzer, Consumer, FFT_SIZE, SCOPE_POINTS, Scope, ring};
use juglar_platform::{AudioCapture, PlatformError, SystemAudio};
use juglar_render::{Renderer, View};
use juglar_signals::{Signals, Synthetic};
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::{ElementState, KeyEvent, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{Key, NamedKey};
use winit::window::{Window, WindowId};

const _: () = assert!(juglar_dsp::SPECTRUM_BANDS == juglar_signals::SPECTRUM_BANDS);
const _: () = assert!(SCOPE_POINTS == juglar_signals::WAVEFORM_POINTS);

/// Frames per second Juglar aims for. The real rate is the display's refresh
/// divided by a whole number (see `Pacer`), as close to this as possible.
const TARGET_FPS: f64 = 60.0;
/// How often to try again while the window is minimized or hidden.
const HIDDEN_RETRY: Duration = Duration::from_millis(100);
/// Intensity presets on keys 1, 2 and 3: gentle, medium, strong.
const INTENSITY_PRESETS: [f32; 3] = [0.2, 0.5, 0.9];

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
    let intensity = std::env::var("JUGLAR_INTENSITY").ok().and_then(|v| v.parse().ok()).unwrap_or(INTENSITY_PRESETS[1]);
    let mut app = App {
        source,
        signals: Signals { intensity: f32::clamp(intensity, 0.0, 1.0), ..Signals::default() },
        window: None,
        renderer: None,
        pacer: Pacer::new(None, TARGET_FPS),
        display_hz: None,
        print_stats: std::env::var("JUGLAR_STATS").is_ok_and(|v| v != "0"),
        last_frame: Instant::now(),
        stats: Stats::new(),
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

    /// Drains the new audio and analyzes it. Beat and novelty stay 0 until
    /// Milestone 5 detects them.
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
        out.spectrum = analysis.spectrum;
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
        self.follow_display();

        let mut renderer = renderer;
        renderer.set_reduced_motion(reduced_motion());
        if reduced_motion() {
            println!("Reduced motion is on: the picture stays still.");
        }
        self.renderer = Some(renderer);
        self.last_frame = Instant::now();
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested
            | WindowEvent::KeyboardInput {
                event: KeyEvent { logical_key: Key::Named(NamedKey::Escape), state: ElementState::Pressed, .. },
                ..
            } => event_loop.exit(),
            WindowEvent::KeyboardInput { event: KeyEvent { logical_key, state: ElementState::Pressed, repeat: false, .. }, .. } => {
                self.key(&logical_key)
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

    /// Tab switches between the scene and the analysis view; 1, 2 and 3
    /// set the intensity to gentle, medium or strong.
    fn key(&mut self, key: &Key) {
        match key {
            Key::Named(NamedKey::Tab) => {
                if let Some(renderer) = &mut self.renderer {
                    let next = if renderer.view() == View::Scene { View::Analysis } else { View::Scene };
                    renderer.set_view(next);
                }
            }
            Key::Character(c) => {
                if let Some(preset) = c.parse::<usize>().ok().and_then(|n| INTENSITY_PRESETS.get(n.wrapping_sub(1))) {
                    self.signals.intensity = *preset;
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
        let analysis_time = work.elapsed();

        let Some(renderer) = &mut self.renderer else { return };
        // A minimized window can't be seen, but DirectX 12 still accepts its
        // frames, so drawing them would cost as much as drawing visible ones.
        let minimized = self.window.as_ref().and_then(|w| w.is_minimized()).unwrap_or(false);
        let work = Instant::now();
        let drawn = !minimized && renderer.render(&self.signals);
        let draw_time = work.elapsed();
        self.pacer.frame_done(now, drawn);
        if let Some(ms) = renderer.take_gpu_ms() {
            self.stats.gpu_ms.push(ms);
        }
        if drawn {
            self.stats.frame(dt, analysis_time, draw_time);
        }
        if let Some(title) = self.stats.report(now) {
            // Once a second: cheap enough to follow the OS setting live.
            renderer.set_reduced_motion(reduced_motion());
            if self.print_stats {
                println!("{title}");
            }
            if let Some(window) = &self.window {
                window.set_title(&title);
            }
        }
    }
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
struct Stats {
    since: Instant,
    frames: u32,
    shortest: f32,
    longest: f32,
    analysis: Duration,
    /// Wall-clock time spent recording, submitting and presenting frames.
    draw: Duration,
    gpu_ms: Vec<f32>,
}

impl Stats {
    fn new() -> Self {
        Self { since: Instant::now(), frames: 0, shortest: f32::MAX, longest: 0.0, analysis: Duration::ZERO, draw: Duration::ZERO, gpu_ms: Vec::new() }
    }

    fn frame(&mut self, dt: f32, analysis: Duration, draw: Duration) {
        self.frames += 1;
        self.shortest = self.shortest.min(dt);
        self.longest = self.longest.max(dt);
        self.analysis += analysis;
        self.draw += draw;
    }

    /// Once a second, the title summarizing it.
    fn report(&mut self, now: Instant) -> Option<String> {
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
        *self = Self::new();
        self.since = now;
        Some(title)
    }
}
