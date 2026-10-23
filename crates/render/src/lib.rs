//! GPU rendering for Juglar.
//!
//! Plain Rust on top of `wgpu`, which speaks DirectX 12, Vulkan, Metal or
//! WebGPU depending on where it runs. Nothing here calls the operating system
//! or a windowing library: the app hands over anything that has a window
//! handle, then each frame's `Signals`, a few hundred plain numbers.
//!
//! A frame is four steps on the GPU (see `docs/theory/05-feedback.md`):
//!
//! 1. **Feedback** (`feedback.wgsl`), on a half-size image: copy the last
//!    picture, warped and faded, and let the scene paint on top.
//! 2. **Bloom** (`bloom.rs`): the bright parts, blurred ever wider.
//! 3. **Composite** (`composite.wgsl`), at full size: turn that picture's
//!    ink into colors, tone-map them and add the glow.
//! 4. **Present** (`present.rs`): dither the frame onto the screen. Just
//!    before, any text (`hud.rs`, drawn by `ink.rs`) goes over the frame.
//!    Nothing is held back: flashes and hard cuts are the scenes' to make
//!    (D48 in `docs/decisions.md`).
//!
//! The analysis view (`ear.rs`) replaces the first three with what Juglar
//! hears, drawn by `ink.rs`: the signals over the last seconds, labeled,
//! and when and why the director changed scene.
//!
//! During a transition between scenes (`begin_transition`) the first three
//! steps run twice, once per scene, each in its own feedback loop, and the
//! composite step draws the old scene over the new one through the
//! transition's shape (`fs_fade` in `composite.wgsl`).

mod bloom;
mod ear;
mod feedback;
mod hud;
mod ink;
mod intro;
mod menu;
mod present;
mod scene;
mod timer;

use std::fmt;
use std::num::NonZeroU64;
use std::sync::mpsc;

use bytemuck::{Pod, Zeroable};
use bloom::{Bloom, Glow};
use ear::Ear;
use feedback::Feedback;
use hud::Hud;
use ink::Ink;
use intro::Intro;
use menu::MenuView;
use present::Present;
use scene::{Look, Scene};
use timer::GpuTimer;

pub use juglar_director::{Calm, Character, Style, Transition};
pub use juglar_signals::{SPECTRUM_BANDS, Signals, WAVEFORM_POINTS};
pub use intro::Song;
pub use menu::{Hit, Menu, Row};

/// The largest cover `set_cover` takes, in pixels a side.
pub const COVER_SIDE: u32 = ink::PICTURE;

const COMMON: &str = include_str!("common.wgsl");

/// What the window shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum View {
    /// The visuals.
    Scene,
    /// What Juglar hears: waveform, spectrum and band levels.
    Analysis,
}

/// The frame's numbers laid out exactly as the shaders' `Frame` struct (in
/// `common.wgsl`) reads them: GPU uniform memory works in 16-byte rows, so
/// arrays are packed four floats to a row and some rows end in padding.
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Uniforms {
    resolution: [f32; 2],
    feedback_size: [f32; 2],
    time: f32,
    dt: f32,
    bloom: f32,
    seed: f32,
    center: [f32; 2],
    zoom: f32,
    rotation: f32,
    swirl: f32,
    decay: f32,
    palette_drift: f32,
    palette_wrap: f32,
    knobs: [[f32; 4]; 2],
    palette_a: [f32; 4],
    palette_b: [f32; 4],
    palette_c: [f32; 4],
    palette_d: [f32; 4],
    palette_shift: f32,
    exposure: f32,
    beat: f32,
    novelty: f32,
    bands: [f32; 4],
    spectrum: [[f32; 4]; SPECTRUM_BANDS / 4],
    waveform: [[f32; 4]; WAVEFORM_POINTS / 4],
    /// The transition: progress, style, edge softness, seed.
    reveal: [f32; 4],
    /// A calm mode: saturation, floor, ceiling, how far it has come in.
    calm: [f32; 4],
    /// Kick, snare and cymbals; the percussive and harmonic levels.
    drums: [f32; 4],
    layers: [f32; 4],
    /// The scene's screen effects, and chaos, impact, hit and loudness.
    fx: [[f32; 4]; 3],
    heavy: [f32; 4],
    /// How the old picture works on itself: sharpen, cap, erode, reach.
    material: [f32; 4],
}

/// Seconds a calm mode takes to fade in or out.
const CALM_FADE: f32 = 3.0;

/// A frame read back from the screen, 8-bit RGB, rows top to bottom.
pub struct Capture {
    pub width: u32,
    pub height: u32,
    pub rgb: Vec<u8>,
}

#[derive(Debug)]
pub struct RenderError(String);

impl RenderError {
    fn new(what: &str, cause: impl fmt::Display) -> Self {
        Self(format!("{what}: {cause}"))
    }
}

impl fmt::Display for RenderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for RenderError {}

/// What each scene is like, in the order the renderer has them, for the
/// director to match them with music. Needs no GPU.
pub fn scene_characters() -> Vec<Character> {
    scene::all().iter().map(|scene| scene.character()).collect()
}

/// Which GPU and graphics API to prefer; lower is better.
///
/// The integrated GPU comes first: Juglar's work is tiny, and the dedicated
/// one stays free for whatever game runs next to it. Among APIs, DirectX 12
/// first on Windows: measured on a GTX 1060, the whole process used about a
/// fifth less CPU and 20 MB less memory than with Vulkan, whose driver does
/// part of the work on a thread of its own. Vulkan is the fallback there, and
/// the only choice on Linux; Metal and WebGPU are the only ones elsewhere.
fn adapter_rank(device: wgpu::DeviceType, backend: wgpu::Backend) -> (u8, u8) {
    let device = match device {
        wgpu::DeviceType::IntegratedGpu => 0,
        wgpu::DeviceType::DiscreteGpu => 1,
        wgpu::DeviceType::Other | wgpu::DeviceType::VirtualGpu => 2,
        // A software renderer would burn the CPU this project exists to save.
        wgpu::DeviceType::Cpu => 3,
    };
    let backend = match backend {
        wgpu::Backend::Dx12 => 0,
        wgpu::Backend::Vulkan | wgpu::Backend::Metal | wgpu::Backend::BrowserWebGpu => 1,
        _ => 2,
    };
    (device, backend)
}

/// One picture being drawn: its frame numbers, feedback images and glow.
/// There are two, so that during a transition the old scene and the new
/// one each keep a loop of their own.
struct Layer {
    uniforms: wgpu::Buffer,
    feedback: Feedback,
    glow: Glow,
    /// Which scene this layer draws.
    scene: usize,
}

/// A transition under way, from the front layer's scene to the other's.
struct Crossing {
    transition: Transition,
    /// 0 at the start, 1 when only the new scene is left.
    progress: f32,
    /// Makes each transition's edge different.
    seed: f32,
    /// Whether the new scene has been handed the old picture yet.
    handed_over: bool,
}

pub struct Renderer {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    /// Uniforms plus an image and how to filter it: feedback and composite.
    image_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    scenes: Vec<Box<dyn Scene>>,
    /// Each scene's pipelines, its WGSL compiled into each; `None` until
    /// they are compiled.
    pipelines: Vec<Option<ScenePipelines>>,
    factory: PipelineFactory,
    /// Pipelines compiled by the background thread, as they are ready.
    compiled: Option<mpsc::Receiver<(usize, ScenePipelines)>>,
    layers: [Layer; 2],
    /// The layer whose scene is showing (the old one, during a transition).
    front: usize,
    crossing: Option<Crossing>,
    /// The scene asked for while a transition was under way, and how to
    /// get there: it starts when that one ends.
    pending: Option<(usize, Transition)>,
    /// How many transitions have started, for their seeds.
    crossings: u32,
    bloom: Bloom,
    present: Present,
    ink: Ink,
    hud: Hud,
    ear: Ear,
    intro: Intro,
    menu: MenuView,
    view: View,
    reduced_motion: bool,
    /// The calm mode asked for, or the last one while it fades out.
    calm: Calm,
    calm_on: bool,
    /// How far the calm mode has faded in, 0..1.
    calm_amount: f32,
    frames: u32,
    encode_srgb: bool,
    timer: Option<GpuTimer>,
    adapter: wgpu::AdapterInfo,
    /// Whether each frame is read back (`enable_capture`), and the last.
    capturing: bool,
    captured: Option<Capture>,
}

impl Renderer {
    /// Opens the GPU and prepares to draw into `target` (a window, or a canvas
    /// on the web) of `width` × `height` pixels.
    pub async fn new(
        target: impl Into<wgpu::SurfaceTarget<'static>>,
        width: u32,
        height: u32,
    ) -> Result<Self, RenderError> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::PRIMARY,
            ..wgpu::InstanceDescriptor::new_without_display_handle()
        });
        let surface = instance.create_surface(target).map_err(|e| RenderError::new("window surface", e))?;

        let adapter = instance
            .enumerate_adapters(wgpu::Backends::PRIMARY)
            .await
            .into_iter()
            .filter(|adapter| adapter.is_surface_supported(&surface))
            // `JUGLAR_GPU_API=vulkan` (or `dx12`, `metal`) keeps only that
            // API, to compare their costs until there is a settings file.
            .filter(|adapter| {
                std::env::var("JUGLAR_GPU_API")
                    .map_or(true, |api| adapter.get_info().backend.to_str().eq_ignore_ascii_case(&api))
            })
            .min_by_key(|adapter| {
                let info = adapter.get_info();
                adapter_rank(info.device_type, info.backend)
            })
            .ok_or_else(|| RenderError("no GPU can draw to this window".into()))?;

        // Timestamps let us measure GPU time; not every GPU or browser has them.
        let timestamps = adapter.features().contains(wgpu::Features::TIMESTAMP_QUERY);
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("juglar"),
                required_features: if timestamps { wgpu::Features::TIMESTAMP_QUERY } else { wgpu::Features::empty() },
                required_limits: wgpu::Limits::default(),
                memory_hints: wgpu::MemoryHints::MemoryUsage,
                ..Default::default()
            })
            .await
            .map_err(|e| RenderError::new("GPU device", e))?;

        // Colors are blended in linear light and stored as sRGB, the encoding
        // screens expect. Most native surfaces offer an sRGB format that
        // encodes for free; browsers usually don't, so then the shader does it.
        let caps = surface.get_capabilities(&adapter);
        let format = caps.formats.iter().copied().find(|f| f.is_srgb()).unwrap_or(caps.formats[0]);
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            color_space: wgpu::SurfaceColorSpace::Auto,
            width: width.max(1),
            height: height.max(1),
            // Ask to wait for the display's refresh instead of tearing, and
            // keep at most one frame queued so what you see is fresh. Drivers
            // may ignore the first (gamers often force vsync off), so the app
            // paces frames itself rather than relying on it.
            present_mode: wgpu::PresentMode::AutoVsync,
            desired_maximum_frame_latency: 1,
            alpha_mode: caps.alpha_modes[0],
            view_formats: vec![],
        };
        surface.configure(&device, &config);

        let uniform_size = size_of::<Uniforms>() as u64;
        let uniform_entry = wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: NonZeroU64::new(uniform_size),
            },
            count: None,
        };
        let image_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("image"),
            entries: &[
                uniform_entry,
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        // Blends the four nearest pixels; at the edges, repeats the border.
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("smooth"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        let bloom = Bloom::new(&device, &image_layout, COMMON);
        let factory = PipelineFactory::new(&device, &image_layout, bloom.apply_layout());

        // Only the first scene is compiled before the first frame; a thread
        // compiles the rest while it shows (see `PipelineFactory`).
        let scenes = scene::all();
        let shaders: Vec<&'static str> = scenes.iter().map(|scene| scene.shader()).collect();
        let mut pipelines: Vec<Option<ScenePipelines>> = scenes.iter().map(|_| None).collect();
        pipelines[0] = Some(factory.scene(shaders[0]));
        let compiled = factory.compile_in_background(shaders[1..].iter().copied().enumerate().map(|(i, shader)| (i + 1, shader)).collect());

        let layers = std::array::from_fn(|_| {
            let uniforms = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("frame uniforms"),
                size: uniform_size,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            Layer {
                feedback: Feedback::new(&device, &image_layout, &uniforms, &sampler, config.width, config.height),
                glow: bloom.glow(&device, &image_layout, &uniforms, &sampler, config.width, config.height),
                uniforms,
                scene: 0,
            }
        });
        let present = Present::new(&device, config.width, config.height, format);
        let ink = Ink::new(&device);
        let timer = timestamps.then(|| GpuTimer::new(&device, queue.get_timestamp_period()));

        Ok(Self {
            surface,
            device,
            queue,
            config,
            image_layout,
            sampler,
            scenes,
            pipelines,
            factory,
            compiled,
            layers,
            front: 0,
            crossing: None,
            pending: None,
            crossings: 0,
            bloom,
            present,
            ink,
            hud: Hud::default(),
            ear: Ear::default(),
            intro: Intro::default(),
            menu: MenuView::default(),
            view: View::Scene,
            reduced_motion: false,
            calm: Calm { motion: 1.0, saturation: 1.0, floor: 0.0, ceiling: 1.0 },
            calm_on: false,
            calm_amount: 0.0,
            frames: 0,
            encode_srgb: !format.is_srgb(),
            timer,
            adapter: adapter.get_info(),
            capturing: false,
            captured: None,
        })
    }

    /// Reads every frame back from the screen from now on, for recording
    /// (`take_capture`). It waits for the GPU each frame: only for making
    /// videos, never while running live.
    pub fn enable_capture(&mut self) {
        self.capturing = true;
        self.config.usage |= wgpu::TextureUsages::COPY_SRC;
        self.surface.configure(&self.device, &self.config);
    }

    /// The last frame drawn, if capturing.
    pub fn take_capture(&mut self) -> Option<Capture> {
        self.captured.take()
    }

    /// The GPU in use and the graphics API spoken to it, e.g. "Intel UHD 620 (Dx12)".
    pub fn adapter_name(&self) -> String {
        format!("{} ({:?})", self.adapter.name, self.adapter.backend)
    }

    /// Follows a window resize. A zero size (minimized) is ignored. The
    /// feedback images are made anew at the new size, so the picture
    /// rebuilds from black.
    pub fn resize(&mut self, width: u32, height: u32) {
        if width == 0 || height == 0 {
            return;
        }
        self.config.width = width;
        self.config.height = height;
        self.surface.configure(&self.device, &self.config);
        for layer in &mut self.layers {
            layer.feedback = Feedback::new(&self.device, &self.image_layout, &layer.uniforms, &self.sampler, width, height);
            layer.glow = self.bloom.glow(&self.device, &self.image_layout, &layer.uniforms, &self.sampler, width, height);
        }
        self.present.resize(&self.device, width, height);
    }

    /// The names of the scenes, in order.
    pub fn scene_names(&self) -> Vec<&'static str> {
        self.scenes.iter().map(|scene| scene.name()).collect()
    }


    /// Makes sure scene `index`'s pipelines exist: collects what the
    /// background thread has finished, and compiles the scene here if it
    /// hasn't got to it yet (which only happens in the first seconds).
    fn ready(&mut self, index: usize) {
        if let Some(compiled) = &self.compiled {
            for (i, pipelines) in compiled.try_iter() {
                self.pipelines[i].get_or_insert(pipelines);
            }
        }
        if self.pipelines[index].is_none() {
            self.pipelines[index] = Some(self.factory.scene(self.scenes[index].shader()));
        }
    }

    /// Which scene is showing, as an index into `scene_names`; during a
    /// transition, the one it leads to (or the one asked for after it).
    pub fn scene(&self) -> usize {
        match (self.pending, &self.crossing) {
            (Some((index, _)), _) => index,
            (None, Some(_)) => self.layers[1 - self.front].scene,
            (None, None) => self.layers[self.front].scene,
        }
    }

    /// Whether a transition is under way.
    pub fn in_transition(&self) -> bool {
        self.crossing.is_some()
    }

    /// Switches to scene `index` (wrapping around) at once, with no
    /// transition. The picture carries over and the new scene's warp takes
    /// it from there.
    pub fn set_scene(&mut self, index: usize) {
        let index = index % self.scenes.len();
        self.ready(index);
        self.crossing = None;
        self.pending = None;
        self.layers[self.front].scene = index;
    }

    /// Starts a transition to scene `index` (wrapping around). While one is
    /// under way, the new one waits for it to end, and only the last asked
    /// for is kept: cutting the first short would change the whole screen
    /// at once.
    pub fn begin_transition(&mut self, index: usize, transition: Transition) {
        let index = index % self.scenes.len();
        if self.crossing.is_some() {
            let arriving = self.layers[1 - self.front].scene;
            self.pending = (index != arriving).then_some((index, transition));
            return;
        }
        if index == self.layers[self.front].scene {
            return;
        }
        self.ready(index);
        self.layers[1 - self.front].scene = index;
        self.crossings = self.crossings.wrapping_add(1);
        // Seeds far apart, so no two edges look alike.
        let seed = (self.crossings % 64) as f32 * 7.31;
        self.crossing = Some(Crossing { transition, progress: 0.0, seed, handed_over: false });
    }

    pub fn view(&self) -> View {
        self.view
    }

    pub fn set_view(&mut self, view: View) {
        self.view = view;
    }

    /// With reduced motion the scene stands still and stops following the
    /// music: a fixed picture, as the user's OS setting asks.
    pub fn set_reduced_motion(&mut self, reduced: bool) {
        self.reduced_motion = reduced;
    }

    /// A calm mode's look (slower, muted, low in contrast, no camera
    /// flow), or `None` for the scenes as they are. It fades in and out
    /// over `CALM_FADE` seconds: switching the whole screen's contrast at
    /// once would be a flash.
    pub fn set_calm(&mut self, calm: Option<Calm>) {
        if let Some(calm) = calm {
            self.calm = calm;
        }
        self.calm_on = calm.is_some();
    }

    /// Draws one frame. Returns `false` when nothing could be drawn (the
    /// window is minimized or hidden), so the caller can slow down.
    ///
    /// With vsync this may wait until the display is ready for a new frame.
    pub fn render(&mut self, signals: &Signals) -> bool {
        if let Some(timer) = &mut self.timer {
            timer.collect(&self.device);
        }

        let frame = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(frame) => frame,
            wgpu::CurrentSurfaceTexture::Suboptimal(frame) => {
                // Still drawable; reconfigure for the next one.
                self.surface.configure(&self.device, &self.config);
                frame
            }
            wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                self.surface.configure(&self.device, &self.config);
                return false;
            }
            wgpu::CurrentSurfaceTexture::Timeout
            | wgpu::CurrentSurfaceTexture::Occluded
            | wgpu::CurrentSurfaceTexture::Validation => return false,
        };

        // A transition moves on with the clock and the beats; once it is
        // complete, the new scene is the only one left.
        if let Some(crossing) = &mut self.crossing {
            crossing.progress += crossing.transition.step(signals.dt, signals.beat);
            if crossing.progress >= 1.0 {
                self.crossing = None;
                self.front = 1 - self.front;
                if let Some((index, transition)) = self.pending.take() {
                    self.begin_transition(index, transition);
                }
            }
        }

        let fade = signals.dt / CALM_FADE;
        self.calm_amount = (self.calm_amount + if self.calm_on { fade } else { -fade }).clamp(0.0, 1.0);
        let calm = self.calm_amount;

        // Reduced motion: the scenes see silence and no time passing.
        let still;
        let (scene_signals, motion) = if self.reduced_motion {
            still = Signals { dt: signals.dt, intensity: signals.intensity, ..Signals::default() };
            (&still, 0.0)
        } else {
            (signals, 1.0 + (self.calm.motion - 1.0) * calm)
        };
        self.frames = self.frames.wrapping_add(1);
        self.ear.hear(signals);

        // The layers to draw, the showing one first: with a transition, the
        // arriving one too.
        let (front, back) = (self.front, 1 - self.front);
        let reveal = match &self.crossing {
            Some(c) => {
                let (style, soft) = match c.transition.style {
                    Style::Bloom => (0.0, 0.06),
                    Style::Iris => (1.0, 0.06),
                    Style::Seep => (2.0, 0.12),
                    Style::Spiral => (3.0, 0.04),
                };
                [c.progress, style, soft, c.seed]
            }
            None => [0.0; 4],
        };
        let drawn: &[usize] = if self.crossing.is_some() { &[front, back] } else { &[front] };
        let mut looks = Vec::with_capacity(2);
        for &i in drawn {
            let mut look = self.scenes[self.layers[i].scene].update(scene_signals, motion);
            // A calm mode holds the camera still: no zoom, turn or swirl of
            // the whole picture.
            look.zoom = look.zoom.powf(1.0 - calm);
            look.rotation *= 1.0 - calm;
            look.swirl *= 1.0 - calm;
            let uniforms = self.uniforms(scene_signals, &look, reveal);
            self.queue.write_buffer(&self.layers[i].uniforms, 0, bytemuck::bytes_of(&uniforms));
            looks.push(look);
        }

        let screen = frame.texture.create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("frame") });
        let timed = self.timer.as_mut().is_some_and(GpuTimer::begin);
        let timestamps = |first, last| self.timer.as_ref().filter(|_| timed).map(|t| t.pass_writes(first, last));

        match self.view {
            View::Scene => {
                // The arriving scene starts from the old one's picture.
                if let Some(crossing) = &mut self.crossing
                    && !crossing.handed_over
                {
                    self.layers[back].feedback.take_over(&self.layers[front].feedback, &mut encoder);
                    crossing.handed_over = true;
                }
                for (n, (&i, look)) in drawn.iter().zip(&looks).enumerate() {
                    let layer = &mut self.layers[i];
                    let pipelines = self.pipelines[layer.scene].as_ref().expect("compiled when chosen");
                    {
                        // The GPU clock starts with the first pass.
                        let timing = if n == 0 { timestamps(true, false) } else { None };
                        let mut feedback = begin_pass(&mut encoder, "feedback", layer.feedback.next(), timing);
                        feedback.set_pipeline(&pipelines.feedback);
                        feedback.set_bind_group(0, layer.feedback.latest(), &[]);
                        // One triangle large enough to cover the image; see `vs_main`.
                        feedback.draw(0..3, 0..1);
                    }
                    // The image just written is now the latest picture.
                    layer.feedback.swap();
                    // A scene without glow skips the chain; the composite step
                    // multiplies the stale blurs by zero.
                    if look.bloom > 0.0 {
                        self.bloom.encode(&mut encoder, &layer.glow, &pipelines.bloom_source, layer.feedback.latest());
                    }
                }
                let mut composite = begin_pass(&mut encoder, "composite", self.present.candidate(), None);
                // With a transition, the new scene whole, then the old one
                // over it, through the transition's shape.
                for (n, &i) in drawn.iter().rev().enumerate() {
                    let layer = &self.layers[i];
                    let pipelines = self.pipelines[layer.scene].as_ref().expect("compiled when chosen");
                    composite.set_pipeline(if n == 0 { &pipelines.composite } else { &pipelines.fade });
                    composite.set_bind_group(0, layer.feedback.latest(), &[]);
                    composite.set_bind_group(1, layer.glow.apply(), &[]);
                    composite.draw(0..3, 0..1);
                }
            }
            View::Analysis => {
                // A cleared image; `ear.rs` draws the whole view with `ink`.
                begin_pass(&mut encoder, "analysis", self.present.candidate(), timestamps(true, false));
                let name = self.scenes[self.layers[front].scene].name();
                self.ear.draw(&mut self.ink, &self.queue, [self.config.width as f32, self.config.height as f32], signals, name);
            }
        }
        let size = [self.config.width as f32, self.config.height as f32];
        if self.view == View::Scene && !self.menu.showing() {
            self.intro.draw(&mut self.ink, &self.queue, size, signals.dt, signals.beat, self.reduced_motion);
        }
        self.menu.draw(&mut self.ink, &self.queue, size, signals.dt);
        self.hud.draw(&mut self.ink, &self.queue, size, signals.dt);
        self.ink.encode(&self.device, &self.queue, &mut encoder, self.present.candidate(), size);
        let seed = (self.frames % (1 << 24)) as f32;
        self.present.encode(&self.queue, &mut encoder, &screen, seed, self.encode_srgb, timestamps(false, true));
        if timed && let Some(timer) = &self.timer {
            timer.resolve(&mut encoder);
        }
        let readback = self.capturing.then(|| self.copy_out(&mut encoder, &frame.texture));
        self.queue.submit([encoder.finish()]);
        if timed && let Some(timer) = &mut self.timer {
            timer.after_submit();
        }
        self.queue.present(frame);
        if let Some((buffer, row)) = readback {
            self.captured = self.read_back(&buffer, row);
        }
        true
    }

    /// Records a copy of the screen into a buffer that can be read; returns
    /// it and its bytes per row.
    fn copy_out(&self, encoder: &mut wgpu::CommandEncoder, screen: &wgpu::Texture) -> (wgpu::Buffer, u32) {
        let (width, height) = (self.config.width, self.config.height);
        let row = (width * 4).div_ceil(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT) * wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
        let buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("capture"),
            size: u64::from(row * height),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        encoder.copy_texture_to_buffer(
            screen.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(row), rows_per_image: Some(height) },
            },
            wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
        );
        (buffer, row)
    }

    /// Waits for the copy and turns it into RGB.
    fn read_back(&self, buffer: &wgpu::Buffer, row: u32) -> Option<Capture> {
        let (width, height) = (self.config.width, self.config.height);
        let slice = buffer.slice(..);
        let (send, receive) = mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |result| {
            let _ = send.send(result);
        });
        self.device.poll(wgpu::PollType::wait_indefinitely()).ok()?;
        receive.recv().ok()?.ok()?;
        let bgr = matches!(self.config.format, wgpu::TextureFormat::Bgra8Unorm | wgpu::TextureFormat::Bgra8UnormSrgb);
        let data = slice.get_mapped_range().ok()?;
        let mut rgb = Vec::with_capacity((width * height * 3) as usize);
        for y in 0..height {
            let line = &data[(y * row) as usize..(y * row + width * 4) as usize];
            for pixel in line.chunks_exact(4) {
                if bgr {
                    rgb.extend_from_slice(&[pixel[2], pixel[1], pixel[0]]);
                } else {
                    rgb.extend_from_slice(&pixel[..3]);
                }
            }
        }
        Some(Capture { width, height, rgb })
    }

    fn uniforms(&self, s: &Signals, look: &Look, reveal: [f32; 4]) -> Uniforms {
        let (feedback_width, feedback_height) = self.layers[self.front].feedback.size();
        let row = |v: [f32; 3]| [v[0], v[1], v[2], 0.0];
        Uniforms {
            resolution: [self.config.width as f32, self.config.height as f32],
            feedback_size: [feedback_width as f32, feedback_height as f32],
            time: s.time,
            dt: s.dt,
            bloom: look.bloom,
            // Exact as a float up to 2^24 frames, then wraps (~60 hours).
            seed: (self.frames % (1 << 24)) as f32,
            center: look.center,
            zoom: look.zoom,
            rotation: look.rotation,
            swirl: look.swirl,
            decay: look.decay,
            palette_drift: look.palette_drift,
            palette_wrap: if look.palette_wrap { 1.0 } else { 0.0 },
            knobs: bytemuck::cast(look.knobs),
            palette_a: row(look.palette.a),
            palette_b: row(look.palette.b),
            palette_c: row(look.palette.c),
            palette_d: row(look.palette.d),
            palette_shift: look.palette_shift,
            exposure: look.exposure,
            beat: s.beat,
            novelty: s.novelty,
            bands: [s.bass, s.mid, s.treble, s.energy],
            spectrum: bytemuck::cast(look.spectrum),
            waveform: bytemuck::cast(look.waveform),
            reveal,
            calm: [self.calm.saturation, self.calm.floor, self.calm.ceiling, self.calm_amount],
            drums: [s.kick, s.snare, s.cymbals, 0.0],
            layers: [s.percussive, s.harmonic, 0.0, 0.0],
            fx: look.fx.packed(),
            heavy: [s.chaos, s.impact, s.hit, s.loudness],
            material: look.material.packed(),
        }
    }

    /// The fonts for text on screen, as font files: a regular weight, then
    /// a stronger one. Without them no text is drawn.
    pub fn set_fonts(&mut self, fonts: &[Vec<u8>]) {
        self.ink.set_fonts(fonts);
    }

    /// Says on screen, for a moment, that something changed: `label: value`.
    pub fn note(&mut self, label: &str, value: &str) {
        self.hud.note(label, value);
    }

    /// The director changed to `scene` at a moment of the music (`why`),
    /// for the analysis view's record.
    pub fn director_changed(&mut self, why: &str, scene: usize) {
        let name = self.scenes.get(scene).map_or("", |s| s.name());
        self.ear.changed(why, name);
    }

    /// Shows the keys and what each does, or hides them (an empty list).
    pub fn help(&mut self, lines: &[(&str, &str)]) {
        self.hud.help(lines);
    }

    /// Whether the help is showing.
    pub fn helping(&self) -> bool {
        self.hud.helping()
    }

    /// Shows a song's title in the middle of the screen for a few seconds,
    /// with the cover if one is set (`set_cover`).
    pub fn intro(&mut self, song: Song) {
        self.intro.start(song, self.ink.picture_aspect().is_some());
    }

    /// Takes the song's title off the screen at once.
    pub fn end_intro(&mut self) {
        self.intro.stop();
    }

    /// Whether a song's title is showing.
    pub fn in_intro(&self) -> bool {
        self.intro.showing()
    }

    /// The song's cover, straight 8-bit RGBA (`width` × `height`, at most
    /// `COVER_SIDE` a side), for the intro and the menu; `None` for none.
    pub fn set_cover(&mut self, cover: Option<(u32, u32, &[u8])>) {
        self.ink.set_picture(&self.queue, cover);
        if cover.is_some() {
            self.intro.cover_arrived();
        }
    }

    /// Shows the menu over the scene, or (with `None`) hides it.
    pub fn set_menu(&mut self, menu: Option<Menu>) {
        self.menu.set(menu);
    }

    /// Whether the menu is showing.
    pub fn in_menu(&self) -> bool {
        self.menu.showing()
    }

    /// What of the menu lies at `point`, in pixels from the window's top
    /// left, as last drawn.
    pub fn menu_hit(&self, point: [f32; 2]) -> Option<Hit> {
        self.menu.hit(point)
    }

    /// The newest GPU time measurement of a frame, in milliseconds, if one
    /// arrived since the last call. Always `None` without timestamp support.
    pub fn take_gpu_ms(&mut self) -> Option<f32> {
        self.timer.as_mut().and_then(GpuTimer::take)
    }
}

/// A scene's four pipelines.
struct ScenePipelines {
    feedback: wgpu::RenderPipeline,
    /// The scene's bright parts at half size, to start the bloom chain.
    bloom_source: wgpu::RenderPipeline,
    composite: wgpu::RenderPipeline,
    /// The composite step as the old scene of a transition: blended over
    /// the new one, leaving it where the transition has passed.
    fade: wgpu::RenderPipeline,
}

/// Compiles scenes' pipelines, on the main thread or on one of its own.
///
/// Compiling is the slow part of starting: each pipeline turns WGSL into
/// the GPU's own code, about 75 ms apiece on the development PC with
/// DirectX 12, and every scene has four. Compiling all twelve scenes before
/// the first frame kept the window empty for four seconds; compiling only
/// the first, for 1.7–1.9 s. The others follow on a thread while it shows.
#[derive(Clone)]
struct PipelineFactory {
    device: wgpu::Device,
    /// Feedback and the bloom source: frame numbers and one image.
    image: wgpu::PipelineLayout,
    /// The composite step: that, plus the three blurs.
    composite: wgpu::PipelineLayout,
}

impl PipelineFactory {
    fn new(device: &wgpu::Device, image_layout: &wgpu::BindGroupLayout, apply_layout: &wgpu::BindGroupLayout) -> Self {
        let layout = |label, groups: &[&wgpu::BindGroupLayout]| {
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some(label),
                bind_group_layouts: &groups.iter().map(|&group| Some(group)).collect::<Vec<_>>(),
                immediate_size: 0,
            })
        };
        Self {
            device: device.clone(),
            image: layout("image", &[image_layout]),
            composite: layout("composite", &[image_layout, apply_layout]),
        }
    }

    /// The four pipelines of a scene whose WGSL is `shader`.
    fn scene(&self, shader: &str) -> ScenePipelines {
        let device = &self.device;
        let feedback = shader_module(device, "feedback", [COMMON, shader, include_str!("feedback.wgsl")].concat());
        let composite = shader_module(device, "composite", [COMMON, shader, include_str!("composite.wgsl")].concat());
        let candidate = present::CANDIDATE_FORMAT;
        ScenePipelines {
            feedback: render_pipeline(device, "feedback", &self.image, &feedback, "fs_feedback", feedback::FORMAT, None),
            bloom_source: render_pipeline(device, "bloom source", &self.image, &composite, "fs_bloom_source", feedback::FORMAT, None),
            composite: render_pipeline(device, "composite", &self.composite, &composite, "fs_composite", candidate, None),
            fade: render_pipeline(
                device,
                "fade",
                &self.composite,
                &composite,
                "fs_fade",
                candidate,
                Some(wgpu::BlendState::ALPHA_BLENDING),
            ),
        }
    }

    /// On the web there are no threads to spare: scenes compile when chosen.
    #[cfg(target_arch = "wasm32")]
    fn compile_in_background(&self, _scenes: Vec<(usize, &'static str)>) -> Option<mpsc::Receiver<(usize, ScenePipelines)>> {
        None
    }

    /// Compiles each `(index, shader)` in order on a thread of its own,
    /// sending each scene's pipelines as they are ready.
    #[cfg(not(target_arch = "wasm32"))]
    fn compile_in_background(&self, scenes: Vec<(usize, &'static str)>) -> Option<mpsc::Receiver<(usize, ScenePipelines)>> {
        let (send, receive) = mpsc::channel();
        let factory = self.clone();
        let spawned = std::thread::Builder::new().name("juglar shaders".into()).spawn(move || {
            for (index, shader) in scenes {
                // The renderer is gone: nobody needs the rest.
                if send.send((index, factory.scene(shader))).is_err() {
                    return;
                }
            }
        });
        // Without the thread, scenes compile when chosen.
        spawned.ok().map(|_| receive)
    }
}

fn shader_module(device: &wgpu::Device, label: &str, source: String) -> wgpu::ShaderModule {
    device.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some(label), source: wgpu::ShaderSource::Wgsl(source.into()) })
}

/// A pipeline drawing one triangle over the whole target (see `vs_main`)
/// with `entry_point` of `module`.
fn render_pipeline(
    device: &wgpu::Device,
    label: &str,
    layout: &wgpu::PipelineLayout,
    module: &wgpu::ShaderModule,
    entry_point: &str,
    target: wgpu::TextureFormat,
    blend: Option<wgpu::BlendState>,
) -> wgpu::RenderPipeline {
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some(label),
        layout: Some(layout),
        vertex: wgpu::VertexState {
            module,
            entry_point: Some("vs_main"),
            compilation_options: Default::default(),
            buffers: &[],
        },
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        fragment: Some(wgpu::FragmentState {
            module,
            entry_point: Some(entry_point),
            compilation_options: Default::default(),
            targets: &[Some(wgpu::ColorTargetState { format: target, blend, write_mask: wgpu::ColorWrites::ALL })],
        }),
        multiview_mask: None,
        cache: None,
    })
}

/// A pass drawing over all of `target`. Every pass covers every pixel, so
/// the old contents never need loading.
fn begin_pass<'e>(
    encoder: &'e mut wgpu::CommandEncoder,
    label: &str,
    target: &wgpu::TextureView,
    timestamp_writes: Option<wgpu::RenderPassTimestampWrites<'_>>,
) -> wgpu::RenderPass<'e> {
    encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some(label),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view: target,
            depth_slice: None,
            resolve_target: None,
            ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::BLACK), store: wgpu::StoreOp::Store },
        })],
        timestamp_writes,
        ..Default::default()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use wgpu::{Backend, DeviceType};

    #[test]
    fn integrated_gpu_wins_then_dx12() {
        let mut options = [
            (DeviceType::DiscreteGpu, Backend::Vulkan),
            (DeviceType::IntegratedGpu, Backend::Dx12),
            (DeviceType::DiscreteGpu, Backend::Dx12),
            (DeviceType::Cpu, Backend::Vulkan),
            (DeviceType::IntegratedGpu, Backend::Vulkan),
        ];
        options.sort_by_key(|&(device, backend)| adapter_rank(device, backend));
        assert_eq!(options[0], (DeviceType::IntegratedGpu, Backend::Dx12));
        assert_eq!(options[1], (DeviceType::IntegratedGpu, Backend::Vulkan));
        assert_eq!(options[2], (DeviceType::DiscreteGpu, Backend::Dx12));
        assert_eq!(options[4], (DeviceType::Cpu, Backend::Vulkan));
    }

    #[test]
    fn uniforms_match_the_shader_layout() {
        // Offsets as WGSL lays out `Frame` in common.wgsl.
        assert_eq!(std::mem::offset_of!(Uniforms, center), 32);
        assert_eq!(std::mem::offset_of!(Uniforms, knobs), 64);
        assert_eq!(std::mem::offset_of!(Uniforms, palette_a), 96);
        assert_eq!(std::mem::offset_of!(Uniforms, palette_shift), 160);
        assert_eq!(std::mem::offset_of!(Uniforms, bands), 176);
        assert_eq!(std::mem::offset_of!(Uniforms, spectrum), 192);
        assert_eq!(std::mem::offset_of!(Uniforms, waveform), 320);
        assert_eq!(std::mem::offset_of!(Uniforms, reveal), 1344);
        assert_eq!(std::mem::offset_of!(Uniforms, calm), 1360);
        assert_eq!(std::mem::offset_of!(Uniforms, drums), 1376);
        assert_eq!(std::mem::offset_of!(Uniforms, layers), 1392);
        assert_eq!(std::mem::offset_of!(Uniforms, fx), 1408);
        assert_eq!(std::mem::offset_of!(Uniforms, heavy), 1456);
        assert_eq!(std::mem::offset_of!(Uniforms, material), 1472);
        assert_eq!(size_of::<Uniforms>(), 1488);
    }
}
