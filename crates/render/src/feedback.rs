//! The two images the feedback loop alternates between.
//!
//! Each frame reads one image (last frame's picture) and writes the other;
//! the next frame swaps them. A GPU can't read and write the same image in
//! one step, hence two: "ping-pong".
//!
//! They hold 16-bit floating-point numbers rather than the screen's 8-bit
//! levels. Fading multiplies every pixel by a number just under 1, sixty
//! times a second. With 256 levels, a dim pixel gets stuck: 3 × 0.98 rounds
//! back to 3, and a ghost of every trail stays forever. Floats keep fading
//! smoothly down to nothing, and can hold values above 1 for dense paint.
//!
//! They are half the window's width and height, a quarter of its pixels:
//! the trails are soft anyway, and this is the step that touches every
//! pixel twice (read and write) each frame.

pub(crate) const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;

pub(crate) struct Feedback {
    size: (u32, u32),
    views: [wgpu::TextureView; 2],
    /// `reads[i]` lets a shader read image `i`.
    reads: [wgpu::BindGroup; 2],
    /// The image holding the latest picture.
    current: usize,
}

impl Feedback {
    /// Images for a window of `width` × `height` pixels. New images start
    /// empty (all zeros), so the picture builds up from black.
    pub(crate) fn new(
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        uniforms: &wgpu::Buffer,
        sampler: &wgpu::Sampler,
        width: u32,
        height: u32,
    ) -> Self {
        let size = (width.div_ceil(2).max(1), height.div_ceil(2).max(1));
        let views = std::array::from_fn(|_| {
            device
                .create_texture(&wgpu::TextureDescriptor {
                    label: Some("feedback"),
                    size: wgpu::Extent3d { width: size.0, height: size.1, depth_or_array_layers: 1 },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: FORMAT,
                    usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
                    view_formats: &[],
                })
                .create_view(&wgpu::TextureViewDescriptor::default())
        });
        let reads = std::array::from_fn(|i| {
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("feedback"),
                layout,
                entries: &[
                    wgpu::BindGroupEntry { binding: 0, resource: uniforms.as_entire_binding() },
                    wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&views[i]) },
                    wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::Sampler(sampler) },
                ],
            })
        });
        Self { size, views, reads, current: 0 }
    }

    pub(crate) fn size(&self) -> (u32, u32) {
        self.size
    }

    /// Bindings that let a shader read the latest picture.
    pub(crate) fn latest(&self) -> &wgpu::BindGroup {
        &self.reads[self.current]
    }

    /// The image the next picture is written to.
    pub(crate) fn next(&self) -> &wgpu::TextureView {
        &self.views[1 - self.current]
    }

    /// Makes the image just written the latest picture.
    pub(crate) fn swap(&mut self) {
        self.current = 1 - self.current;
    }
}
