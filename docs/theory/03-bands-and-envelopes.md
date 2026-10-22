# 03 — Bands, automatic gain and envelopes

The FFT returns 1025 numbers per frame. The visuals want a handful, all in `0..1`, that move like the music: they jump on a kick and breathe out afterwards, and they behave the same whether the song is mastered loud or quiet. This note covers the three steps in `crates/dsp/src/analyzer.rs` and `smoothing.rs` that get there.

## 1. Bands

**Grouping bins.** A band is a range of bins. Its level is the sum of the bins' power `|X[k]|²`, scaled back into the signal's mean square (by Parseval's theorem: the energy in the frequency domain equals the energy in the time domain). Its square root is the RMS of whatever is in that band. A test checks that a full-scale sine measures −3 dBFS, the same as the time-domain meter from Milestone 1.

**The three musical bands:**

| Band | Range | Typically |
|---|---|---|
| `bass` | 20–250 Hz | kick, bass, the "body" |
| `mid` | 250–4000 Hz | voice, guitars, synths, snare |
| `treble` | 4–16 kHz | hi-hats, cymbals, air |

Plus `energy`, the whole range together.

**The log spectrum.** For bar displays and per-band effects there are also 32 bands, spaced **logarithmically** from 30 Hz to 16 kHz. Hearing perceives pitch by ratio, not by difference: 100→200 Hz and 1000→2000 Hz both sound like one octave. Linear bands would spend almost all of the display on the treble; log bands give each musical interval the same width.

Log bands expose the FFT's limit at the bottom: between 30 and 100 Hz there are only three bins, but several bands, so neighboring low bands share bins and show the same height. A larger FFT or a dedicated low-frequency analysis can fix that later.

## 2. Automatic gain

Songs differ wildly in loudness, and so do system volumes. Fixed scaling would make a quiet track barely move and a loud one stuck at the top. **Auto-gain** normalizes each band to what it has been doing recently:

- It tracks the band's **recent peak** in dB: it jumps up instantly when the level exceeds it, then **sinks slowly** (3 dB per second).
- The output is where the level sits within the **30 dB below that peak**: the peak maps to 1, 30 dB below to 0.

So a song that turns 20 dB quieter re-levels itself in about seven seconds, while short dips (a break, a quiet bar) still read as quiet.

Two safeguards keep auto-gain from amplifying nothing:

- **Silence floor.** The peak never sinks below −60 dBFS. Background hiss or a muted system stays at 0 instead of being boosted to full scale.
- **Band spread.** A band's peak never sinks more than 18 dB below the *overall* peak. Inside that spread every band uses its full range, even in a bass-heavy mix. A band far below it (nearly empty, like the treble of a pure bass tone) stays low. A test pins this down: with a 24 dB spread, a treble 48 dB under the bass still read 20% of full scale; at 18 dB it reads zero.

The 32-band spectrum is the exception: it shares **one** gain driven by its loudest band, so the bars keep the real shape of the mix (a bass-heavy song looks bass-heavy).

## 3. Envelopes

Even after gain, per-frame values flicker: one frame catches a drum hit, the next falls between two. Visuals driven directly by that look nervous. An **envelope follower** smooths it asymmetrically:

- **Attack** (rising): fast, 10–30 ms. The value reaches a hit almost immediately.
- **Release** (falling): slow, 150–400 ms. After the hit it decays gradually.

That asymmetry is the whole feel: punchy on the beat, breathing in between. Each step moves the value toward the input by a fraction `1 − e^(−dt/τ)`, where `τ` is the attack or release time. After one `τ` it has covered 63% of the way.

**Why `e^(−dt/τ)` and not a fixed factor.** The common shortcut `value += (input − value) · 0.1` moves 10% *per frame*, so the smoothing changes with the frame rate: twice as fast at 120 fps as at 60. Using the real elapsed time makes the result identical at any frame rate; a test checks that a hundred 1 ms steps equal one 100 ms step.

## Silence

WASAPI loopback sends no packets at all while nothing plays. If the analyzer just reused its last window, the visuals would freeze on the last sound forever. Instead, a frame with no new samples is treated as elapsed silence: zeros are written into the history for that time, and everything decays to 0 through the normal release (a test checks this).

## The result

Each frame produces `bass`, `mid`, `treble`, `energy` and 32 spectrum bands, all in `0..1`, smoothed, volume independent, and costing about 0.06% of the CPU together with capture. These are the first fields of the `Signals` that will drive the visuals from Milestone 3 onward.
