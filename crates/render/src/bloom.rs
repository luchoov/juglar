//! Bloom: bright parts of the picture glow into their surroundings.
//!
//! A glow is a very wide blur of the bright parts, added back. Blurring a
//! full-size image that widely would read hundreds of pixels for each one.
//! Instead (`bloom.wgsl`, `docs/theory/08-bloom-and-hdr.md`):
//!
//! 1. The scene's light is drawn once more at a quarter size, keeping only
//!    what is bright (`fs_bloom_source` in `composite.wgsl`).
//! 2. That image is halved twice, three sizes in all, each pixel averaging a
//!    block of the size above: every level is a wider blur.
//! 3. The composite step reads all three, enlarged smoothly, and adds their
//!    average to the picture, as much as the scene asks for (`Look::bloom`):
//!    a tight halo and a wide haze at once.
//!
//! The usual chain also goes back up, enlarging each level onto the one
//! above in a pass of its own. Here the composite step enlarges them as it
//! reads them instead: three passes instead of five.

use crate::feedback;

/// Sizes in the chain: a quarter of the window, an eighth, a sixteenth.
/// Each is a render pass, and each pass costs the CPU about 25 µs to hand
/// to the driver whatever its size: the first version, five sizes from a
/// half, down and back up, cost 0.22 ms per frame, more than all the rest
/// of the drawing. `composite.wgsl` reads exactly three.
pub(crate) const LEVELS: usize = 3;
/// The first size, as a fraction of the window's.
const FIRST: u32 = 4;

/// The chain's pipeline, shared by every set of images.
pub(crate) struct Bloom {
    down: wgpu::RenderPipeline,
    apply_layout: wgpu::BindGroupLayout,
}

/// One chain's images: each picture being drawn (two during a transition
/// between scenes) glows from its own.
pub(crate) struct Glow {
    /// Each level's image, largest first, and bindings to read it.
    views: Vec<wgpu::TextureView>,
    reads: Vec<wgpu::BindGroup>,
    /// Binds the three levels for the composite step (its group 1).
    apply: wgpu::BindGroup,
}

impl Bloom {
    pub(crate) fn new(device: &wgpu::Device, image_layout: &wgpu::BindGroupLayout, common: &str) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("bloom"),
            source: wgpu::ShaderSource::Wgsl([common, include_str!("bloom.wgsl")].concat().into()),
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("bloom"),
            bind_group_layouts: &[Some(image_layout)],
            immediate_size: 0,
        });
        let pipeline = |label, entry_point, blend| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(label),
                layout: Some(&layout),
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
                        format: feedback::FORMAT,
                        blend,
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                multiview_mask: None,
                cache: None,
            })
        };
        let down = pipeline("bloom down", "fs_down", None);
        let apply_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("bloom apply"),
            entries: &std::array::from_fn::<_, LEVELS, _>(|level| wgpu::BindGroupLayoutEntry {
                binding: level as u32,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            }),
        });
        Self { down, apply_layout }
    }

    /// Images for a window of `width` × `height` pixels, read with the
    /// frame numbers in `uniforms`.
    pub(crate) fn glow(
        &self,
        device: &wgpu::Device,
        image_layout: &wgpu::BindGroupLayout,
        uniforms: &wgpu::Buffer,
        sampler: &wgpu::Sampler,
        width: u32,
        height: u32,
    ) -> Glow {
        let mut size = (width.max(1).div_ceil(FIRST) * 2, height.max(1).div_ceil(FIRST) * 2);
        let views: Vec<_> = (0..LEVELS)
            .map(|_| {
                size = (size.0.div_ceil(2), size.1.div_ceil(2));
                device
                    .create_texture(&wgpu::TextureDescriptor {
                        label: Some("bloom"),
                        size: wgpu::Extent3d { width: size.0, height: size.1, depth_or_array_layers: 1 },
                        mip_level_count: 1,
                        sample_count: 1,
                        dimension: wgpu::TextureDimension::D2,
                        format: feedback::FORMAT,
                        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
                        view_formats: &[],
                    })
                    .create_view(&wgpu::TextureViewDescriptor::default())
            })
            .collect();
        let reads = views
            .iter()
            .map(|view| {
                device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("bloom"),
                    layout: image_layout,
                    entries: &[
                        wgpu::BindGroupEntry { binding: 0, resource: uniforms.as_entire_binding() },
                        wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(view) },
                        wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::Sampler(sampler) },
                    ],
                })
            })
            .collect();
        let apply = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("bloom apply"),
            layout: &self.apply_layout,
            entries: &std::array::from_fn::<_, LEVELS, _>(|level| wgpu::BindGroupEntry {
                binding: level as u32,
                resource: wgpu::BindingResource::TextureView(&views[level]),
            }),
        });
        Glow { views, reads, apply }
    }

    /// The layout of `Glow::apply`, for the composite pipelines.
    pub(crate) fn apply_layout(&self) -> &wgpu::BindGroupLayout {
        &self.apply_layout
    }

    /// Records the chain into `glow`: the scene's bright parts at a quarter
    /// size (with `source`, reading the feedback image through `picture`),
    /// then down.
    pub(crate) fn encode(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        glow: &Glow,
        source: &wgpu::RenderPipeline,
        picture: &wgpu::BindGroup,
    ) {
        fn pass<'e>(encoder: &'e mut wgpu::CommandEncoder, target: &wgpu::TextureView, load: wgpu::LoadOp<wgpu::Color>) -> wgpu::RenderPass<'e> {
            encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("bloom"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations { load, store: wgpu::StoreOp::Store },
                })],
                ..Default::default()
            })
        }
        let clear = wgpu::LoadOp::Clear(wgpu::Color::BLACK);
        {
            let mut first = pass(encoder, &glow.views[0], clear);
            first.set_pipeline(source);
            first.set_bind_group(0, picture, &[]);
            first.draw(0..3, 0..1);
        }
        for level in 1..LEVELS {
            let mut down = pass(encoder, &glow.views[level], clear);
            down.set_pipeline(&self.down);
            down.set_bind_group(0, &glow.reads[level - 1], &[]);
            down.draw(0..3, 0..1);
        }
    }
}

impl Glow {
    /// The three blurs, to bind as the composite step's group 1.
    pub(crate) fn apply(&self) -> &wgpu::BindGroup {
        &self.apply
    }
}
