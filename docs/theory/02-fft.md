# 02 — From time to frequency: the FFT

Milestone 1 measured *how loud* the music is. That can't tell a kick drum from a cymbal: both are loud. To let each part of the visuals react to its own part of the music, we need to know *which frequencies* are sounding right now. That is what the Fourier transform gives us.

## The idea

Any short piece of sound can be described as a sum of sine waves of different frequencies, each with its own amplitude and phase. The **discrete Fourier transform (DFT)** takes `N` samples and finds those amplitudes for `N` evenly spaced frequencies:

```
X[k] = Σ  x[t] · e^(−2πi·k·t/N)      for t = 0..N−1
```

Read it as: for each frequency `k`, multiply the signal by a sine/cosine wave of that frequency and add everything up. If the signal contains that frequency, the products line up and the sum is large; if not, they cancel out. `X[k]` is a complex number: its magnitude is "how much of frequency `k`", its angle is the phase (which we ignore).

Each `k` is a **bin**, `sample_rate / N` hertz wide. With `N = 2048` at 48 kHz, bins are **23.4 Hz** apart, up to the **Nyquist frequency** of 24 kHz (half the sample rate; nothing above it can be represented). For a real signal the upper half of the bins mirrors the lower half, so only bins `0..=N/2` carry information.

## Why "fast"

Computed literally, the DFT costs `N²` multiplications: 4 million for 2048 samples, 30 times per second. The **fast Fourier transform** (Cooley–Tukey, 1965) gets the same result in `N·log2(N)` steps, about 22,000 here, roughly 190 times fewer.

The trick is divide and conquer. A DFT of size `N` equals two DFTs of size `N/2`, one over the even samples and one over the odd ones, combined with one **butterfly** per pair:

```
even' = even + w·odd
odd'  = even − w·odd        where w = e^(−2πi·k/N) is a "twiddle factor"
```

Split recursively down to size 1 and you need `log2(N)` stages of `N/2` butterflies. That only works when `N` is a power of two, which is why the FFT size is 2048 and not 2000 (this is *radix-2*).

## How `crates/dsp/src/fft.rs` does it

- **No recursion.** First the samples are reordered by **bit reversal** (index `0b00000000011` goes to `0b11000000000`): that is exactly the order the recursion would end up in. Then the stages run bottom-up, merging pairs of size 1 into size 2, then 4, up to `N`, all in place in one buffer.
- **Everything precomputed.** Twiddle factors and the bit-reversal table depend only on `N`, so `Fft::new` computes them once (in `f64`, so the table adds no rounding error). Each frame, `process` does no allocation and no trigonometry.
- **Tested against the definition.** The tests compare the FFT with a literal, `N²`, double-precision DFT on pseudo-random input for sizes 2 to 1024, plus known cases: an impulse has a flat spectrum, and a pure sine lands in exactly its bin and its mirror.

We feed real samples into a complex FFT with zero imaginary parts. A real-input FFT could do half the work, but at 0.06% of the CPU there is nothing to gain yet.

## Windowing

The FFT assumes its `N` samples repeat forever. A real block almost never ends where it started, so that imaginary repetition has a jump at the seam, and a jump contains every frequency. The result is **spectral leakage**: a pure tone smears across the whole spectrum.

The fix is to fade each block in and out before transforming it, multiplying by a **window**. Juglar uses the **Hann window**, `0.5 − 0.5·cos(2πt/N)`, which starts and ends at zero (no jump) and is 1 in the middle. The cost is a slightly wider peak (a tone now spreads over ~4 bins instead of 1), in exchange for leakage that drops off very quickly. That trade-off is why the Hann window is the default choice for analysis.

## Choosing N: time against frequency

A bigger window sees more cycles, so it separates frequencies better, but it averages over a longer time, so it reacts later. You can't have both; it's the time–frequency uncertainty of signal processing.

| N at 48 kHz | Window | Bin spacing |
|---|---|---|
| 1024 | 21 ms | 46.9 Hz |
| **2048** | **43 ms** | **23.4 Hz** |
| 4096 | 85 ms | 11.7 Hz |

2048 is the usual compromise for visuals: fast enough to feel instant, fine enough to keep a kick (~50–100 Hz) apart from a bass line. Its weak spot shows in the lowest octaves, where one bin is a large musical interval; the next note covers what that means for the spectrum display.
