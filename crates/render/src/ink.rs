//! Text and panels drawn over the finished frame: the overlay that says what
//! a key changed, the help, and the analysis view's labels.
//!
//! Glyphs are rasterized on the CPU (`fontdue`) the first time each one is
//! needed at a size, into one atlas image; every frame, each glyph and each
//! panel is one instance of a quad, all drawn in a single pass. The fonts come
//! from the app (Windows has Segoe UI), so without them nothing is drawn.

use std::collections::HashMap;
use std::num::NonZeroU64;

use bytemuck::{Pod, Zeroable};

use crate::present::CANDIDATE_FORMAT;

/// The atlas is one square image, this many pixels a side.
const ATLAS: u32 = 1024;

/// A weight of the font.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Weight {
    Regular,
    Strong,
}

/// One quad: a glyph from the atlas, or a panel with rounded corners. Laid
/// out as `Quad` in `ink.wgsl`.
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Quad {
    /// Left, top, width, height, in pixels from the top left.
    rect: [f32; 4],
    /// The glyph's place in the atlas, 0..1; unused by panels.
    uv: [f32; 4],
    /// sRGB color, and opacity.
    color: [f32; 4],
    /// Corner radius (or a line's half width) in pixels, and 0 for a panel,
    /// 1 for a glyph, 2 for a line from `rect`'s first point to its second.
    shape: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Uniforms {
    resolution: [f32; 2],
    _pad: [f32; 2],
}

/// Where a glyph sits in the atlas and how to place it.
#[derive(Clone, Copy)]
struct Glyph {
    /// Left, top, width, height in the atlas, in pixels.
    place: [u32; 4],
    /// From the pen to the bitmap's left edge, and from the baseline up to
    /// its bottom edge.
    offset: [f32; 2],
    advance: f32,
}

pub(crate) struct Ink {
    fonts: Vec<fontdue::Font>,
    glyphs: HashMap<(Weight, char, u16), Glyph>,
    /// The next free spot in the atlas: shelves filled left to right.
    shelf: [u32; 3],
    atlas: wgpu::Texture,
    uniforms: wgpu::Buffer,
    instances: wgpu::Buffer,
    capacity: usize,
    group: wgpu::BindGroup,
    pipeline: wgpu::RenderPipeline,
    quads: Vec<Quad>,
}

impl Ink {
    pub(crate) fn new(device: &wgpu::Device) -> Self {
        let atlas = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("glyph atlas"),
            size: wgpu::Extent3d { width: ATLAS, height: ATLAS, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let uniforms = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("ink uniforms"),
            size: size_of::<Uniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let capacity = 1024;
        let instances = quad_buffer(device, capacity);
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("glyph atlas"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("ink"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
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
        let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("ink"),
            layout: &layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: uniforms.as_entire_binding() },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&atlas.create_view(&Default::default())),
                },
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::Sampler(&sampler) },
            ],
        });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("ink"),
            source: wgpu::ShaderSource::Wgsl(include_str!("ink.wgsl").into()),
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("ink"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("ink"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_quad"),
                compilation_options: Default::default(),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: size_of::<Quad>() as u64,
                    step_mode: wgpu::VertexStepMode::Instance,
                    attributes: &wgpu::vertex_attr_array![0 => Float32x4, 1 => Float32x4, 2 => Float32x4, 3 => Float32x4],
                })],
            },
            primitive: wgpu::PrimitiveState { topology: wgpu::PrimitiveTopology::TriangleStrip, ..Default::default() },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_quad"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: CANDIDATE_FORMAT,
                    blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });
        Self {
            fonts: Vec::new(),
            glyphs: HashMap::new(),
            shelf: [0, 0, 0],
            atlas,
            uniforms,
            instances,
            capacity,
            group,
            pipeline,
            quads: Vec::new(),
        }
    }

    /// The fonts to write with: regular first, then a stronger weight. Any
    /// that can't be read are skipped.
    pub(crate) fn set_fonts(&mut self, fonts: &[Vec<u8>]) {
        self.fonts = fonts
            .iter()
            .filter_map(|bytes| fontdue::Font::from_bytes(bytes.as_slice(), fontdue::FontSettings::default()).ok())
            .collect();
        self.glyphs.clear();
        self.shelf = [0, 0, 0];
    }

    pub(crate) fn has_fonts(&self) -> bool {
        !self.fonts.is_empty()
    }

    /// A panel: a rectangle with rounded corners, in pixels.
    pub(crate) fn panel(&mut self, rect: [f32; 4], radius: f32, color: [f32; 3], alpha: f32) {
        if alpha > 0.0 {
            self.quads.push(Quad { rect, uv: [0.0; 4], color: [color[0], color[1], color[2], alpha], shape: [radius, 0.0, 0.0, 0.0] });
        }
    }

    /// A line from `a` to `b`, `width` pixels wide, with round ends.
    pub(crate) fn line(&mut self, a: [f32; 2], b: [f32; 2], width: f32, color: [f32; 3], alpha: f32) {
        if alpha > 0.0 {
            let shape = [width * 0.5, 2.0, 0.0, 0.0];
            self.quads.push(Quad { rect: [a[0], a[1], b[0], b[1]], uv: [0.0; 4], color: [color[0], color[1], color[2], alpha], shape });
        }
    }

    /// How wide `text` is at `size` pixels.
    pub(crate) fn width(&mut self, queue: &wgpu::Queue, text: &str, size: f32, weight: Weight) -> f32 {
        text.chars().filter_map(|c| self.glyph(queue, weight, c, size)).map(|g| g.advance).sum()
    }

    /// Writes `text` with its baseline's left end at `pen`; returns where
    /// the pen ends up.
    pub(crate) fn text(
        &mut self,
        queue: &wgpu::Queue,
        text: &str,
        pen: [f32; 2],
        size: f32,
        weight: Weight,
        color: [f32; 3],
        alpha: f32,
    ) -> f32 {
        // Whole pixels, so every glyph lands on the grid it was drawn for.
        let (mut x, y) = (pen[0].round(), pen[1].round());
        for c in text.chars() {
            let Some(glyph) = self.glyph(queue, weight, c, size) else { continue };
            let [u, v, w, h] = glyph.place;
            if w > 0 && h > 0 && alpha > 0.0 {
                let at = ATLAS as f32;
                self.quads.push(Quad {
                    rect: [x + glyph.offset[0], y - glyph.offset[1] - h as f32, w as f32, h as f32],
                    uv: [u as f32 / at, v as f32 / at, (u + w) as f32 / at, (v + h) as f32 / at],
                    color: [color[0], color[1], color[2], alpha],
                    shape: [0.0, 1.0, 0.0, 0.0],
                });
            }
            x += glyph.advance.round();
        }
        x
    }

    /// The font's ascent and descent at `size`, for centering lines.
    pub(crate) fn metrics(&self, size: f32) -> (f32, f32) {
        self.fonts
            .first()
            .and_then(|font| font.horizontal_line_metrics(size))
            .map_or((size * 0.75, size * 0.25), |m| (m.ascent, -m.descent))
    }

    /// The glyph for `c`, drawn into the atlas the first time.
    fn glyph(&mut self, queue: &wgpu::Queue, weight: Weight, c: char, size: f32) -> Option<Glyph> {
        let px = size.round().clamp(4.0, 200.0) as u16;
        if let Some(glyph) = self.glyphs.get(&(weight, c, px)) {
            return Some(*glyph);
        }
        let font = match weight {
            Weight::Strong => self.fonts.get(1).or(self.fonts.first()),
            Weight::Regular => self.fonts.first(),
        }?;
        let (metrics, bitmap) = font.rasterize(c, f32::from(px));
        let (w, h) = (metrics.width as u32, metrics.height as u32);
        // A full atlas starts over; the next frames draw what they need again.
        if self.shelf[0] + w + 1 > ATLAS {
            self.shelf = [0, self.shelf[1] + self.shelf[2] + 1, 0];
        }
        if self.shelf[1] + h + 1 > ATLAS {
            self.glyphs.clear();
            self.shelf = [0, 0, 0];
        }
        let place = [self.shelf[0], self.shelf[1], w, h];
        if w > 0 && h > 0 {
            queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &self.atlas,
                    mip_level: 0,
                    origin: wgpu::Origin3d { x: place[0], y: place[1], z: 0 },
                    aspect: wgpu::TextureAspect::All,
                },
                &bitmap,
                wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(w), rows_per_image: Some(h) },
                wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
            );
        }
        self.shelf[0] += w + 1;
        self.shelf[2] = self.shelf[2].max(h);
        let glyph = Glyph { place, offset: [metrics.xmin as f32, metrics.ymin as f32], advance: metrics.advance_width };
        self.glyphs.insert((weight, c, px), glyph);
        Some(glyph)
    }

    /// Draws everything queued since the last call over `target`.
    pub(crate) fn encode(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        size: [f32; 2],
    ) {
        if self.quads.is_empty() {
            return;
        }
        if self.quads.len() > self.capacity {
            self.capacity = self.quads.len().next_power_of_two();
            self.instances = quad_buffer(device, self.capacity);
        }
        queue.write_buffer(&self.uniforms, 0, bytemuck::bytes_of(&Uniforms { resolution: size, _pad: [0.0; 2] }));
        queue.write_buffer(&self.instances, 0, bytemuck::cast_slice(&self.quads));
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("ink"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations { load: wgpu::LoadOp::Load, store: wgpu::StoreOp::Store },
            })],
            ..Default::default()
        });
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &self.group, &[]);
        pass.set_vertex_buffer(0, self.instances.slice(..));
        pass.draw(0..4, 0..self.quads.len() as u32);
        drop(pass);
        self.quads.clear();
    }
}

fn quad_buffer(device: &wgpu::Device, capacity: usize) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("ink quads"),
        size: (capacity * size_of::<Quad>()) as u64,
        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}
