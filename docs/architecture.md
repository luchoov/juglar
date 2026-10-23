# Architecture

Juglar turns music into live, procedurally composed visuals on the GPU while keeping the CPU close to idle. This document fixes the shape of the system before any code exists, so every later piece has an obvious home.

## Principles

1. **The art is OS-agnostic.** Everything that decides what you see (analysis, generation, shaders, presets, transitions) is plain Rust plus WGSL. It never calls an operating-system API. Each OS contributes only a thin `platform` implementation.
2. **The generator never sees audio.** It consumes `Signals`, a small struct of normalized values (bands, energy, beats, time). Audio is just one producer of `Signals`; time, scroll, mouse or a synthetic rhythm are others. This is what makes the web library, the screensaver and the live demo possible with the same core.
3. **Everything visual is written from scratch.** No visualization engines, no borrowed presets or shaders. Libraries are used only for plumbing: windowing (`winit`), GPU access (`wgpu`), POD casting (`bytemuck`) and OS bindings (`windows`).
4. **The CPU does almost nothing.** Per frame it reads audio, runs one FFT and uploads a few dozen floats. All per-pixel work happens in shaders.
5. **Cheap on a shared GPU.** Juglar runs next to games. Feedback renders at reduced resolution, frame rate is capped, and rendering stops entirely when nothing is visible or nothing is playing.

## Data flow

```
platform (per OS)            core (OS-agnostic)                         GPU
─────────────────            ──────────────────                         ───
AudioCapture ──samples──► ring buffer ──► dsp ──► Signals ──► generator ──► Scene
 (WASAPI loopback,            (lock-free,      FFT, bands,      composes      │
  own thread)                  SPSC)           envelopes,       building      ▼
                                               beats, loudness  blocks     render
NowPlaying ──track info──────────────────────────────────────► hud        passes:
                                                                           1 feedback (ping-pong, rgba16f, half-res):
                                                                             warp + fade the last picture, paint sources faintly
                                                                           2 bloom (bright parts at 1/4 size, halved twice)
                                                                           3 composite (sources sharp on top, palette,
                                                                             tonemap, glow) ─► candidate (rgba16f, linear)
                                                                           4 flash limiter (per cell: count flashes,
                                                                             hold back; per pixel: red guard,
                                                                             dither) ─► swapchain
```

### Threads

| Thread | Owns | Talks to others through |
|---|---|---|
| Audio | The OS capture client; event-driven, never blocks on the renderer | Single-producer/single-consumer ring buffer of `f32` samples |
| Main/render | Window, event loop, `dsp`, `generator`, `render` | — |
| Shader compiler | Building WGSL for the *next* scene off the hot path | Channel returning a ready pipeline before a transition starts |

The render thread never waits for audio. The audio callback downmixes to mono and pushes into the ring buffer; the render thread drains it every frame and keeps its own analysis window. If the ring buffer is short, the analysis reuses the last window; if it is ever full, new samples are dropped and counted rather than blocking the audio thread.

## Workspace layout

```
juglar/
├─ app/                 binary: wires platform + core together, event loop, input, config
└─ crates/
   ├─ signals/          the `Signals` struct and its producers that need no OS (time, synthetic)
   ├─ dsp/              SPSC ring buffer, levels, FFT (own radix-2), windowing, bands, envelopes, beat/onset detection, auto-gain
   ├─ director/         what to show and when: natural intensity, scene choice, transitions, from slow summaries of the signals
   ├─ generator/        building blocks, composition rules, mutation, WGSL emission
   ├─ render/           wgpu setup, render graph, ping-pong targets, bloom, composite, HUD glyphs
   └─ platform/         traits + per-OS modules
      ├─ src/lib.rs     AudioCapture, NowPlaying, DesktopMode, Tray, ForegroundWatch traits
      └─ src/win32/     WASAPI loopback, GSMTC, WorkerW wallpaper, tray, fullscreen-game detection
```

`signals`, `dsp`, `director`, `generator` and `render` must compile for `wasm32-unknown-unknown`. CI enforces it, so an OS dependency can never leak into the core.

## Key types (sketch)

```rust
/// Everything the generator is allowed to know about the outside world.
/// All fields are normalized and smoothed; producers own the smoothing.
/// Lives in `crates/signals`, next to the synthetic song (`JUGLAR_DEMO=1`).
pub struct Signals {
    pub time: f32,        // seconds since start
    pub dt: f32,          // seconds since last frame
    pub bass: f32,        // 0..1, envelope-followed
    pub mid: f32,
    pub treble: f32,
    pub energy: f32,      // overall loudness after auto-gain
    pub beat: f32,        // 1.0 on a detected beat, decays to 0
    pub novelty: f32,     // spectral flux, high on drops and section changes
    pub kick: f32,        // each drum's hits apart: 1 at a hit, back to 0 in ~0.05 s
    pub snare: f32,
    pub cymbals: f32,
    pub percussive: f32,  // 0..1, the struck part of the sound
    pub harmonic: f32,    // 0..1, the held part
    pub relative: Relative, // levels over their own last 4 s, 1 = usual
    pub intensity: f32,   // 0..1, fixed by the user or derived from the music (auto mode)
    pub spectrum: [f32; 32],  // log-spaced bands, 0..1
    pub waveform: [f32; 256], // a stable (triggered) snapshot of the wave, -1..1
}

/// One generated effect: which blocks, their parameters, and how signals drive them.
pub struct Scene { /* blocks, palette, signal routing, seed */ }
```

## Generator

Effects are not stored; they are composed at runtime from hand-made building blocks:

- **Warps**: zoom, rotation, swirl, kaleidoscope mirror, noise displacement.
- **Sources**: waveform, geometric shapes, particles.
- **Color**: cosine palettes, hue drift, contrast.
- **Routing**: which signal drives which parameter, and how strongly.

Composition rules keep every result good-looking (for example, at most two strong warps at once, palettes always harmonic, parameter ranges bounded by `intensity`). The generator emits WGSL for the chosen combination; the shader compiler thread builds the pipeline so a transition never stalls a frame.

Planned levels: random parameters per block (1), procedural composition (2), music-driven choice of scene family (3), mutation of the previous scene plus saved favorites (4).

## Intensity

A single `intensity` value in `0..1` bounds the generator: motion speed, amplitude, reactivity, scene length, transition style (long crossfades vs. cuts on beats), symmetry complexity and saturation. Presets map to fixed points (`gentle ≈ 0.2`, `medium ≈ 0.5`, `strong ≈ 0.9`); `auto` derives it from energy, onset density, bass weight and novelty, smoothed over several seconds.

A global flash limiter applies in every mode and every view: at most two flashes in any second, in any 8×8-pixel cell, and no saturated red (`docs/research/focus-and-relaxation.md` section 9 for the numbers, `docs/theory/09-photosensitivity.md` for how).

## Platform traits

```rust
pub trait AudioCapture: Sized { fn start(on_samples: SampleCallback) -> Result<Self>; fn info(&self) -> StreamInfo; } // stops on drop
pub trait NowPlaying    { fn current(&self) -> Option<Track>; }          // optional per OS
pub trait DesktopMode   { fn attach_as_wallpaper(&mut self, window: &Window) -> Result<()>; }
pub trait ForegroundWatch { fn fullscreen_app_active(&self) -> bool; }  // pause/eco while gaming
```

Missing capabilities are `None`, not errors: macOS without a public now-playing API simply never shows track titles.

## Targets

| Target | Status | Notes |
|---|---|---|
| Windows 11 desktop app | first | borderless, fullscreen, mini, live wallpaper, tray |
| Windows screensaver | later | same binary renamed to `.scr`, handles `/s` `/p` `/c` |
| Linux | later | PipeWire capture, MPRIS, tray |
| macOS | later | Core Audio process taps, needs signing for the permission prompt |
| Web (WASM + WebGPU) | later | live demo on juglar.dev and an npm web component driven by time/scroll/mouse/audio |
