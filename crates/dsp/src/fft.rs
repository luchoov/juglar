//! Iterative radix-2 Cooley–Tukey FFT.
//!
//! `Fft::new` precomputes everything that depends only on the size (twiddle
//! factors and the bit-reversal permutation), so `process` does no allocation
//! and no trigonometry: just `n/2 · log2(n)` butterflies, in place.

use std::f64::consts::TAU;
use std::ops::{Add, Mul, Sub};

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Complex {
    pub re: f32,
    pub im: f32,
}

impl Complex {
    pub const ZERO: Self = Self { re: 0.0, im: 0.0 };

    pub fn new(re: f32, im: f32) -> Self {
        Self { re, im }
    }

    /// `|z|²`, the power of this bin. Cheaper than the magnitude and what
    /// band energies are summed from.
    pub fn norm_sqr(self) -> f32 {
        self.re * self.re + self.im * self.im
    }
}

impl Add for Complex {
    type Output = Self;
    fn add(self, rhs: Self) -> Self {
        Self::new(self.re + rhs.re, self.im + rhs.im)
    }
}

impl Sub for Complex {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self {
        Self::new(self.re - rhs.re, self.im - rhs.im)
    }
}

impl Mul for Complex {
    type Output = Self;
    fn mul(self, rhs: Self) -> Self {
        Self::new(self.re * rhs.re - self.im * rhs.im, self.re * rhs.im + self.im * rhs.re)
    }
}

pub struct Fft {
    n: usize,
    /// `e^(-2πik/n)` for `k` in `0..n/2`.
    twiddles: Vec<Complex>,
    /// `reversed[i]` is `i` with its `log2(n)` bits reversed.
    reversed: Vec<u32>,
}

impl Fft {
    /// # Panics
    /// If `n` is not a power of two of at least 2.
    pub fn new(n: usize) -> Self {
        assert!(n >= 2 && n.is_power_of_two(), "FFT size must be a power of two >= 2, got {n}");
        let bits = n.trailing_zeros();
        // Computed in f64 so the table itself adds no rounding error.
        let twiddles = (0..n / 2)
            .map(|k| {
                let angle = -TAU * k as f64 / n as f64;
                Complex::new(angle.cos() as f32, angle.sin() as f32)
            })
            .collect();
        let reversed = (0..n as u32).map(|i| i.reverse_bits() >> (32 - bits)).collect();
        Self { n, twiddles, reversed }
    }

    pub fn len(&self) -> usize {
        self.n
    }

    pub fn is_empty(&self) -> bool {
        false
    }

    /// Transforms `buf` in place from the time domain to the frequency
    /// domain: `X[k] = Σ x[t]·e^(-2πikt/n)`, unnormalized.
    ///
    /// # Panics
    /// If `buf.len()` differs from the size given to `new`.
    pub fn process(&self, buf: &mut [Complex]) {
        assert_eq!(buf.len(), self.n, "buffer length must match the FFT size");

        // Reorder so each butterfly stage can work on contiguous blocks.
        for (i, &j) in self.reversed.iter().enumerate() {
            let j = j as usize;
            if j > i {
                buf.swap(i, j);
            }
        }

        // Merge pairs of half-size transforms into full-size ones: sizes 2, 4, ..., n.
        let mut size = 2;
        while size <= self.n {
            let half = size / 2;
            let stride = self.n / size;
            for block in buf.chunks_exact_mut(size) {
                let (evens, odds) = block.split_at_mut(half);
                for (k, (even, odd)) in evens.iter_mut().zip(odds.iter_mut()).enumerate() {
                    let twisted = *odd * self.twiddles[k * stride];
                    *odd = *even - twisted;
                    *even = *even + twisted;
                }
            }
            size *= 2;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The definition, `O(n²)`, computed in f64: the reference the fast
    /// version is checked against.
    fn naive_dft(input: &[Complex]) -> Vec<Complex> {
        let n = input.len();
        (0..n)
            .map(|k| {
                let (mut re, mut im) = (0.0f64, 0.0f64);
                for (t, x) in input.iter().enumerate() {
                    let angle = -TAU * (k * t) as f64 / n as f64;
                    let (s, c) = angle.sin_cos();
                    re += x.re as f64 * c - x.im as f64 * s;
                    im += x.re as f64 * s + x.im as f64 * c;
                }
                Complex::new(re as f32, im as f32)
            })
            .collect()
    }

    /// Deterministic pseudo-random samples in `-1..1`.
    fn noise(n: usize) -> Vec<Complex> {
        let mut state = 0x2545_f491_u32;
        (0..n)
            .map(|_| {
                let mut next = || {
                    state ^= state << 13;
                    state ^= state >> 17;
                    state ^= state << 5;
                    state as f32 / u32::MAX as f32 * 2.0 - 1.0
                };
                Complex::new(next(), next())
            })
            .collect()
    }

    #[test]
    fn matches_the_naive_dft() {
        for n in [2, 4, 8, 64, 1024] {
            let input = noise(n);
            let expected = naive_dft(&input);
            let mut actual = input.clone();
            Fft::new(n).process(&mut actual);
            let tolerance = 1e-4 * n as f32;
            for (k, (a, e)) in actual.iter().zip(&expected).enumerate() {
                assert!(
                    (a.re - e.re).abs() < tolerance && (a.im - e.im).abs() < tolerance,
                    "n={n} bin {k}: got {a:?}, expected {e:?}"
                );
            }
        }
    }

    #[test]
    fn impulse_has_a_flat_spectrum() {
        let mut buf = vec![Complex::ZERO; 16];
        buf[0] = Complex::new(1.0, 0.0);
        Fft::new(16).process(&mut buf);
        assert!(buf.iter().all(|x| (x.re - 1.0).abs() < 1e-6 && x.im.abs() < 1e-6));
    }

    #[test]
    fn sine_lands_in_its_bin() {
        let n = 256;
        let bin = 10;
        let mut buf: Vec<Complex> = (0..n)
            .map(|t| Complex::new((TAU * (bin * t) as f64 / n as f64).sin() as f32, 0.0))
            .collect();
        Fft::new(n).process(&mut buf);
        // A real sine of amplitude 1 puts n/2 in bin k and its mirror n-k.
        for (k, x) in buf.iter().enumerate() {
            let expected = if k == bin || k == n - bin { n as f32 / 2.0 } else { 0.0 };
            assert!((x.norm_sqr().sqrt() - expected).abs() < 1e-3, "bin {k}: {x:?}");
        }
    }

    #[test]
    #[should_panic(expected = "power of two")]
    fn rejects_sizes_that_are_not_powers_of_two() {
        Fft::new(100);
    }
}
