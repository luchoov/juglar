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

**Status: done, pending judgment by ear and by eye.** Beats and novelty are detected and move every scene; every frame passes the flash limiter; bloom adds glow. With the limiter and bloom, GPU 0.222 ms per frame at 960×540 and 0.73 ms maximized, CPU 7.3% of one core (GTX 1060). With real music the limiter held nothing in any of the six scenes then (since then, with another playlist, *Harmonograph* made it hold up to 0.2%).

- **5a, onsets and beats: done.** Spectral flux in three regions with adaptive thresholds fills `beat` (kicks and snares) and `novelty` (a change of character). The analysis view (`Tab`) shows both as meters and a two-second timeline. Analysis cost 0.075 ms per frame (from 0.050). Decisions D27–D28, note `07-beats.md`. Checked on synthetic signals by tests and on three real recordings by statistics, not yet by ear across genres.
- **More scenes (asked for after the milestone closed): done.** Six more, pushed toward the psychedelic: *Julia* and *Crystal*, fractals that the feedback loop computes one step per frame (a Julia set; a snowflake of mirrored copies); *Vortex* (rings turning against each other, five spiral arms); *Corona* (spectrum flames around an eclipse); *Melt* (paint running down, mirrored); *Lenses* (four lenses stirring a marbled coat). Twelve scenes in all. GPU 0.24–0.28 ms per frame at 960×540 and 0.80–1.0 ms maximized; CPU unchanged (7.8–8.0% of a core in that session, *Tide* included). With real music and with the synthetic drop at strong intensity, the limiter held nothing in any of the six, checked also from outside for flicker finer than its cells (D34). Seamless shared noise (D35). Decision D33, note `10-scenes.md`, explorable `05-warps-and-fractals.html` (five warps with arrows, the Julia loop beside the usual way of drawing the same set, Crystal's generations, bands of two palettes passing a point; checked in Chrome with no errors, every control responding, no horizontal scroll at 375 px, paused with reduced motion).
- **5e, explorable: done.** `explorables/04-beats-and-flashes.html`: a drum pattern you edit, spectral flux with each of its details switchable, novelty at a drop, the limiter on a strobe and on small test pictures, the red guard. Its JavaScript mirrors `onset.rs` and `Cell::step`. Checked in Chrome: on the simple pattern all 16 kicks and snares are found as beats with none extra; at most 4 transitions shown in any second over 160 strobe settings; the trembling line held at 0.0% with three smoothing stages; no horizontal scroll at 375 px.
- **5d, beats and novelty in the scenes: done.** Every scene moves with them, never brightens: in *Tide* a beat surges the outward flow, a change of section lengthens the trails and cools the colors for a few seconds; the sketches use them for orbit speed, zoom, current, wave travel, and the next harmonograph ratio. With the synthetic song's drop (kicks twice a second) at strong intensity and with real music, the limiter held nothing in any scene, after the ring's size got the same three-stage smoothing as its waveform. Judged only on one evening's music and the synthetic song.
- **5c, bloom: done.** Bright parts drawn at a quarter size, halved twice, the three blurs averaged into the final picture before the limiter. Three render passes, after a nine-pass first version cost the CPU 0.22 ms per frame. GPU 0.222 ms per frame at 960×540 (from 0.163), 0.73–0.74 ms maximized (from 0.529); CPU 7.33% of a core (from 6.77%). Decision D32, note `08-bloom-and-hdr.md`. Screenshots of *Orbits* show isolated black pixels that frames read back from the GPU don't have: something after Juglar (desktop composition or the capture); to be checked by eye.
- **Scene sketches (asked for during 5b): done.** Five new hand-made scenes beside *Tide*, deliberately different: *Orbits*, *Kaleido* (psychedelic), *Oil* (liquid light show), *Plasma*, *Harmonograph*, switched with the arrow keys. Looping palettes, a `warp` hook for folds and currents. None triggers the limiter with music. Decision D31, note `10-scenes.md`.
- **5b, flash limiter: done.** Every frame passes through a GPU limiter before the screen: 8×8-pixel cells (at 1080 rows), at most four transitions of 0.1 in any second per cell, holding back only the cells that would break it, plus a guard against saturated red. Checked by unit tests and by GPU tests with adversarial strobes read back from the screen. Its first finding made Tide's ring calmer (D30). GPU 0.163 ms per frame at 960×540 (from 0.096), 0.529 ms maximized; CPU 6.75–6.79% of a core (from 6.05%). Decisions D29–D30, note `09-photosensitivity.md`. Not checked by an independent flash analyzer.

Open: beats were never judged by ear, only by tests on synthetic signals and by statistics on three recordings; they need listening across genres. The isolated black pixels in screenshots (5c) must be looked for by eye on the monitor, since frames read back from the GPU don't have them. The analysis view (`Tab`) still has the limiter holding about 1% of the screen. The five scene sketches are untuned: drawn once and checked against the limiter, not refined. *Harmonograph* made the limiter hold up to 0.2% with a later playlist; retuned since (longer trails, a slower change of ratio), it held nothing with real music, a breakbeat and the synthetic song. None of the six newer scenes has been looked at on a screen, only in screenshots: how they move, and whether they are as striking as intended, is still to be judged by eye. The check for fine flicker (D34) still counts a few cells over the limit in *Kaleido*, *Harmonograph*, *Julia* and *Vortex*, most likely the capture's timing, not proved.

## M5.5 — Listening like a musician

The core innovation (see `docs/vision.md`). Built before the generator so it is designed around these signals from day one.

- Chromagram, key and chord estimate → harmony signals.
- Harmonic/percussive separation → separate `percussive` and `harmonic` energies.
- Tempo tracking with beat prediction; build-up and drop detection → `tension`, `beat_phase`.
- Beat-synchronous features and a self-similarity matrix → `section` and `recurrence` (this part sounds like that earlier part).
- Audio fingerprint → per-song seed.
- Explorables: one page per idea (chroma wheel, harmonic/percussive split, self-similarity matrix filling in live, tension curve).

**Done when:** on a pop song with a repeated chorus, the second chorus is recognized as a recurrence of the first within a few bars, and a drop is announced by rising tension before it lands.

**Status: started.** Tempo and beat phase are done (D43, `crates/dsp/src/tempo.rs`): an autocorrelation of the spectral flux and a ring of phase bins. Played through the speakers, drum patterns at 100 and 128 per minute were read exactly, with every detected beat within a tenth of a beat of the count; a breakbeat at 174 read 87, its half. Build-up tension and the drop are done in the director (D44): synthesized build-ups were caught on the drop's first kick, but no real music with build-ups has been tried, and a build-up that keeps its kick hides its drop. Note `11-tempo-and-the-director.md`, explorable `06-tempo-and-the-drop.html`. The harmonic/percussive split and the drums apart are done (D45–D47, `crates/dsp/src/percussion.rs`): `Signals` carries `kick`, `snare` and `cymbals` (short events that follow a double kick at 17 per second in the tests), the `percussive` and `harmonic` levels, and `relative`, six levels against their own last 4 s; the analysis view shows them. Against isolated drum tracks of three metal songs, kick, snare and cymbals score F 48, 51 and 63 (the onset detector's three regions, 3, 43 and 57); about half of each drum stays buried in a loud master, the kick and blast beats most. Analysis cost: 0.059 to 0.073 ms per frame over a minute of recorded music (0.35% to 0.44% of a core at 60 fps), 0.082 to 0.100 ms live; to be judged by ear. Note `12-drums.md`. Not started: chroma, key and chords; self-similarity, sections and recurrence; bars and downbeats; the per-song seed.

## M6 — The generator

- Building blocks, composition rules, WGSL emission, background pipeline compilation.
- Crossfade transitions between scenes; intensity modes (`1`/`2`/`3`/`A`).
- Theory: `10-procedural-composition.md`, `11-intensity.md`.

**Done when:** an hour of music never shows an ugly scene and never stutters on a transition.

**Status: the director is done; the generator is not started.** Ahead of the generator, the parts around it were built for the twelve hand-made scenes:

- **Transitions** (D38): the new scene's loop takes over the old picture, in four shapes: bloom, iris, seep and spiral.
- **Background pipeline compilation** (D39): the first frame comes in 1.65–1.9 s instead of 4.0–4.1.
- **Intensity modes**: natural (`A`, the default, D37) and fixed (`1`/`2`/`3`).
- **Automatic scene changes** (D40): at the music's turns, on the beat, to the scene whose character suits it, the transition chosen by the kind of moment.
- **Focus and relax modes** (D41).

All of this lives in the OS-free `crates/director`. Over 8 minutes of real music with automatic changes, the limiter held nothing and the GPU averaged 0.27 ms per frame. Still open: scene choice was tuned on metal only, and the wildest scenes never came up with real music. Nothing has been judged by eye on a screen. Theory: note 11 covers what was planned as `11-intensity.md`.

## M7 — Living on the desktop

- Borderless / fullscreen / mini modes, fading HUD with own glyph rendering, tray icon.
- Now-playing overlay (GSMTC).
- Pause on silence, hidden window or fullscreen game; lower frame rate while a game runs (M3 showed that CPU cost follows the frame rate, not the picture).
- Live wallpaper mode; screensaver build.

**Status: started.**

- **Done:** borderless fullscreen (D36: `F11`, `Alt`+`Enter`, double click, `--fullscreen`).
- **Done:** wallpaper mode on Windows (D42: `W`, `--wallpaper`), behind the desktop icons on both the 24H2 desktop and the earlier one. A tray icon has the controls, and nothing is drawn while a fullscreen program or a maximized window covers the desktop. Measured: 0 fps and 0.42% of a core while covered; 72 fps and 1.66% with the desktop shown (synthetic song).
- **Not tested:** several displays, and Windows before 24H2.
- **Not started:** the HUD, the now-playing overlay, mini mode, pausing on silence, the screensaver build.

## M8 — Evolution and the web

- Scene mutation and saved favorites.
- `wasm32` build with WebGPU; live demo for juglar.dev; npm web component.
- Linux platform implementation.

## Always

- CI builds the core for `wasm32-unknown-unknown` to keep it OS-free.
- Every technique worth seeing gets an explorable in `explorables/`, showing the real algorithm next to its code.
- README tracks measured CPU/GPU cost against other visualizers.
