//! Measuring how long the GPU spends on a frame.
//!
//! The CPU can't time GPU work with a stopwatch: it only records commands,
//! and the GPU runs them later. Instead the GPU writes its own clock into a
//! small buffer at the start of the frame's first render pass and the end of
//! its last. Reading that buffer back takes a frame or two, so the timer
//! never waits for it: it starts a measurement, keeps rendering, and picks
//! the result up once it is ready. While one measurement is in flight,
//! frames go untimed.

use std::sync::Arc;
use std::sync::atomic::{AtomicU8, Ordering};

const PENDING: u8 = 0;
const READY: u8 = 1;
const FAILED: u8 = 2;

/// Two timestamps of 8 bytes each: start and end of the frame's drawing.
const BYTES: u64 = 2 * size_of::<u64>() as u64;

enum State {
    Idle,
    /// Timestamps are being written in the frame being recorded.
    Recording,
    /// Copied to `readback`, waiting for the GPU to finish and hand it over.
    Mapping(Arc<AtomicU8>),
}

pub(crate) struct GpuTimer {
    queries: wgpu::QuerySet,
    /// Where the GPU resolves its raw timestamps; it can't be read directly.
    resolve: wgpu::Buffer,
    /// A copy the CPU can read.
    readback: wgpu::Buffer,
    /// Nanoseconds per timestamp tick.
    period_ns: f32,
    state: State,
    latest_ms: Option<f32>,
}

impl GpuTimer {
    pub(crate) fn new(device: &wgpu::Device, period_ns: f32) -> Self {
        let buffer = |label, usage| {
            device.create_buffer(&wgpu::BufferDescriptor { label: Some(label), size: BYTES, usage, mapped_at_creation: false })
        };
        Self {
            queries: device.create_query_set(&wgpu::QuerySetDescriptor {
                label: Some("frame timestamps"),
                ty: wgpu::QueryType::Timestamp,
                count: 2,
            }),
            resolve: buffer("timestamp resolve", wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC),
            readback: buffer("timestamp readback", wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST),
            period_ns,
            state: State::Idle,
            latest_ms: None,
        }
    }

    /// Starts timing the frame being recorded, unless a measurement is
    /// still in flight. Returns whether this frame is timed.
    pub(crate) fn begin(&mut self) -> bool {
        if matches!(self.state, State::Idle) {
            self.state = State::Recording;
            true
        } else {
            false
        }
    }

    /// Timestamps for a render pass: the frame's start if it is the first
    /// pass, its end if it is the last.
    pub(crate) fn pass_writes(&self, first: bool, last: bool) -> wgpu::RenderPassTimestampWrites<'_> {
        wgpu::RenderPassTimestampWrites {
            query_set: &self.queries,
            beginning_of_pass_write_index: first.then_some(0),
            end_of_pass_write_index: last.then_some(1),
        }
    }

    /// Records the copy of the timestamps into the readable buffer.
    pub(crate) fn resolve(&self, encoder: &mut wgpu::CommandEncoder) {
        encoder.resolve_query_set(&self.queries, 0..2, &self.resolve, 0);
        encoder.copy_buffer_to_buffer(&self.resolve, 0, &self.readback, 0, BYTES);
    }

    /// Asks for the readable buffer once the submitted frame finishes.
    pub(crate) fn after_submit(&mut self) {
        let status = Arc::new(AtomicU8::new(PENDING));
        let done = Arc::clone(&status);
        self.readback.slice(..).map_async(wgpu::MapMode::Read, move |result| {
            done.store(if result.is_ok() { READY } else { FAILED }, Ordering::Release);
        });
        self.state = State::Mapping(status);
    }

    /// Picks up a finished measurement, without waiting for one.
    pub(crate) fn collect(&mut self, device: &wgpu::Device) {
        let State::Mapping(status) = &self.state else { return };
        // Lets wgpu notice finished GPU work and run the callback above.
        let _ = device.poll(wgpu::PollType::Poll);
        match status.load(Ordering::Acquire) {
            PENDING => return,
            READY => {
                if let Ok(bytes) = self.readback.get_mapped_range(..) {
                    let [start, end]: [u64; 2] = bytemuck::pod_read_unaligned(&bytes);
                    // A GPU may reset or reorder clocks between the two
                    // writes (power state changes); such samples are dropped.
                    if end > start {
                        self.latest_ms = Some((end - start) as f32 * self.period_ns / 1e6);
                    }
                }
                self.readback.unmap();
            }
            _ => {}
        }
        self.state = State::Idle;
    }

    pub(crate) fn take(&mut self) -> Option<f32> {
        self.latest_ms.take()
    }
}
