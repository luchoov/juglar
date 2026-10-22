//! The flash limiter: every frame passes through it on its way to the screen.
//!
//! Juglar's policy (`docs/research/focus-and-relaxation.md`, section 9) is at
//! most two flashes in any second, anywhere on screen, and no saturated red
//! at all. A flash is a pair of opposing swings in luminance of 0.1 or more.
//! Scenes are designed to stay far from that, but designs have bugs and
//! music is unpredictable, so the final picture is checked, not trusted.
//!
//! Every view draws its frame into a `candidate` image, in linear light
//! between 0 and 1. Then, on the GPU, in the same frame (`limiter.wgsl`):
//!
//! 1. For each cell of a grid of 8×8-pixel cells at 1080 rows, the
//!    candidate's mean luminance.
//!    Each cell follows its luminance over time: its last extreme, which way
//!    it is heading, and the age of its last four transitions (swings of 0.1
//!    or more against the way it was heading; two make a flash). With four
//!    in the last second, a swing that would make a fifth is held back: the
//!    cell shows only as much of the candidate, mixed with what it showed
//!    last frame, as keeps the swing under 0.1.
//! 2. For each pixel, that mix; then any color with more than 75% of its
//!    light in red is moved toward gray of the same luminance, dithered and
//!    encoded for the screen. The mix is kept as the next frame's "shown".
//!
//! Nothing reaches the screen before it has been checked: the decision is
//! made in the same frame it applies to. Reading results back to the CPU
//! (how many cells are being held) happens a frame or two later, only to
//! report it.
//!
//! What this doesn't see is written up in `docs/theory/09-photosensitivity.md`:
//! flicker much finer than a cell, which a cell's mean averages away.

use std::num::NonZeroU64;
use std::sync::Arc;
use std::sync::atomic::{AtomicU8, Ordering};

use bytemuck::{Pod, Zeroable};

/// A swing of luminance this large is a transition (WCAG's general flash).
pub const DELTA: f32 = 0.1;
/// Held swings stay this far under `DELTA`.
pub const MARGIN: f32 = 0.01;
/// Transitions allowed within `WINDOW` seconds: two flashes.
pub const MAX_TRANSITIONS: usize = 4;
pub const WINDOW: f32 = 1.0;
/// Rows of cells; the cells are square, so columns follow the aspect ratio.
/// At 1080 rows a cell is 8×8 pixels: a line one pixel wide flashing from
/// black to white still swings its cell by 0.125, enough to be counted.
pub const CELL_ROWS: u32 = 135;
/// The largest share of the light red may carry (saturated red is 0.8).
pub const RED_SHARE: f32 = 0.75;

/// The candidate and the shown image: linear light, enough precision that
/// dithering at the very end still has something to do.
pub(crate) const CANDIDATE_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;
/// Cell state: luminances need more precision than 16-bit floats near 1.
const CELL_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba32Float;
/// The age of a transition that never happened.
const NEVER: f32 = 1000.0;

/// One cell of the limiter, as `fs_decide` in `limiter.wgsl` runs it. The GPU
/// runs the shader; this copy is for tests, and to read in plain Rust.
#[derive(Clone, Copy, Debug)]
pub struct Cell {
    /// The last extreme of shown luminance since the last transition.
    pub extreme: f32,
    /// +1 heading up, -1 heading down, 0 before the first transition.
    pub direction: f32,
    /// The luminance shown last frame.
    pub last: f32,
    /// Seconds since each of the last four transitions, newest first.
    pub ages: [f32; MAX_TRANSITIONS],
}

impl Default for Cell {
    fn default() -> Self {
        Self { extreme: 0.0, direction: 0.0, last: 0.0, ages: [NEVER; MAX_TRANSITIONS] }
    }
}

impl Cell {
    /// Takes the candidate's luminance `y` after `dt` seconds; returns the
    /// luminance shown and how much of the candidate it took (1 = all).
    pub fn step(&mut self, y: f32, dt: f32) -> (f32, f32) {
        self.ages.iter_mut().for_each(|a| *a += dt);
        let recent = self.ages.iter().filter(|&&a| a < WINDOW).count();
        // With the budget spent, the cell may keep going the way it is
        // heading, but may not swing back by DELTA.
        let (mut low, mut high) = (-1.0, 2.0);
        if recent >= MAX_TRANSITIONS {
            if self.direction > 0.0 {
                low = self.extreme - (DELTA - MARGIN);
            } else if self.direction < 0.0 {
                high = self.extreme + (DELTA - MARGIN);
            }
        }
        let mut alpha: f32 = 1.0;
        if y < low {
            alpha = (low - self.last) / (y - self.last);
        } else if y > high {
            alpha = (high - self.last) / (y - self.last);
        }
        let alpha = alpha.clamp(0.0, 1.0);
        let shown = self.last + (y - self.last) * alpha;

        // Follow the luminance actually shown.
        let mut transition = false;
        if self.direction == 0.0 {
            if (shown - self.extreme).abs() >= DELTA {
                self.direction = (shown - self.extreme).signum();
                self.extreme = shown;
                transition = true;
            }
        } else if self.direction > 0.0 {
            if shown > self.extreme {
                self.extreme = shown;
            } else if self.extreme - shown >= DELTA {
                self.direction = -1.0;
                self.extreme = shown;
                transition = true;
            }
        } else if shown < self.extreme {
            self.extreme = shown;
        } else if shown - self.extreme >= DELTA {
            self.direction = 1.0;
            self.extreme = shown;
            transition = true;
        }
        if transition {
            self.ages.rotate_right(1);
            self.ages[0] = 0.0;
        }
        self.ages.iter_mut().for_each(|a| *a = a.min(NEVER));
        self.last = shown;
        (shown, alpha)
    }
}

/// The grid for a screen of `width` × `height` pixels: the cell side in
/// pixels (even, so the four-pixel samples never straddle two cells) and
/// the number of columns and rows.
pub fn grid(width: u32, height: u32) -> (u32, u32, u32) {
    let side = 2 * height.div_ceil(2 * CELL_ROWS).max(1);
    (side, width.div_ceil(side), height.div_ceil(side))
}

/// `Limit` in `limiter.wgsl`.
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Uniforms {
    size: [f32; 2],
    cells: [f32; 2],
    cell: f32,
    dt: f32,
    seed: f32,
    encode_srgb: f32,
}

pub(crate) struct Limiter {
    uniforms: wgpu::Buffer,
    decide: wgpu::RenderPipeline,
    present: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    images: Images,
}

/// Everything that depends on the screen's size.
struct Images {
    size: (u32, u32),
    grid: (u32, u32, u32),
    /// Kept for tests, which write candidates directly.
    #[cfg_attr(not(test), allow(dead_code))]
    candidate: wgpu::Texture,
    candidate_view: wgpu::TextureView,
    /// The last two frames shown, linear: one read, one written, in turns.
    shown: [wgpu::TextureView; 2],
    /// Cell state, two textures of four numbers per cell, also in turns:
    /// (extreme, direction, last shown luminance, share of the candidate
    /// shown) and the ages of the last four transitions.
    cells_a: [wgpu::Texture; 2],
    cells_a_views: [wgpu::TextureView; 2],
    cells_b: [wgpu::TextureView; 2],
    /// `decide[i]` reads state `i`. `present[i]` reads shown image `i` and
    /// the decision just written into state `1 - i`.
    decide: [wgpu::BindGroup; 2],
    present: [wgpu::BindGroup; 2],
    /// Which of each pair holds the latest state.
    current: usize,
    /// The ages must be written as "never" before first use.
    fresh: bool,
    readback: Readback,
}

impl Limiter {
    pub(crate) fn new(device: &wgpu::Device, width: u32, height: u32, screen_format: wgpu::TextureFormat) -> Self {
        let uniforms = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("limiter uniforms"),
            size: size_of::<Uniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let texture = |binding, filterable| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Float { filterable },
                view_dimension: wgpu::TextureViewDimension::D2,
                multisampled: false,
            },
            count: None,
        };
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("limiter"),
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
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                texture(2, true),
                texture(3, false),
                texture(4, false),
                texture(5, false),
            ],
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("limiter"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("limiter"),
            source: wgpu::ShaderSource::Wgsl(include_str!("limiter.wgsl").into()),
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("limiter"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let target = |format| Some(wgpu::ColorTargetState { format, blend: None, write_mask: wgpu::ColorWrites::ALL });
        let pipeline = |label, entry_point, targets: &[Option<wgpu::ColorTargetState>]| {
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
                    targets,
                }),
                multiview_mask: None,
                cache: None,
            })
        };
        let decide = pipeline("limiter decide", "fs_decide", &[target(CELL_FORMAT), target(CELL_FORMAT)]);
        let present = pipeline("limiter present", "fs_present", &[target(screen_format), target(CANDIDATE_FORMAT)]);
        let images = Images::new(device, &layout, &uniforms, &sampler, width, height);
        Self { uniforms, decide, present, layout, sampler, images }
    }

    /// Follows a resize. The limiter starts over from a black screen, so the
    /// first frame at the new size counts as a transition where it is bright.
    pub(crate) fn resize(&mut self, device: &wgpu::Device, width: u32, height: u32) {
        self.images = Images::new(device, &self.layout, &self.uniforms, &self.sampler, width, height);
    }

    /// The image every view draws its frame into: linear light, 0 to 1.
    pub(crate) fn candidate(&self) -> &wgpu::TextureView {
        &self.images.candidate_view
    }

    /// Records the limiter's two passes: decide per cell, then present the
    /// limited frame into `screen`. `timestamps` go on the last pass.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn encode(
        &mut self,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        screen: &wgpu::TextureView,
        dt: f32,
        seed: f32,
        encode_srgb: bool,
        timestamps: Option<wgpu::RenderPassTimestampWrites<'_>>,
    ) {
        let images = &mut self.images;
        let (side, columns, rows) = images.grid;
        let uniforms = Uniforms {
            size: [images.size.0 as f32, images.size.1 as f32],
            cells: [columns as f32, rows as f32],
            cell: side as f32,
            // A stall (the window was dragged, or hidden) counts as a second
            // at most: the ages only need to know it was long.
            dt: dt.clamp(0.0, WINDOW),
            seed,
            encode_srgb: if encode_srgb { 1.0 } else { 0.0 },
        };
        queue.write_buffer(&self.uniforms, 0, bytemuck::bytes_of(&uniforms));

        let now = images.current;
        let next = 1 - now;
        if images.fresh {
            let never = f64::from(NEVER);
            clear(encoder, &images.cells_b[now], wgpu::Color { r: never, g: never, b: never, a: never });
            clear(encoder, &images.cells_a_views[now], wgpu::Color::TRANSPARENT);
            clear(encoder, &images.shown[now], wgpu::Color::BLACK);
            images.fresh = false;
        }
        {
            let mut decide = begin(encoder, "limiter decide", &[&images.cells_a_views[next], &images.cells_b[next]], None);
            decide.set_pipeline(&self.decide);
            decide.set_bind_group(0, &images.decide[now], &[]);
            decide.draw(0..3, 0..1);
        }
        {
            let mut present = begin(encoder, "limiter present", &[screen, &images.shown[next]], timestamps);
            present.set_pipeline(&self.present);
            present.set_bind_group(0, &images.present[now], &[]);
            present.draw(0..3, 0..1);
        }
        images.readback.maybe_copy(encoder, &images.cells_a[next]);
        images.current = next;
    }

    /// Call after submitting the frame `encode` recorded.
    pub(crate) fn after_submit(&mut self) {
        self.images.readback.after_submit();
    }

    /// The share of cells being held back in a recent frame, if a count
    /// arrived since the last call. Never waits for the GPU.
    pub(crate) fn take_held(&mut self, device: &wgpu::Device) -> Option<f32> {
        self.images.readback.collect(device)
    }
}

impl Images {
    fn new(
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        uniforms: &wgpu::Buffer,
        sampler: &wgpu::Sampler,
        width: u32,
        height: u32,
    ) -> Self {
        let size = (width.max(1), height.max(1));
        let grid = grid(size.0, size.1);
        let make = |label, (width, height), format, usage| {
            device.create_texture(&wgpu::TextureDescriptor {
                label: Some(label),
                size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format,
                usage,
                view_formats: &[],
            })
        };
        let image = wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING;
        let view = |texture: &wgpu::Texture| texture.create_view(&wgpu::TextureViewDescriptor::default());
        let cells = (grid.1, grid.2);
        // COPY_DST lets tests write a candidate directly.
        let candidate = make("candidate", size, CANDIDATE_FORMAT, image | wgpu::TextureUsages::COPY_DST);
        let candidate_view = view(&candidate);
        let shown = std::array::from_fn(|_| view(&make("shown", size, CANDIDATE_FORMAT, image)));
        let cells_a: [wgpu::Texture; 2] =
            std::array::from_fn(|_| make("limiter cells", cells, CELL_FORMAT, image | wgpu::TextureUsages::COPY_SRC));
        let cells_a_views = std::array::from_fn(|i| view(&cells_a[i]));
        let cells_b = std::array::from_fn(|_| view(&make("limiter ages", cells, CELL_FORMAT, image)));
        let group = |label, shown: &wgpu::TextureView, a: &wgpu::TextureView, b: &wgpu::TextureView| {
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some(label),
                layout,
                entries: &[
                    wgpu::BindGroupEntry { binding: 0, resource: uniforms.as_entire_binding() },
                    wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(sampler) },
                    wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::TextureView(&candidate_view) },
                    wgpu::BindGroupEntry { binding: 3, resource: wgpu::BindingResource::TextureView(shown) },
                    wgpu::BindGroupEntry { binding: 4, resource: wgpu::BindingResource::TextureView(a) },
                    wgpu::BindGroupEntry { binding: 5, resource: wgpu::BindingResource::TextureView(b) },
                ],
            })
        };
        let decide = std::array::from_fn(|i| group("limiter decide", &shown[i], &cells_a_views[i], &cells_b[i]));
        let present = std::array::from_fn(|i| group("limiter present", &shown[i], &cells_a_views[1 - i], &cells_b[i]));
        Self {
            size,
            grid,
            candidate,
            candidate_view,
            shown,
            cells_a,
            cells_a_views,
            cells_b,
            decide,
            present,
            current: 0,
            fresh: true,
            readback: Readback::new(device, cells),
        }
    }
}

/// A pass over all of `targets`, cleared first.
fn begin<'e>(
    encoder: &'e mut wgpu::CommandEncoder,
    label: &str,
    targets: &[&wgpu::TextureView],
    timestamp_writes: Option<wgpu::RenderPassTimestampWrites<'_>>,
) -> wgpu::RenderPass<'e> {
    let attachments: Vec<_> = targets
        .iter()
        .map(|&view| {
            Some(wgpu::RenderPassColorAttachment {
                view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::BLACK), store: wgpu::StoreOp::Store },
            })
        })
        .collect();
    encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some(label),
        color_attachments: &attachments,
        timestamp_writes,
        ..Default::default()
    })
}

fn clear(encoder: &mut wgpu::CommandEncoder, view: &wgpu::TextureView, color: wgpu::Color) {
    encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("limiter reset"),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view,
            depth_slice: None,
            resolve_target: None,
            ops: wgpu::Operations { load: wgpu::LoadOp::Clear(color), store: wgpu::StoreOp::Store },
        })],
        ..Default::default()
    });
}

/// Frames between two counts of held cells.
const REPORT_EVERY: u32 = 30;

const PENDING: u8 = 0;
const READY: u8 = 1;
const FAILED: u8 = 2;

enum State {
    Idle,
    /// The copy is recorded in the frame being built.
    Copied,
    Mapping(Arc<AtomicU8>),
}

/// Copies the cell state to the CPU now and then, to count held cells, the
/// same way `GpuTimer` reads its timestamps: never waiting.
struct Readback {
    buffer: wgpu::Buffer,
    cells: (u32, u32),
    /// Bytes per row in the buffer: copies need rows of a multiple of 256.
    row: u32,
    frames: u32,
    state: State,
}

impl Readback {
    fn new(device: &wgpu::Device, cells: (u32, u32)) -> Self {
        let row = (cells.0 * 16).next_multiple_of(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT);
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("limiter readback"),
            size: u64::from(row * cells.1),
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        Self { buffer, cells, row, frames: 0, state: State::Idle }
    }

    fn maybe_copy(&mut self, encoder: &mut wgpu::CommandEncoder, cells: &wgpu::Texture) {
        self.frames = self.frames.wrapping_add(1);
        if !matches!(self.state, State::Idle) || !self.frames.is_multiple_of(REPORT_EVERY) {
            return;
        }
        encoder.copy_texture_to_buffer(
            cells.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: &self.buffer,
                layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(self.row), rows_per_image: None },
            },
            wgpu::Extent3d { width: self.cells.0, height: self.cells.1, depth_or_array_layers: 1 },
        );
        self.state = State::Copied;
    }

    fn after_submit(&mut self) {
        if !matches!(self.state, State::Copied) {
            return;
        }
        let status = Arc::new(AtomicU8::new(PENDING));
        let done = Arc::clone(&status);
        self.buffer.slice(..).map_async(wgpu::MapMode::Read, move |result| {
            done.store(if result.is_ok() { READY } else { FAILED }, Ordering::Release);
        });
        self.state = State::Mapping(status);
    }

    fn collect(&mut self, device: &wgpu::Device) -> Option<f32> {
        let State::Mapping(status) = &self.state else { return None };
        // Lets wgpu notice finished GPU work and run the callback above.
        let _ = device.poll(wgpu::PollType::Poll);
        let mut held = None;
        match status.load(Ordering::Acquire) {
            PENDING => return None,
            READY => {
                if let Ok(bytes) = self.buffer.get_mapped_range(..) {
                    let width = self.cells.0 as usize * 16;
                    let count: usize = bytes
                        .chunks(self.row as usize)
                        .map(|row| bytemuck::cast_slice::<u8, [f32; 4]>(&row[..width]).iter().filter(|c| c[3] < 0.999).count())
                        .sum();
                    held = Some(count as f32 / (self.cells.0 * self.cells.1) as f32);
                }
                self.buffer.unmap();
            }
            _ => {}
        }
        self.state = State::Idle;
        held
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FPS: f32 = 60.0;

    /// The most transitions within any second of a luminance sequence,
    /// counted without `Cell`'s bookkeeping: the longest chain of frames
    /// inside the second where each step to the next swings by `DELTA` or
    /// more, against the step before.
    pub(crate) fn worst_second(luminance: &[f32], fps: f32) -> usize {
        // Frames spanning just under a second.
        let span = fps as usize;
        let mut worst = 0;
        for start in 0..luminance.len() {
            let window = &luminance[start..(start + span).min(luminance.len())];
            // The most swings in a chain ending at frame i with a rise / a fall.
            let mut up = vec![0usize; window.len()];
            let mut down = vec![0usize; window.len()];
            for i in 0..window.len() {
                for j in 0..i {
                    if window[i] - window[j] >= DELTA - 1e-4 {
                        up[i] = up[i].max(down[j] + 1);
                    }
                    if window[j] - window[i] >= DELTA - 1e-4 {
                        down[i] = down[i].max(up[j] + 1);
                    }
                }
                worst = worst.max(up[i]).max(down[i]);
            }
        }
        worst
    }

    fn run(candidate: impl Fn(usize) -> f32, frames: usize) -> (Vec<f32>, Vec<f32>) {
        let mut cell = Cell::default();
        let input: Vec<f32> = (0..frames).map(candidate).collect();
        let shown = input.iter().map(|&y| cell.step(y, 1.0 / FPS).0).collect();
        (input, shown)
    }

    #[test]
    fn a_strobe_is_held_to_two_flashes_a_second() {
        for hz in [30.0, 15.0, 10.0, 6.0, 4.0, 3.0] {
            let (input, shown) = run(|i| if (i as f32 * hz / FPS) % 1.0 < 0.5 { 1.0 } else { 0.0 }, 600);
            assert!(worst_second(&input, FPS) > MAX_TRANSITIONS, "{hz} Hz should be a strobe");
            let worst = worst_second(&shown, FPS);
            assert!(worst <= MAX_TRANSITIONS, "{hz} Hz: {worst} transitions in a second");
        }
    }

    #[test]
    fn small_and_slow_changes_pass_untouched() {
        // A flash a second; swings of 0.08 at 10 Hz; a slow sine of full range.
        let scenes: [&dyn Fn(usize) -> f32; 3] = [
            &|i| if (i / 30) % 2 == 0 { 0.8 } else { 0.1 },
            &|i| 0.4 + if (i / 3) % 2 == 0 { 0.08 } else { 0.0 },
            &|i| 0.5 + 0.5 * (i as f32 / FPS * std::f32::consts::TAU * 0.25).sin(),
        ];
        for scene in scenes {
            let (input, shown) = run(scene, 600);
            let off = input.iter().zip(&shown).map(|(a, b)| (a - b).abs()).fold(0.0, f32::max);
            assert!(off < 1e-6, "changed by {off}");
        }
    }

    #[test]
    fn a_held_cell_recovers_once_the_strobe_stops() {
        // A strobe for 2 s, then a steady 0.7.
        let (_, shown) = run(|i| if i < 120 { (i % 2) as f32 } else { 0.7 }, 300);
        assert!((shown[299] - 0.7).abs() < 1e-6, "{}", shown[299]);
    }

    #[test]
    fn the_grid_has_even_square_cells_of_8_pixels_at_1080_rows() {
        assert_eq!(grid(1920, 1080), (8, 240, 135));
        assert_eq!(grid(960, 540), (4, 240, 135));
        assert_eq!(grid(1920, 1009), (8, 240, 127));
        assert_eq!(grid(1, 1), (2, 1, 1));
    }
}

/// The limiter on a real GPU: adversarial frames in, the screen read back
/// and checked by `tests::worst_second`, which knows nothing of cells.
/// Skipped (passing) when the machine has no GPU.
#[cfg(test)]
mod gpu_tests {
    use super::tests::worst_second;
    use super::*;

    const WIDTH: u32 = 128;
    const HEIGHT: u32 = 72;
    const FPS: f32 = 60.0;
    const LUMA: [f32; 3] = [0.2126, 0.7152, 0.0722];

    fn device() -> Option<(wgpu::Device, wgpu::Queue)> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::PRIMARY,
            ..wgpu::InstanceDescriptor::new_without_display_handle()
        });
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default())).ok()?;
        pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).ok()
    }

    /// The nearest 16-bit float, for values from 0 to 1.
    fn half(x: f32) -> u16 {
        let bits = x.max(0.0).to_bits() + 0x1000;
        let exponent = ((bits >> 23) & 0xff) as i32 - 127 + 15;
        if exponent <= 0 { 0 } else { (((exponent as u32) << 10) | ((bits >> 13) & 0x3ff)) as u16 }
    }

    fn decode(byte: u8) -> f32 {
        let c = f32::from(byte) / 255.0;
        if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
    }

    /// Runs `scene(frame, x, y)` (linear RGB) through the limiter; returns
    /// what reached the screen, in linear light, frame by frame.
    fn limit(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        frames: usize,
        scene: impl Fn(usize, u32, u32) -> [f32; 3],
    ) -> Vec<Vec<[f32; 3]>> {
        let format = wgpu::TextureFormat::Rgba8Unorm;
        let size = wgpu::Extent3d { width: WIDTH, height: HEIGHT, depth_or_array_layers: 1 };
        let mut limiter = Limiter::new(device, WIDTH, HEIGHT, format);
        let screen = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("test screen"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let screen_view = screen.create_view(&wgpu::TextureViewDescriptor::default());
        let readback = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("test readback"),
            size: u64::from(WIDTH * HEIGHT * 4),
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let mut shown = Vec::with_capacity(frames);
        for frame in 0..frames {
            let mut pixels = Vec::with_capacity((WIDTH * HEIGHT * 4) as usize);
            for y in 0..HEIGHT {
                for x in 0..WIDTH {
                    let [r, g, b] = scene(frame, x, y);
                    pixels.extend([half(r), half(g), half(b), half(1.0)]);
                }
            }
            queue.write_texture(
                limiter.images.candidate.as_image_copy(),
                bytemuck::cast_slice(&pixels),
                wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(WIDTH * 8), rows_per_image: None },
                size,
            );
            let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
            limiter.encode(queue, &mut encoder, &screen_view, 1.0 / FPS, frame as f32, true, None);
            encoder.copy_texture_to_buffer(
                screen.as_image_copy(),
                wgpu::TexelCopyBufferInfo {
                    buffer: &readback,
                    layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(WIDTH * 4), rows_per_image: None },
                },
                size,
            );
            queue.submit([encoder.finish()]);
            limiter.after_submit();
            readback.slice(..).map_async(wgpu::MapMode::Read, |_| {});
            device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
            let bytes = readback.get_mapped_range(..).unwrap();
            shown.push(bytes.chunks(4).map(|p| [decode(p[0]), decode(p[1]), decode(p[2])]).collect());
            drop(bytes);
            readback.unmap();
        }
        shown
    }

    /// The most transitions within a second in any 8×8-pixel block (16
    /// cells here, at 72 rows) or in the whole screen.
    fn worst(shown: &[Vec<[f32; 3]>]) -> usize {
        let luminance = |c: &[f32; 3]| c[0] * LUMA[0] + c[1] * LUMA[1] + c[2] * LUMA[2];
        let mean = |frame: &Vec<[f32; 3]>, (x0, y0, w, h): (u32, u32, u32, u32)| {
            let mut sum = 0.0;
            for y in y0..y0 + h {
                for x in x0..x0 + w {
                    sum += luminance(&frame[(y * WIDTH + x) as usize]);
                }
            }
            sum / (w * h) as f32
        };
        let mut regions = vec![(0, 0, WIDTH, HEIGHT)];
        for by in 0..HEIGHT / 8 {
            for bx in 0..WIDTH / 8 {
                regions.push((bx * 8, by * 8, 8, 8));
            }
        }
        regions
            .into_iter()
            .map(|region| {
                let series: Vec<f32> = shown.iter().map(|frame| mean(frame, region)).collect();
                worst_second(&series, FPS)
            })
            .max()
            .unwrap()
    }

    #[test]
    fn strobes_reach_the_screen_as_two_flashes_a_second_at_most() {
        let Some((device, queue)) = device() else {
            eprintln!("no GPU: skipped");
            return;
        };
        let square = |frame: usize, hz: f32| (frame as f32 * hz / FPS) % 1.0 < 0.5;
        type Scene<'a> = (&'a str, &'a dyn Fn(usize, u32, u32) -> [f32; 3]);
        let scenes: [Scene; 4] = [
            ("whole screen at 30 Hz", &|f, _, _| if square(f, 30.0) { [1.0; 3] } else { [0.0; 3] }),
            ("left half at 10 Hz", &|f, x, _| {
                if x >= WIDTH / 2 {
                    [0.3; 3]
                } else if square(f, 10.0) {
                    [0.9; 3]
                } else {
                    [0.05; 3]
                }
            }),
            ("checkerboard at 12 Hz, blocks out of phase", &|f, x, y| {
                let phase = ((x / 16 + y / 16) % 2) as usize * 2;
                if square(f + phase, 12.0) { [0.0, 0.8, 0.9] } else { [0.02; 3] }
            }),
            ("a slow fade broken by a 20 Hz flicker", &|f, _, _| {
                let base = 0.2 + 0.6 * f as f32 / 180.0;
                [if square(f, 20.0) { base } else { base * 0.4 }; 3]
            }),
        ];
        for (name, scene) in scenes {
            let shown = limit(&device, &queue, 180, scene);
            let worst = worst(&shown);
            assert!(worst <= MAX_TRANSITIONS, "{name}: {worst} transitions in a second");
        }
    }

    #[test]
    fn gentle_frames_pass_untouched() {
        let Some((device, queue)) = device() else {
            eprintln!("no GPU: skipped");
            return;
        };
        // A slow pulse over a gradient, with a flash a second on one side.
        let scene = |f: usize, x: u32, y: u32| {
            let pulse = 0.5 + 0.4 * (f as f32 / FPS * std::f32::consts::TAU * 0.3).sin();
            let flash = if x < 32 && (f / 30).is_multiple_of(2) { 0.4 } else { 0.0 };
            let v = (pulse * (x + y) as f32 / (WIDTH + HEIGHT) as f32 + flash).min(1.0);
            [v, v * 0.8, v * 0.6]
        };
        let shown = limit(&device, &queue, 120, scene);
        for (f, frame) in shown.iter().enumerate() {
            for (i, pixel) in frame.iter().enumerate() {
                let want = scene(f, i as u32 % WIDTH, i as u32 / WIDTH);
                for c in 0..3 {
                    // Dithering moves a pixel by up to one level in 255
                    // (0.009 in linear light near white), 16-bit floats and
                    // 8-bit rounding by less.
                    assert!((pixel[c] - want[c]).abs() < 0.015, "frame {f}, pixel {i}: {pixel:?} vs {want:?}");
                }
            }
        }
    }

    #[test]
    fn saturated_red_is_never_shown() {
        let Some((device, queue)) = device() else {
            eprintln!("no GPU: skipped");
            return;
        };
        let scene = |f: usize, x: u32, _| {
            let v = 0.2 + 0.8 * (x as f32 / WIDTH as f32) * (1.0 - (f % 60) as f32 / 120.0);
            [v, 0.02 * v, 0.01 * v]
        };
        let shown = limit(&device, &queue, 30, scene);
        for frame in &shown {
            for pixel in frame {
                let total: f32 = pixel.iter().sum();
                // Rounding to 8 bits can move a dark pixel's share a little.
                if total > 0.05 {
                    assert!(pixel[0] / total <= RED_SHARE + 0.01, "{pixel:?}");
                }
            }
        }
    }
}
