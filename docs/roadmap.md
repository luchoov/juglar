# Roadmap

Each milestone ends with something visible or measurable, a short theory note in `docs/theory/` explaining what was built and why, new entries in `docs/decisions.md` for any important choice, and, when the technique is worth seeing, an interactive page in `explorables/`. All of it must be understandable without reading code (decision D13). Read the note before moving on.

The north star is in `docs/vision.md`: a visualizer that listens like a musician.

## M1 — Hearing the system

- `platform::windows`: WASAPI loopback capture on the default render device, event-driven, on its own thread.
- Lock-free SPSC ring buffer into the main thread.
- Debug output: RMS level and peak in the terminal.
- Theory: `01-audio-capture.md` (sample rate, frames, loopback, why event-driven, why lock-free).

**Done when:** playing music prints a moving level; silence prints ~0; CPU stays under 0.5%.

**Status: done.** 48 kHz stereo, no dropped samples, CPU below what Windows can measure, 12 MB RAM.

## M2 — Seeing the spectrum

- Own radix-2 FFT (validated against a reference implementation in tests only).
- Hann window, magnitude spectrum, logarithmic band grouping into bass/mid/treble.
- Attack/release envelopes and auto-gain.
- Theory: `02-fft.md`, `03-bands-and-envelopes.md`.

**Done when:** terminal bars track the music smoothly, without jitter, at any volume.

**Status: done.** 0.47% of one core, 12 MB RAM.

## M3 — First pixels

- `winit` window + `wgpu` surface, frame pacing with `dt`, FPS cap.
- A single shader drawing the waveform and spectrum bars from uniforms.
- Theory: `04-gpu-pipeline.md` (surface, pipeline, uniforms, why WGSL).

**Done when:** bars render at a steady capped frame rate; GPU time is measured and shown in the debug HUD.

**Status: done.** The HUD comes in M7, so stats go to the window title for now. Triggered (stable) waveform, 32 bars and 4 meters from one fragment shader. Own frame pacer snapped to the display refresh (72 fps at 144 Hz, intervals 13.1–14.4 ms). GPU time 0.03 ms per frame on a GTX 1060. DirectX 12 preferred on Windows after measuring the whole process (decisions D14–D19; D19 replaced D17). The core builds for `wasm32-unknown-unknown`.

Re-measured with both monitors on, counting processor cycles (decision D26): 5.8% of one core at 75 fps and 5.6% at 72 fps (under 1% of the CPU), proportional to the frame rate, almost all of it on the main thread. Vulkan, counted the same way, costs 6.4%, so D19 stands. Earlier spreads (0.2% to 15% for the same work) came from Windows' tick-based CPU accounting, which is unreliable for work locked to the display, and from another program loading the machine. A minimized window was still drawn at full cost; now it costs 0.3%. The frame rate now follows the window to another display.

Open: memory went from 12 MB to 100–115 MB, mostly GPU driver allocations. Repeat the Vulkan/DirectX 12 comparison on AMD and Intel GPUs (`JUGLAR_GPU_API` forces one).

## M4 — The feedback loop

- Ping-pong `rgba16float` targets at half resolution.
- Warp pass (zoom, rotation, swirl) driven by `Signals`, decay, then sources drawn on top.
- Composite: cosine palette, tonemapping, dithering.
- Theory: `05-feedback.md` (why every frame is a warped copy of the last), `06-color.md`.

**Done when:** a single hand-written scene looks hypnotic and reacts to the music; no banding.

**Status: done, pending a human verdict on "hypnotic".** Two half-size 16-bit float images take turns; each frame warps (zoom, rotation, swirl, wandering center) and fades the last picture by per-second rates, and the scene paints faintly into it and sharply on top. The composite applies a cosine palette, tone mapping, a contrast curve, dithering and a vignette. First scene: *Tide*, a ring drawn by the waveform whose trails flow outward. `Tab` shows the analysis view, keys 1–3 set the intensity, Windows' reduced-motion setting freezes the picture. GPU 0.095 ms per frame at 960×540 and 0.32 ms at 1920×1009 (GTX 1060); CPU unchanged from M3 (5.6–5.8% of one core, measured with the monitors on). Decisions D20–D25.

Open: the stripe limit holds by design only (the flash limit is measured since M5b). Audio `beat` and `novelty` came in M5a. The scene was tuned on one evening of music; it needs judging on more genres.

## M5 — Beats and bloom

- Onset/beat detection via spectral flux; `novelty` signal.
- Bloom chain (downsample/upsample).
- Global flash limiter.
- Theory: `07-beats.md`, `08-bloom-and-hdr.md`, `09-photosensitivity.md`.

**Status: in progress.**

- **5a, onsets and beats: done.** Spectral flux in three regions with adaptive thresholds fills `beat` (kicks and snares) and `novelty` (a change of character). The analysis view (`Tab`) shows both as meters and a two-second timeline. Analysis cost 0.075 ms per frame (from 0.050). Decisions D27–D28, note `07-beats.md`. Checked on synthetic signals by tests and on three real recordings by statistics, not yet by ear across genres.
- **5c, bloom: done.** Bright parts drawn at a quarter size, halved twice, the three blurs averaged into the final picture before the limiter. Three render passes, after a nine-pass first version cost the CPU 0.22 ms per frame. GPU 0.222 ms per frame at 960×540 (from 0.163), 0.73–0.74 ms maximized (from 0.529); CPU 7.33% of a core (from 6.77%). Decision D32, note `08-bloom-and-hdr.md`. Screenshots of *Orbits* show isolated black pixels that frames read back from the GPU don't have: something after Juglar (desktop composition or the capture); to be checked by eye.
- **Scene sketches (asked for during 5b): done.** Five new hand-made scenes beside *Tide*, deliberately different: *Orbits*, *Kaleido* (psychedelic), *Oil* (liquid light show), *Plasma*, *Harmonograph*, switched with the arrow keys. Looping palettes, a `warp` hook for folds and currents. None triggers the limiter with music. Decision D31, note `10-scenes.md`.
- **5b, flash limiter: done.** Every frame passes through a GPU limiter before the screen: 8×8-pixel cells (at 1080 rows), at most four transitions of 0.1 in any second per cell, holding back only the cells that would break it, plus a guard against saturated red. Checked by unit tests and by GPU tests with adversarial strobes read back from the screen. Its first finding made Tide's ring calmer (D30). GPU 0.163 ms per frame at 960×540 (from 0.096), 0.529 ms maximized; CPU 6.75–6.79% of a core (from 6.05%). Decisions D29–D30, note `09-photosensitivity.md`. Not checked by an independent flash analyzer.

## M5.5 — Listening like a musician

The core innovation (see `docs/vision.md`). Built before the generator so it is designed around these signals from day one.

- Chromagram, key and chord estimate → harmony signals.
- Harmonic/percussive separation → separate `percussive` and `harmonic` energies.
- Tempo tracking with beat prediction; build-up and drop detection → `tension`, `beat_phase`.
- Beat-synchronous features and a self-similarity matrix → `section` and `recurrence` (this part sounds like that earlier part).
- Audio fingerprint → per-song seed.
- Explorables: one page per idea (chroma wheel, harmonic/percussive split, self-similarity matrix filling in live, tension curve).

**Done when:** on a pop song with a repeated chorus, the second chorus is recognized as a recurrence of the first within a few bars, and a drop is announced by rising tension before it lands.

## M6 — The generator

- Building blocks, composition rules, WGSL emission, background pipeline compilation.
- Crossfade transitions between scenes; intensity modes (`1`/`2`/`3`/`A`).
- Theory: `10-procedural-composition.md`, `11-intensity.md`.

**Done when:** an hour of music never shows an ugly scene and never stutters on a transition.

## M7 — Living on the desktop

- Borderless / fullscreen / mini modes, fading HUD with own glyph rendering, tray icon.
- Now-playing overlay (GSMTC).
- Pause on silence, hidden window or fullscreen game; lower frame rate while a game runs (M3 showed that CPU cost follows the frame rate, not the picture).
- Live wallpaper mode; screensaver build.

## M8 — Evolution and the web

- Scene mutation and saved favorites.
- `wasm32` build with WebGPU; live demo for juglar.dev; npm web component.
- Linux platform implementation.

## Always

- CI builds the core for `wasm32-unknown-unknown` to keep it OS-free.
- Every technique worth seeing gets an explorable in `explorables/`, showing the real algorithm next to its code.
- README tracks measured CPU/GPU cost against other visualizers.
