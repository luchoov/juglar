//! GPU rendering for Juglar.
//!
//! Plain Rust on top of `wgpu`, which speaks DirectX 12, Vulkan, Metal or
//! WebGPU depending on where it runs. Nothing here calls the operating system
//! or a windowing library: the app hands over anything that has a window
//! handle, then a few hundred plain numbers every frame. Everything else
//! happens in `frame.wgsl`, once per pixel, on the GPU.

mod timer;

use std::fmt;
use std::num::NonZeroU64;

use bytemuck::{Pod, Zeroable};
use timer::GpuTimer;

pub const SPECTRUM_BANDS: usize = 32;
pub const WAVEFORM_POINTS: usize = 256;

/// Everything one frame shows. Levels are in `0..=1`; the waveform in `-1..=1`.
#[derive(Clone, Debug)]
pub struct FrameInput {
    /// Seconds since start.
    pub time: f32,
    pub bass: f32,
    pub mid: f32,
    pub treble: f32,
    pub energy: f32,
    pub spectrum: [f32; SPECTRUM_BANDS],
    pub waveform: [f32; WAVEFORM_POINTS],
}

impl Default for FrameInput {
    fn default() -> Self {
        Self {
            time: 0.0,
            bass: 0.0,
            mid: 0.0,
            treble: 0.0,
            energy: 0.0,
            spectrum: [0.0; SPECTRUM_BANDS],
            waveform: [0.0; WAVEFORM_POINTS],
        }
    }
}

/// The frame's numbers laid out exactly as the shader's `Frame` struct reads
/// them: GPU uniform memory works in 16-byte rows, so arrays are packed four
/// floats to a row.
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Uniforms {
    resolution: [f32; 2],
    time: f32,
    /// 1.0 when the shader must encode its output to sRGB itself (see `new`).
    encode_srgb: f32,
    bands: [f32; 4],
    spectrum: [[f32; 4]; SPECTRUM_BANDS / 4],
    waveform: [[f32; 4]; WAVEFORM_POINTS / 4],
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
    pipeline: wgpu::RenderPipeline,
    uniform_buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
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

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("frame"),
            source: wgpu::ShaderSource::Wgsl(include_str!("frame.wgsl").into()),
        });

        let uniform_size = size_of::<Uniforms>() as u64;
        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("frame uniforms"),
            size: uniform_size,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("frame"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: NonZeroU64::new(uniform_size),
                },
                count: None,
            }],
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("frame"),
            layout: &layout,
            entries: &[wgpu::BindGroupEntry { binding: 0, resource: uniform_buffer.as_entire_binding() }],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("frame"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("frame"),
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
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });

        let timer = timestamps.then(|| GpuTimer::new(&device, queue.get_timestamp_period()));

        Ok(Self {
            surface,
            device,
            queue,
            config,
            pipeline,
            uniform_buffer,
            bind_group,
            encode_srgb: !format.is_srgb(),
            timer,
            adapter: adapter.get_info(),
        })
    }

    /// The GPU in use and the graphics API spoken to it, e.g. "Intel UHD 620 (Dx12)".
    pub fn adapter_name(&self) -> String {
        format!("{} ({:?})", self.adapter.name, self.adapter.backend)
    }

    /// Follows a window resize. A zero size (minimized) is ignored.
    pub fn resize(&mut self, width: u32, height: u32) {
        if width == 0 || height == 0 {
            return;
        }
        self.config.width = width;
        self.config.height = height;
        self.surface.configure(&self.device, &self.config);
    }

    /// Draws one frame. Returns `false` when nothing could be drawn (the
    /// window is minimized or hidden), so the caller can slow down.
    ///
    /// With vsync this may wait until the display is ready for a new frame.
    pub fn render(&mut self, input: &FrameInput) -> bool {
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

        let uniforms = Uniforms {
            resolution: [self.config.width as f32, self.config.height as f32],
            time: input.time,
            encode_srgb: if self.encode_srgb { 1.0 } else { 0.0 },
            bands: [input.bass, input.mid, input.treble, input.energy],
            spectrum: bytemuck::cast(input.spectrum),
            waveform: bytemuck::cast(input.waveform),
        };
        self.queue.write_buffer(&self.uniform_buffer, 0, bytemuck::bytes_of(&uniforms));

        let view = frame.texture.create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("frame") });
        let timed = self.timer.as_mut().is_some_and(GpuTimer::begin);
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("frame"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    depth_slice: None,
                    resolve_target: None,
                    // Every pixel is overwritten, so the old contents never
                    // need to be loaded.
                    ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::BLACK), store: wgpu::StoreOp::Store },
                })],
                timestamp_writes: self.timer.as_ref().filter(|_| timed).map(GpuTimer::pass_writes),
                ..Default::default()
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &self.bind_group, &[]);
            // One triangle large enough to cover the screen; see `vs_main`.
            pass.draw(0..3, 0..1);
        }
        if timed && let Some(timer) = &self.timer {
            timer.resolve(&mut encoder);
        }
        self.queue.submit([encoder.finish()]);
        if timed && let Some(timer) = &mut self.timer {
            timer.after_submit();
        }
        self.queue.present(frame);
        true
    }

    /// The newest GPU time measurement of a frame, in milliseconds, if one
    /// arrived since the last call. Always `None` without timestamp support.
    pub fn take_gpu_ms(&mut self) -> Option<f32> {
        self.timer.as_mut().and_then(GpuTimer::take)
    }
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
        // 16 bytes of header, bands, then 8 and 64 rows of 16 bytes.
        assert_eq!(size_of::<Uniforms>(), 16 + 16 + 8 * 16 + 64 * 16);
    }
}
