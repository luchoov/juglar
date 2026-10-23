//! The last step before the screen: every view draws its frame into a
//! "candidate" image, in linear light, and `present.wgsl` dithers it and
//! encodes it for the screen.
//!
//! Until D48 this step was a flash limiter, which held back flashing and
//! saturated red. Now nothing is held back.

use std::num::NonZeroU64;

use bytemuck::{Pod, Zeroable};

/// The candidate image's format: linear light, finer than the screen's
/// levels so that dithering has something to work with.
pub(crate) const CANDIDATE_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;

/// `Present` in `present.wgsl`, padded to a 16-byte row.
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Uniforms {
    seed: f32,
    encode_srgb: f32,
    _pad: [f32; 2],
}

pub(crate) struct Present {
    uniforms: wgpu::Buffer,
    pipeline: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
    candidate: wgpu::TextureView,
    group: wgpu::BindGroup,
}

impl Present {
    pub(crate) fn new(device: &wgpu::Device, width: u32, height: u32, screen_format: wgpu::TextureFormat) -> Self {
        let uniforms = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("present uniforms"),
            size: size_of::<Uniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("present"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: NonZeroU64::new(size_of::<Uniforms>() as u64),
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
            ],
        });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("present"),
            source: wgpu::ShaderSource::Wgsl(include_str!("present.wgsl").into()),
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("present"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("present"),
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
                entry_point: Some("fs_present"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState { format: screen_format, blend: None, write_mask: wgpu::ColorWrites::ALL })],
            }),
            multiview_mask: None,
            cache: None,
        });
        let (candidate, group) = images(device, &layout, &uniforms, width, height);
        Self { uniforms, pipeline, layout, candidate, group }
    }

    /// Follows a resize.
    pub(crate) fn resize(&mut self, device: &wgpu::Device, width: u32, height: u32) {
        (self.candidate, self.group) = images(device, &self.layout, &self.uniforms, width, height);
    }

    /// The image every view draws its frame into: linear light, 0 to 1.
    pub(crate) fn candidate(&self) -> &wgpu::TextureView {
        &self.candidate
    }

    /// Records the pass that puts the candidate on `screen`.
    pub(crate) fn encode(
        &self,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        screen: &wgpu::TextureView,
        seed: f32,
        encode_srgb: bool,
        timestamps: Option<wgpu::RenderPassTimestampWrites<'_>>,
    ) {
        let uniforms = Uniforms { seed, encode_srgb: if encode_srgb { 1.0 } else { 0.0 }, _pad: [0.0; 2] };
        queue.write_buffer(&self.uniforms, 0, bytemuck::bytes_of(&uniforms));
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("present"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: screen,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::BLACK), store: wgpu::StoreOp::Store },
            })],
            timestamp_writes: timestamps,
            ..Default::default()
        });
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &self.group, &[]);
        pass.draw(0..3, 0..1);
    }
}

/// The candidate image for a screen of `width` × `height` pixels, and the
/// bindings that let `fs_present` read it.
fn images(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    uniforms: &wgpu::Buffer,
    width: u32,
    height: u32,
) -> (wgpu::TextureView, wgpu::BindGroup) {
    let candidate = device
        .create_texture(&wgpu::TextureDescriptor {
            label: Some("candidate"),
            size: wgpu::Extent3d { width: width.max(1), height: height.max(1), depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: CANDIDATE_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        })
        .create_view(&wgpu::TextureViewDescriptor::default());
    let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("present"),
        layout,
        entries: &[
            wgpu::BindGroupEntry { binding: 0, resource: uniforms.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&candidate) },
        ],
    });
    (candidate, group)
}
