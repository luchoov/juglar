//! GPU rendering for Juglar.
//!
//! Plain Rust on top of `wgpu`, which speaks DirectX 12, Vulkan, Metal or
//! WebGPU depending on where it runs. Nothing here calls the operating system
//! or a windowing library: the app hands over anything that has a window
//! handle, then each frame's `Signals`, a few hundred plain numbers.
//!
//! A frame is three steps on the GPU (see `docs/theory/05-feedback.md`):
//!
//! 1. **Feedback** (`feedback.wgsl`), on a half-size image: copy the last
//!    picture, warped and faded, and let the scene paint on top.
//! 2. **Composite** (`composite.wgsl`), at full size: turn that picture's
//!    ink into colors and tone-map them.
//! 3. **Limiter** (`limiter.rs`): hold back any flashing beyond two flashes
//!    a second and any saturated red, then dither onto the screen (see
//!    `docs/theory/09-photosensitivity.md`).
//!
//! The analysis view (`analysis.wgsl`) replaces the first two with
//! Milestone 3's waveform and bars, to see what Juglar hears. It goes
//! through the limiter too: nothing reaches the screen without it.

mod feedback;
pub mod limiter;
mod scene;
mod timer;

use std::fmt;
use std::num::NonZeroU64;

use bytemuck::{Pod, Zeroable};
use feedback::Feedback;
use limiter::Limiter;
use scene::{Look, Tide};
use timer::GpuTimer;

pub use juglar_signals::{SPECTRUM_BANDS, Signals, WAVEFORM_POINTS};

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
    _pad1: f32,
    seed: f32,
    center: [f32; 2],
    zoom: f32,
    rotation: f32,
    swirl: f32,
    decay: f32,
    palette_drift: f32,
    _pad0: f32,
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
    /// The last `HISTORY` frames' beat and novelty, oldest first, for the
    /// analysis view's timeline.
    beat_history: [[f32; 4]; HISTORY / 4],
    novelty_history: [[f32; 4]; HISTORY / 4],
}

/// Frames of beat and novelty the analysis view shows: about two seconds.
const HISTORY: usize = 128;

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

pub struct Renderer {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    uniform_buffer: wgpu::Buffer,
    /// Uniforms plus an image and how to filter it: feedback and composite.
    image_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    feedback_pipeline: wgpu::RenderPipeline,
    composite_pipeline: wgpu::RenderPipeline,
    analysis_pipeline: wgpu::RenderPipeline,
    analysis_bind_group: wgpu::BindGroup,
    feedback: Feedback,
    limiter: Limiter,
    /// The share of the limiter's cells held back, last time it was counted.
    held: Option<f32>,
    scene: Tide,
    view: View,
    reduced_motion: bool,
    frames: u32,
    /// Beat and novelty of the last `HISTORY` frames, circular.
    history: [[f32; 2]; HISTORY],
    encode_srgb: bool,
    timer: Option<GpuTimer>,
    adapter: wgpu::AdapterInfo,
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
        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("frame uniforms"),
            size: uniform_size,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
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
        let analysis_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("analysis"),
            entries: &[uniform_entry],
        });
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
        let analysis_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("analysis"),
            layout: &analysis_layout,
            entries: &[wgpu::BindGroupEntry { binding: 0, resource: uniform_buffer.as_entire_binding() }],
        });
        // Blends the four nearest pixels; at the edges, repeats the border.
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("smooth"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        let pipeline = |label, layout: &wgpu::BindGroupLayout, source: String, entry_point, target| {
            let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some(label),
                source: wgpu::ShaderSource::Wgsl(source.into()),
            });
            let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some(label),
                bind_group_layouts: &[Some(layout)],
                immediate_size: 0,
            });
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(label),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs_main"),
                    compilation_options: Default::default(),
                    buffers: &[],
                },
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some(entry_point),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: target,
                        blend: None,
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                multiview_mask: None,
                cache: None,
            })
        };
        let feedback_pipeline = pipeline(
            "feedback",
            &image_layout,
            [COMMON, Tide::SHADER, include_str!("feedback.wgsl")].concat(),
            "fs_feedback",
            feedback::FORMAT,
        );
        let composite_pipeline =
            pipeline(
            "composite",
            &image_layout,
            [COMMON, Tide::SHADER, include_str!("composite.wgsl")].concat(),
            "fs_composite",
            limiter::CANDIDATE_FORMAT,
        );
        let analysis_pipeline =
            pipeline(
            "analysis",
            &analysis_layout,
            [COMMON, include_str!("analysis.wgsl")].concat(),
            "fs_analysis",
            limiter::CANDIDATE_FORMAT,
        );

        let feedback = Feedback::new(&device, &image_layout, &uniform_buffer, &sampler, config.width, config.height);
        let limiter = Limiter::new(&device, config.width, config.height, format);
        let timer = timestamps.then(|| GpuTimer::new(&device, queue.get_timestamp_period()));

        Ok(Self {
            surface,
            device,
            queue,
            config,
            uniform_buffer,
            image_layout,
            sampler,
            feedback_pipeline,
            composite_pipeline,
            analysis_pipeline,
            analysis_bind_group,
            feedback,
            limiter,
            held: None,
            scene: Tide::new(),
            view: View::Scene,
            reduced_motion: false,
            frames: 0,
            history: [[0.0; 2]; HISTORY],
            encode_srgb: !format.is_srgb(),
            timer,
            adapter: adapter.get_info(),
        })
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
        self.feedback =
            Feedback::new(&self.device, &self.image_layout, &self.uniform_buffer, &self.sampler, width, height);
        self.limiter.resize(&self.device, width, height);
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

    /// Draws one frame. Returns `false` when nothing could be drawn (the
    /// window is minimized or hidden), so the caller can slow down.
    ///
    /// With vsync this may wait until the display is ready for a new frame.
    pub fn render(&mut self, signals: &Signals) -> bool {
        if let Some(timer) = &mut self.timer {
            timer.collect(&self.device);
        }
        if let Some(held) = self.limiter.take_held(&self.device) {
            self.held = Some(held);
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

        // Reduced motion: the scene sees silence and no time passing.
        let still;
        let (scene_signals, motion) = if self.reduced_motion {
            still = Signals { dt: signals.dt, intensity: signals.intensity, ..Signals::default() };
            (&still, 0.0)
        } else {
            (signals, 1.0)
        };
        let look = self.scene.update(scene_signals, motion);
        self.frames = self.frames.wrapping_add(1);
        self.history[self.frames as usize % HISTORY] = [scene_signals.beat, scene_signals.novelty];
        let uniforms = self.uniforms(scene_signals, &look);
        self.queue.write_buffer(&self.uniform_buffer, 0, bytemuck::bytes_of(&uniforms));

        let screen = frame.texture.create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("frame") });
        let timed = self.timer.as_mut().is_some_and(GpuTimer::begin);
        let timestamps = |first, last| self.timer.as_ref().filter(|_| timed).map(|t| t.pass_writes(first, last));

        match self.view {
            View::Scene => {
                {
                    let mut feedback = begin_pass(&mut encoder, "feedback", self.feedback.next(), timestamps(true, false));
                    feedback.set_pipeline(&self.feedback_pipeline);
                    feedback.set_bind_group(0, self.feedback.latest(), &[]);
                    // One triangle large enough to cover the image; see `vs_main`.
                    feedback.draw(0..3, 0..1);
                }
                // The image just written is now the latest picture.
                self.feedback.swap();
                let mut composite = begin_pass(&mut encoder, "composite", self.limiter.candidate(), None);
                composite.set_pipeline(&self.composite_pipeline);
                composite.set_bind_group(0, self.feedback.latest(), &[]);
                composite.draw(0..3, 0..1);
            }
            View::Analysis => {
                let mut analysis = begin_pass(&mut encoder, "analysis", self.limiter.candidate(), timestamps(true, false));
                analysis.set_pipeline(&self.analysis_pipeline);
                analysis.set_bind_group(0, &self.analysis_bind_group, &[]);
                analysis.draw(0..3, 0..1);
            }
        }
        let seed = (self.frames % (1 << 24)) as f32;
        self.limiter.encode(&self.queue, &mut encoder, &screen, signals.dt, seed, self.encode_srgb, timestamps(false, true));
        if timed && let Some(timer) = &self.timer {
            timer.resolve(&mut encoder);
        }
        self.queue.submit([encoder.finish()]);
        if timed && let Some(timer) = &mut self.timer {
            timer.after_submit();
        }
        self.limiter.after_submit();
        self.queue.present(frame);
        true
    }

    fn uniforms(&self, s: &Signals, look: &Look) -> Uniforms {
        let (feedback_width, feedback_height) = self.feedback.size();
        let row = |v: [f32; 3]| [v[0], v[1], v[2], 0.0];
        Uniforms {
            resolution: [self.config.width as f32, self.config.height as f32],
            feedback_size: [feedback_width as f32, feedback_height as f32],
            time: s.time,
            dt: s.dt,
            _pad1: 0.0,
            // Exact as a float up to 2^24 frames, then wraps (~60 hours).
            seed: (self.frames % (1 << 24)) as f32,
            center: look.center,
            zoom: look.zoom,
            rotation: look.rotation,
            swirl: look.swirl,
            decay: look.decay,
            palette_drift: look.palette_drift,
            _pad0: 0.0,
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
            spectrum: bytemuck::cast(s.spectrum),
            // The analysis view shows the waveform as heard; a scene may
            // draw a calmer one.
            waveform: bytemuck::cast(if self.view == View::Scene { look.waveform } else { s.waveform }),
            beat_history: bytemuck::cast(self.history_of(0)),
            novelty_history: bytemuck::cast(self.history_of(1)),
        }
    }

    /// One of the two histories, oldest first.
    fn history_of(&self, which: usize) -> [f32; HISTORY] {
        let newest = self.frames as usize % HISTORY;
        std::array::from_fn(|i| self.history[(newest + 1 + i) % HISTORY][which])
    }

    /// The newest GPU time measurement of a frame, in milliseconds, if one
    /// arrived since the last call. Always `None` without timestamp support.
    pub fn take_gpu_ms(&mut self) -> Option<f32> {
        self.timer.as_mut().and_then(GpuTimer::take)
    }

    /// The share of the screen the flash limiter was holding back when it
    /// last counted (twice a second at 60 Hz), if a new count arrived.
    pub fn take_held(&mut self) -> Option<f32> {
        self.held.take()
    }
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
        assert_eq!(std::mem::offset_of!(Uniforms, beat_history), 1344);
        assert_eq!(std::mem::offset_of!(Uniforms, novelty_history), 1344 + 32 * 16);
        assert_eq!(size_of::<Uniforms>(), 1344 + 64 * 16);
    }
}
