# Decisions

Every important decision in Juglar, in plain language: what was decided, why, what else was considered, and what it costs. No code needed to follow it. Newest at the bottom.

---

### D1 — Native app in Rust, not a web/Electron app

**Decided:** Juglar is a native program written in Rust.
**Why:** the whole point is to run next to a game on a modest PC. Electron apps (Spotify, Discord) ship an entire browser and use hundreds of MB of RAM. A native Rust program is a few MB, starts instantly, and has direct access to the GPU.
**Considered:** a web page (easy to share, but can't capture system audio and runs inside a browser's overhead); C++ (as fast, but easier to get crashes and memory bugs).
**Cost:** Rust takes longer to write. Agents absorb most of that.

### D2 — Everything artistic and musical is written from scratch

**Decided:** audio capture, the FFT, the analysis, every effect, shader and preset are Juglar's own. Libraries only for plumbing: opening a window (`winit`), talking to the GPU (`wgpu`), and calling Windows (`windows`).
**Why:** the project exists to show and to learn how these things work. "I integrated a visualization library" proves little; "I built the analysis and every effect" proves a lot.
**Considered:** reusing projectM (MilkDrop's open-source engine) or an FFT library.
**Cost:** more work. Known techniques (feedback, palettes, bloom) are still fair game: implementing a public technique yourself *is* doing it from scratch.

### D3 — The art never depends on the operating system

**Decided:** the code that decides what you see and hear (analysis, generator, shaders) never calls Windows, Linux or macOS directly. Each OS only provides a thin layer: capturing audio, reading "now playing", the tray icon, wallpaper mode.
**Why:** it makes Linux, macOS and a web version possible later without rewriting the core, and it is checked automatically by also building the core for the web.
**Cost:** a little more structure up front.

### D4 — Windows 11 first

**Decided:** only Windows is implemented for now; Linux comes when the developer's own setup moves to Linux; macOS later.
**Why:** it's the machine available for testing, and the one where Juglar will mostly be used (next to games).

### D5 — The generator consumes "signals", never audio

**Decided:** effects are driven by a small set of numbers (bass, mid, treble, energy, beats, and later key, section, tension), not by raw sound.
**Why:** the same generator can then be driven by music, by time, by page scrolling or by the mouse. That is what makes the screensaver, the website demo and a web-background library possible from the same core.

### D6 — Capture with WASAPI loopback on its own thread

**Decided:** audio comes from Windows' "loopback" mode, which records exactly what the speakers play, on a dedicated thread that sleeps until the audio engine wakes it.
**Why:** it works with any app without plugins or virtual cables, and sleeping between packets costs almost nothing (below what Windows can measure).
**Considered:** a virtual audio cable (asks the user to install a driver); polling for data in a loop (wastes CPU).

### D7 — A lock-free buffer between audio and graphics

**Decided:** the audio thread hands samples to the graphics side through a ring buffer that never makes either side wait.
**Why:** if the audio thread ever had to wait for the graphics side, the sound capture would glitch. The golden rule of audio code: the audio side never waits and never asks for memory.
**Cost:** if the graphics side stopped reading for a full second, the newest samples would be dropped (and counted) rather than block anything.

### D8 — FFT of 2048 samples

**Decided:** analyze 2048 samples (43 ms) at a time.
**Why:** it's the usual balance: short enough to feel instant, fine enough to tell a kick drum from a bass line.
**Cost:** the lowest frequencies are coarse (one step every 23 Hz), so the lowest bands of the 32-band display share data. Can be improved later with a bigger window just for the bass.

### D9 — Automatic gain, with two safety limits

**Decided:** every band is scaled to how loud it has been recently, so quiet and loud songs both use the full range. It never boosts silence (below −60 dB stays at zero), and a band can't be boosted when it's far quieter (more than 18 dB) than the music overall.
**Why:** without the first limit, background hiss would fill the screen; without the second, a nearly empty band would jump as if it were loud. Tests caught the second case.

### D10 — Smoothing based on real time, not frames

**Decided:** values rise fast and fall slowly (attack/release), computed from the real time between frames.
**Why:** fast rise, slow fall is what makes visuals hit on the beat and breathe in between. Using real time makes it look the same at 30, 60 or 144 frames per second.

### D11 — The innovation: listening like a musician

**Decided:** Juglar's distinguishing feature is musical understanding: song structure (the chorus comes back, so does its image), a visual identity per song, harmony as color, rhythm and melody driving different things, and anticipation of drops. Details in `vision.md`.
**Why:** reacting to loudness frame by frame is what every visualizer already does.

### D12 — Honest about focus and relaxation

**Decided:** focus and relaxation modes are style options, not health claims. Flash limits (at most 2 per second, no saturated red flashes) apply in every mode.
**Why:** the research review (`research/focus-and-relaxation.md`) found no evidence that synchronized visuals improve focus or relaxation over music alone, and real risks for photosensitive viewers.

### D13 — Understandable without reading code

**Decided:** every milestone is explained in plain language: a theory note (`theory/`), entries here, and when worth it an interactive page (`explorables/`). Code excerpts are always optional extras, never required to follow the explanation.
**Why:** Juglar is built mostly by agents, and its author should be able to understand and defend every idea and decision without reading Rust.

### D14 — A stable waveform, like an oscilloscope

**Decided:** the waveform on screen always starts where the bass crosses zero going up, as an oscilloscope's *trigger* does. To find that point, the sound is passed through a filter that keeps only the low end, run forward and then backward so the filter's delay cancels out.
**Why:** drawing "the last 21 ms" each frame cuts the wave at a random point of its cycle, so it jitters sideways. With the trigger, repeating sounds land in the same place every frame and look still. Triggering on the bass instead of the full signal keeps hi-hats, which cross zero constantly, from making it jump.
**Cost:** the newest 21 ms are sometimes skipped to find a crossing. Nobody can see that delay.

### D15 — One shader draws the whole frame

**Decided:** for now, a single fragment shader decides the color of every pixel from about 1 KB of numbers (bands, spectrum, waveform) uploaded each frame. The GPU draws one triangle that covers the window.
**Why:** it's the smallest possible pipeline: one pass, no geometry, nothing on the CPU beyond copying numbers. Later effects (Milestone 4 onward) add passes in front of it rather than replacing it.
**Considered:** building the bars and the line out of triangles on the CPU, which means more CPU work and more data for the same picture.

### D16 — Juglar paces its own frames

**Decided:** a timer wakes Juglar for each frame, at the display's refresh rate divided by the whole number that lands closest to 60 fps (72 fps on a 144 Hz display). Vsync is still requested, but never relied on.
**Why:** frames out of step with the display stutter, so the rate snaps to it. Vsync alone can't set the pace: on the development PC the driver ignored it and the loop ran at 1,400 fps, using 80% of a CPU core. Drivers let users force vsync off, and gamers often do.
**Cost:** on a 144 Hz display Juglar draws 72 fps rather than exactly 60. The `JUGLAR_FPS` environment variable sets another target until there is a settings file.

### D17 — Prefer the integrated GPU, and Vulkan on Windows

**Decided:** when a machine has both an integrated and a dedicated GPU, Juglar uses the integrated one. On Windows it prefers Vulkan and falls back to DirectX 12. OpenGL is not included on any platform.
**Why:** Juglar's drawing is tiny (0.03 ms of GPU per frame), so the integrated GPU easily handles it and the dedicated one stays free for the game. The fixed cost of submitting and presenting a frame turned out to be most of Juglar's CPU work, and on the development GPU (GTX 1060) Vulkan's was a third to a half of DirectX 12's (about 0.25 ms against 0.75 ms per frame).
**Considered:** DirectX 12 only, as Windows' native API, which would make a smaller binary and use about 20 MB less memory. Measured, it was the more expensive one in CPU, which matters more next to a game. This may differ on other GPUs, so the measurement should be repeated on AMD and Intel hardware.

### D18 — Measure GPU time without waiting for it

**Decided:** the GPU writes its own clock at the start and end of each frame's drawing. Juglar reads the result back a frame or two later, whenever it's ready, and shows it in the window title.
**Why:** waiting for the GPU to finish would make the CPU idle and couple the two. Measuring on the GPU is the only way to time GPU work at all, because the CPU only records commands. It runs later.
**Cost:** frames drawn while a measurement is in flight aren't timed. A browser or GPU without timestamp support simply shows no GPU time.
