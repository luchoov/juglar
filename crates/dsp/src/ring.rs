//! Lock-free single-producer/single-consumer ring buffer of samples.
//!
//! The audio thread pushes, the render thread pops. Neither ever blocks or
//! allocates: when the buffer is full, new samples are dropped and counted.
//! The consumer drains everything each frame and keeps its own analysis
//! window, so overflow only happens if it stops reading for a while.

use std::cell::UnsafeCell;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

struct Shared {
    slots: Box<[UnsafeCell<f32>]>,
    mask: usize,
    /// Total samples ever written. Only the producer stores it.
    head: AtomicUsize,
    /// Total samples ever read. Only the consumer stores it.
    tail: AtomicUsize,
    dropped: AtomicUsize,
}

// SAFETY: a slot is written only by the producer while it lies in the free
// region (between head and tail + capacity), and read only by the consumer
// while it lies in the filled region (between tail and head). The
// Release/Acquire pairs on head and tail hand each slot over between them.
unsafe impl Sync for Shared {}

impl Shared {
    fn capacity(&self) -> usize {
        self.mask + 1
    }
}

/// Creates a ring buffer holding at least `capacity` samples (rounded up to a
/// power of two).
pub fn ring(capacity: usize) -> (Producer, Consumer) {
    let capacity = capacity.max(2).next_power_of_two();
    let shared = Arc::new(Shared {
        slots: (0..capacity).map(|_| UnsafeCell::new(0.0)).collect(),
        mask: capacity - 1,
        head: AtomicUsize::new(0),
        tail: AtomicUsize::new(0),
        dropped: AtomicUsize::new(0),
    });
    (Producer { shared: shared.clone() }, Consumer { shared })
}

pub struct Producer {
    shared: Arc<Shared>,
}

impl Producer {
    /// Writes as many samples as fit and returns how many were written. The
    /// rest are dropped.
    pub fn push(&mut self, samples: &[f32]) -> usize {
        let s = &*self.shared;
        let head = s.head.load(Ordering::Relaxed);
        let tail = s.tail.load(Ordering::Acquire);
        let free = s.capacity() - head.wrapping_sub(tail);
        let n = free.min(samples.len());
        for (i, &sample) in samples[..n].iter().enumerate() {
            // SAFETY: the slot is in the free region, see `Shared`.
            unsafe { *s.slots[head.wrapping_add(i) & s.mask].get() = sample };
        }
        s.head.store(head.wrapping_add(n), Ordering::Release);
        if n < samples.len() {
            s.dropped.fetch_add(samples.len() - n, Ordering::Relaxed);
        }
        n
    }
}

pub struct Consumer {
    shared: Arc<Shared>,
}

impl Consumer {
    /// Reads up to `out.len()` samples in arrival order and returns how many
    /// were read.
    pub fn pop(&mut self, out: &mut [f32]) -> usize {
        let s = &*self.shared;
        let tail = s.tail.load(Ordering::Relaxed);
        let head = s.head.load(Ordering::Acquire);
        let n = head.wrapping_sub(tail).min(out.len());
        for (i, slot) in out[..n].iter_mut().enumerate() {
            // SAFETY: the slot is in the filled region, see `Shared`.
            *slot = unsafe { *s.slots[tail.wrapping_add(i) & s.mask].get() };
        }
        s.tail.store(tail.wrapping_add(n), Ordering::Release);
        n
    }

    /// Samples waiting to be read.
    pub fn len(&self) -> usize {
        let s = &*self.shared;
        s.head.load(Ordering::Acquire).wrapping_sub(s.tail.load(Ordering::Relaxed))
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Samples dropped so far because the buffer was full.
    pub fn dropped(&self) -> usize {
        self.shared.dropped.load(Ordering::Relaxed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rounds_capacity_up_to_a_power_of_two() {
        let (mut tx, _rx) = ring(5);
        assert_eq!(tx.push(&[0.0; 10]), 8);
    }

    #[test]
    fn preserves_order_across_wraparound() {
        let (mut tx, mut rx) = ring(4);
        let mut out = [0.0; 4];
        for round in 0..10 {
            let base = round as f32 * 3.0;
            assert_eq!(tx.push(&[base, base + 1.0, base + 2.0]), 3);
            assert_eq!(rx.pop(&mut out), 3);
            assert_eq!(&out[..3], &[base, base + 1.0, base + 2.0]);
        }
    }

    #[test]
    fn drops_and_counts_samples_when_full() {
        let (mut tx, mut rx) = ring(4);
        assert_eq!(tx.push(&[1.0, 2.0, 3.0, 4.0, 5.0, 6.0]), 4);
        assert_eq!(rx.dropped(), 2);
        let mut out = [0.0; 8];
        assert_eq!(rx.pop(&mut out), 4);
        assert_eq!(&out[..4], &[1.0, 2.0, 3.0, 4.0]);
        assert!(rx.is_empty());
    }

    #[test]
    fn delivers_every_sample_in_order_across_threads() {
        const TOTAL: usize = 200_000;
        let (mut tx, mut rx) = ring(1024);
        let writer = std::thread::spawn(move || {
            let mut next = 0;
            while next < TOTAL {
                let chunk: Vec<f32> = (next..(next + 64).min(TOTAL)).map(|i| i as f32).collect();
                next += tx.push(&chunk);
            }
        });
        let mut expected = 0;
        let mut out = [0.0; 100];
        while expected < TOTAL {
            let n = rx.pop(&mut out);
            for &sample in &out[..n] {
                assert_eq!(sample, expected as f32);
                expected += 1;
            }
        }
        // The writer retries whatever didn't fit, so `dropped` counts those
        // retries; what matters is that every sample arrived exactly once, in order.
        writer.join().unwrap();
    }
}
