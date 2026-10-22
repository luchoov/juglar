# 01 — Capturing system audio

Milestone 1 gets the raw material: the exact samples your speakers are playing, from any app, delivered to the render thread without ever stalling either side.

## What a sample is

Sound is air pressure changing over time. A sound card measures (or, for playback, produces) that pressure tens of thousands of times per second. Each measurement is a **sample**; how many per second is the **sample rate** (48,000 Hz on most PCs). With two channels, samples come **interleaved**: left, right, left, right. One left+right pair is a **frame**.

Windows mixes every app's audio into one stream, the **mix format**. In shared mode it is 32-bit float, one value per sample in `-1.0..=1.0`, where `±1.0` is the loudest the device can play ("full scale").

## Loopback: recording the speakers

WASAPI is Windows' low-level audio API. Normally you open a *capture* device (a microphone) to record. **Loopback** lets you open a *render* device (the speakers) for capture instead: the engine hands you a copy of the final mix right before it reaches the hardware. No virtual cables, no app integration — Juglar sees whatever is playing.

Setup, in `crates/platform/src/win32/wasapi.rs`:

1. `IMMDeviceEnumerator::GetDefaultAudioEndpoint(eRender, eConsole)` — the default output device.
2. `IAudioClient::GetMixFormat` — learn the sample rate and channel count, and check it is 32-bit float.
3. `IAudioClient::Initialize(SHARED, LOOPBACK | EVENTCALLBACK, ...)` — capture the mix, wake us with an event.
4. `IAudioCaptureClient::GetBuffer` / `ReleaseBuffer` — borrow each packet, hand it on, give it back.

## Why event-driven

The engine works in **periods** of about 10 ms. Instead of polling ("any data yet?") in a loop that burns CPU, the capture thread blocks on a Windows event that the engine signals once per period. Between packets the thread is asleep and costs nothing. That is why capture runs below what Windows can even measure.

One quirk: while nothing is playing, loopback sends **no packets at all** rather than packets of zeros. The wait therefore has a 100 ms timeout, so the thread can still notice when it is asked to stop.

## Why its own thread

The engine expects us to drain the buffer every period. If capture shared a thread with rendering, a slow frame (a shader compiling, a window resize) would make us miss packets and the audio would glitch. On its own thread, capture keeps the engine's pace no matter what the renderer is doing. COM objects are tied to the thread that created them, so every WASAPI object lives and dies on the capture thread.

## Why a lock-free ring buffer

The two threads still have to exchange data. A `Mutex<Vec<f32>>` would work until the moment the renderer holds the lock while the audio thread needs it: the audio thread would block, which is exactly what we must avoid. Audio code has one golden rule: **the audio callback never blocks and never allocates**.

`crates/dsp/src/ring.rs` is a **single-producer/single-consumer (SPSC) ring buffer**:

- A fixed array of slots, sized to a power of two so `index & mask` replaces a modulo.
- `head` counts samples ever written; only the producer changes it. `tail` counts samples ever read; only the consumer changes it. `head - tail` is how many are waiting.
- The producer writes into free slots, then publishes them with a **Release** store of `head`. The consumer reads `head` with an **Acquire** load, which guarantees it sees those slot writes, reads them, then frees the slots with a Release store of `tail`.

No locks, no waiting: each side only ever looks at the other's counter. If the buffer is full (the renderer stopped reading for a whole second), new samples are dropped and counted instead of blocking the audio thread.

## Measuring loudness

To check that everything works, Milestone 1 prints a level meter (`crates/dsp/src/level.rs`):

- **Peak**: the largest absolute sample in a block. It shows the transients.
- **RMS** (root mean square): `sqrt(mean(sample²))`. It tracks perceived loudness much better than the peak. A full-scale sine wave has RMS `1/√2 ≈ 0.707`.
- **dBFS**: decibels relative to full scale, `20·log10(amplitude)`. Full scale is 0 dBFS and everything quieter is negative; halving the amplitude is about −6 dB. The scale is logarithmic because hearing is.

Stereo is downmixed to mono (the average of both channels) before entering the ring buffer; the analysis in the next milestones works on mono.

## What comes next

Loudness alone can't tell a kick drum from a cymbal. Milestone 2 takes these samples into the frequency domain with an FFT, so each part of the visuals can react to its own part of the music.
