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

*Replaced by D19: the Vulkan half of this decision was based on an incomplete measurement.*


**Decided:** when a machine has both an integrated and a dedicated GPU, Juglar uses the integrated one. On Windows it prefers Vulkan and falls back to DirectX 12. OpenGL is not included on any platform.
**Why:** Juglar's drawing is tiny (0.03 ms of GPU per frame), so the integrated GPU easily handles it and the dedicated one stays free for the game. The fixed cost of submitting and presenting a frame turned out to be most of Juglar's CPU work, and on the development GPU (GTX 1060) Vulkan's was a third to a half of DirectX 12's (about 0.25 ms against 0.75 ms per frame).
**Considered:** DirectX 12 only, as Windows' native API, which would make a smaller binary and use about 20 MB less memory. Measured, it was the more expensive one in CPU, which matters more next to a game. This may differ on other GPUs, so the measurement should be repeated on AMD and Intel hardware.

### D18 — Measure GPU time without waiting for it

**Decided:** the GPU writes its own clock at the start and end of each frame's drawing. Juglar reads the result back a frame or two later, whenever it's ready, and shows it in the window title.
**Why:** waiting for the GPU to finish would make the CPU idle and couple the two. Measuring on the GPU is the only way to time GPU work at all, because the CPU only records commands. It runs later.
**Cost:** frames drawn while a measurement is in flight aren't timed. A browser or GPU without timestamp support simply shows no GPU time.

### D19 — Prefer DirectX 12 on Windows (replaces D17)

**Decided:** on Windows, Juglar prefers DirectX 12 and falls back to Vulkan. The rest of D17 stands: the integrated GPU comes first, and OpenGL is not included.
**Why:** D17 chose Vulkan after timing the steps of each frame on Juglar's own thread, where Vulkan was two to three times cheaper. Measured again on a quiet machine, counting every thread of the process, NVIDIA's Vulkan driver turned out to do the rest of that work on a helper thread of its own. In total, Vulkan used about a fifth more CPU than DirectX 12 (7.7% against 6.3% of one core at 75 fps, six alternating pairs of runs, DirectX 12 lower in all six) and about 20 MB more memory. What matters next to a game is the total.
**Considered:** keeping Vulkan for its lighter main thread, which would only help if Juglar's own thread were the bottleneck. It isn't: the GPU work is 0.04 ms.
**Cost:** one GPU from one vendor was measured. AMD and Intel may differ, so the comparison has to be repeated there before this is final.

### D20 — A synthetic song drives the visuals in silence

**Decided:** `Signals` lives in a crate of its own, `signals`, and the renderer takes nothing else. Besides the levels, it carries the spectrum's 32 bands and the waveform's 256 points, already normalized. The same crate has a second producer, a synthetic song: a 32-bar loop at 120 beats per minute with an intro, a build, a drop and a breakdown. `JUGLAR_DEMO=1` uses it instead of capturing audio (`JUGLAR_DEMO_START=40` starts 40 seconds in, at the drop).
**Why:** effects need to be worked on, tested and photographed without music playing, and with the same "music" every time. The web demo, the screensaver preview and the npm component will have no system audio either. If the visuals only accepted audio, each of those would need a special case.
**Considered:** recorded audio files, which would sound real but need a decoder (a library, or a lot of code) and still go through the analysis. The synthetic song skips both, and its numbers are known exactly, which makes tests easy.
**Cost:** it is a caricature of music: perfectly regular, no melody. Good for development, not for judging how an effect reacts to real songs.

### D21 — The feedback loop stores ink, in half-size 16-bit float images

**Decided:** the feedback loop runs on two images half the window's width and height that take turns (one is read, the other written). Each pixel holds two 16-bit floating-point numbers: an amount of ink, and its palette position weighted by that amount. Colors are chosen only at the end.
**Why:** half size is a quarter of the pixels for the one step that reads and writes all of them, and trails are soft anyway. Floats let fading reach zero; with the screen's 256 levels, dim pixels round back to themselves and ghosts never leave. Storing ink and palette positions instead of colors lets paint change color as it ages and lets the whole picture's colors move with the music without repainting.
**Considered:** full-size images (four times the work for detail the trails don't have); 8-bit images (stuck ghosts, visible banding); storing colors, MilkDrop's way (no aging, and every recolor needs new paint).
**Cost:** half size blurs anything drawn into the loop, so sharp shapes are drawn again on top (D22). Two numbers per pixel cover one palette; a scene that needs two independent colors at once would need the spare channels.

### D22 — A scene paints twice: faintly into the trails, sharply on top

**Decided:** each shape is painted with a small fraction of its ink (5–11% in Tide) into the feedback images, and again in full, at full resolution, over the final picture.
**Why:** painted fully into the loop every frame, a shape smears into a solid band and loud music turns the screen into fog: the first version of Tide did exactly that. Many faint copies make trails with structure, and the crisp copy on top keeps the present readable.
**Considered:** lowering the ink of a single painting, which made the shape itself dim; fading faster, which shortened the trails to nothing.
**Cost:** each shape is computed twice per frame, once per resolution. At 0.1 ms of GPU per frame, that is affordable.

### D23 — Palettes are cosine formulas; Tide's runs from sand to blue

**Decided:** palettes are cosine palettes, color = a + b·cos(2π(c·t + d)) per channel. Tide's makes half a swing per channel, from sand (#dcc69a) to steel blue (#6f9bd1) through silver, and positions are clamped to that range. Present paint is warm in loud music; trails start cooler and keep cooling as they age.
**Why:** twelve numbers describe a whole smooth palette, cost a few GPU instructions, and can be shifted, mutated or derived from harmony later (Milestones 5.5 and 6). The first palette went through blue, teal and sand, and on screen its yellow-greens turned muddy when dim, which trails always are. The half swing avoids green entirely and reads as warm light leaving cool smoke.
**Considered:** gradients stored as images (more freedom, harder to generate and to animate); hue rotation (passes through every color, including garish ones).
**Cost:** a clamped half swing has two ends; a scene that wants a cycling palette sets c = 1 and doesn't clamp.

### D24 — Reduced motion freezes the picture, and follows the OS live

**Decided:** when Windows' "Show animations" setting is off, the scene receives silence and no passing time, so it shows a still ring. Juglar reads the setting once a second, so a change applies while it runs. `JUGLAR_REDUCED_MOTION=1` or `0` overrides it until there is a settings file (Milestone 7).
**Why:** the research note asks for zero animation and zero audio-driven change with reduced motion, honored before the first frame and during changes. Slower motion is not enough for people who need a static display.
**Considered:** slowing everything down (not what the setting asks for); a blank screen (allowed, but a still picture tells the user Juglar is running).
**Cost:** reduced motion shows no reaction to music at all. A still picture that changes once per section might suit some people; that would be a user option, not the default.

### D25 — The analysis view stays, behind Tab

**Decided:** Milestone 3's picture (waveform, spectrum, band levels) is kept as a second view. `Tab` switches between it and the scene. Keys 1, 2 and 3 set the intensity to gentle, medium and strong.
**Why:** when a scene reacts oddly, the first question is what Juglar heard. The analysis view answers it in the same window, and costs nothing while hidden. The intensity keys arrive before the generator (Milestone 6) because tuning a scene needs them.
**Cost:** keyboard shortcuts with no visible hint until the HUD exists (Milestone 7); the README lists them.

### D26 — Measure CPU in processor cycles, not clock ticks

**Decided:** Juglar's CPU cost is measured with the processor cycles Windows counts for each thread (`QueryProcessCycleTime`, `QueryThreadCycleTime`), converted to time at the processor's base clock, not with the usual CPU-time figure (`TotalProcessorTime`, what Task Manager shows). Every measurement uses a window that is really on screen, three runs per case, and reads Juglar's own once-a-second stats (`JUGLAR_STATS=1`) to confirm frames were being drawn the whole time.
**Why:** the usual figure is sampled, not timed: at each tick of the system clock (every 15.6 ms) Windows charges the whole tick to whichever thread is running. Juglar works in short bursts locked to the display, and on the 144 Hz monitor its bursts stayed in step with the ticks: one window drawing the same 72 frames per second measured anywhere between 0% and 12.6% of a core. The cycle count gave 5.6% in every run. This also explained the 0.2% readings taken with the monitors off, once blamed on hidden windows. Re-measured this way, D19 holds: DirectX 12 5.8%, Vulkan 6.4% at 75 fps, DirectX 12 lower in every pair, a smaller margin than the fifth measured before.
**Considered:** keeping tick-based numbers and averaging more runs (the bias doesn't average out when the phase holds for many seconds); external profilers (heavier, and not needed for a total).
**Cost:** cycles are converted with one fixed clock rate (3.6 GHz on the development PC, calibrated by spinning a thread for a second), which assumes a constant-rate cycle counter, true of every recent x86 processor. The figures for Milestones 1 and 2 (well under 1% of a core) were taken by ticks and have not been repeated.
