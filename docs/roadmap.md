# Roadmap

Each milestone ends with something visible or measurable, and with a short theory note in `docs/theory/` explaining what was built and why. Read the note before moving on.

## M1 — Hearing the system

- `platform::windows`: WASAPI loopback capture on the default render device, event-driven, on its own thread.
- Lock-free SPSC ring buffer into the main thread.
- Debug output: RMS level and peak in the terminal.
- Theory: `01-audio-capture.md` (sample rate, frames, loopback, why event-driven, why lock-free).

**Done when:** playing music prints a moving level; silence prints ~0; CPU stays under 0.5%.

## M2 — Seeing the spectrum

- Own radix-2 FFT (validated against a reference implementation in tests only).
- Hann window, magnitude spectrum, logarithmic band grouping into bass/mid/treble.
- Attack/release envelopes and auto-gain.
- Theory: `02-fft.md`, `03-bands-and-envelopes.md`.

**Done when:** terminal bars track the music smoothly, without jitter, at any volume.

## M3 — First pixels

- `winit` window + `wgpu` surface, frame pacing with `dt`, FPS cap.
- A single shader drawing the waveform and spectrum bars from uniforms.
- Theory: `04-gpu-pipeline.md` (surface, pipeline, uniforms, why WGSL).

**Done when:** bars render at a steady capped frame rate; GPU time is measured and shown in the debug HUD.

## M4 — The feedback loop

- Ping-pong `rgba16float` targets at half resolution.
- Warp pass (zoom, rotation, swirl) driven by `Signals`, decay, then sources drawn on top.
- Composite: cosine palette, tonemapping, dithering.
- Theory: `05-feedback.md` (why every frame is a warped copy of the last), `06-color.md`.

**Done when:** a single hand-written scene looks hypnotic and reacts to the music; no banding.

## M5 — Beats and bloom

- Onset/beat detection via spectral flux; `novelty` signal.
- Bloom chain (downsample/upsample).
- Global flash limiter.
- Theory: `07-beats.md`, `08-bloom-and-hdr.md`, `09-photosensitivity.md`.

## M6 — The generator

- Building blocks, composition rules, WGSL emission, background pipeline compilation.
- Crossfade transitions between scenes; intensity modes (`1`/`2`/`3`/`A`).
- Theory: `10-procedural-composition.md`, `11-intensity.md`.

**Done when:** an hour of music never shows an ugly scene and never stutters on a transition.

## M7 — Living on the desktop

- Borderless / fullscreen / mini modes, fading HUD with own glyph rendering, tray icon.
- Now-playing overlay (GSMTC).
- Pause on silence, hidden window or fullscreen game.
- Live wallpaper mode; screensaver build.

## M8 — Evolution and the web

- Scene mutation and saved favorites.
- `wasm32` build with WebGPU; live demo for juglar.dev; npm web component.
- Linux platform implementation.

## Always

- CI builds the core for `wasm32-unknown-unknown` to keep it OS-free.
- README tracks measured CPU/GPU cost against other visualizers.
