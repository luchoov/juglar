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

### D27 — Beats are onsets found by spectral flux, in three regions, at a fixed rate

**Decided:** onsets are found by spectral flux: every 10.7 ms, how much each of about 18 bands got louder (in decibels) than two hops earlier, each band's rise divided by how much that band jumps around in steady noise (6 dB over the square root of its width in frequency steps). The flux is computed separately for low, middle and high frequencies, each with an adaptive threshold (the last second's mean plus three deviations, never below a floor derived from noise statistics). A beat is an onset in the low or middle region; high-region onsets (hi-hats) are not beats. Levels are measured relative to the loudest band, so playback volume doesn't matter. The detector has its own 21 ms window and fixed hop, independent of the frame rate.
**Why:** loudness alone can't find a kick inside a wall of guitars; rises in the spectrum can. Each choice fixed a failure seen in tests or on real music: averaged over the whole spectrum, kicks were diluted by everything else (found 7 of 20 under loud noise); narrow bass bands made steady noise fire several times a second; a fixed −60 dB floor found nothing at all on the development PC, where Windows' volume was low; counting hi-hats as beats would fire four times per beat.
**Considered:** energy thresholds on the bass level (misses snares, fires on swells); flux over raw energy instead of decibels (dominated by the loudest band); a tempo tracker now (it belongs with beat prediction in Milestone 5.5, and needs onsets first).
**Cost:** onsets are reported 10.7 ms late. Very soft kicks under loud broadband noise are partly missed (57 of 62 at the hardest level tested). Matching what a listener feels as the beat was checked only by statistics on three recordings of one genre, not by ear.

### D28 — Novelty compares a half-second and a four-second view of the spectrum

**Decided:** novelty is the difference, in decibels across all bands, between running averages of the band levels over 0.5 s and over 4 s, mapped so that 2.5 dB is 0 and 6 dB is 1. Bands more than 30 dB below the loudest are held at that edge; after real silence the averages start from the first sound.
**Why:** a drop or a new section changes the spectrum for seconds, not for one beat; two averages of different lengths disagree exactly then. The mapping comes from measurements: real songs within a section stayed under 1.5 dB nine tenths of the time (3.4 dB at most), a synthetic drop reached 4.6 dB within a second. Without the audible range, quiet bands swinging in near-silence kept novelty near 1 all the time.
**Considered:** novelty as the onset flux itself (high on every drum hit, not on changes of character); a self-similarity matrix (the real tool for structure, planned for Milestone 5.5 with beat-synchronous features).
**Cost:** novelty rises over a second or two and fades over several, so it marks "the music changed recently", not the exact moment. A section that changes only in harmony, not in loudness per band, goes unnoticed.

### D29 — A flash limiter on the GPU checks every frame before it is shown

**Decided:** every view draws into a candidate image in linear light. Two GPU passes in the same frame then decide and present: one per cell of an 8×8-pixel grid (at 1080 rows; proportional at other sizes) measures the cell's mean luminance and follows it over time (last extreme, direction, ages of the last four transitions of 0.1 or more); one per pixel shows the candidate, or, in a cell that would complete a third flash within a second, a mix of the last shown frame and the candidate that keeps the swing at 0.09. The same pass moves any color with more than 75% of its light in red toward gray of the same luminance, then dithers and encodes for the screen. The CPU reads the cell state back twice a second, without waiting, only to report the share held (`held` in the title).
**Why:** the research note (section 9) asks for the check before presentation, on the final composited image, counting transitions over a rolling second in real time, with no small-area exception. Reading the frame back to decide on the CPU (as GPU timing does) lags one or two frames: a flash would already be on screen. Mixing with the last frame, instead of dropping the frame, changes only the cells that break the rule and leaves the rest of the picture live. Cells of 8 pixels see a one-pixel line flashing (it swings its cell by 0.125); the first version, with 30-pixel cells, didn't. Measuring is cheap at any cell size, since every pixel is read once.
**Considered:** reading a reduced image back to the CPU and lowering exposure for the next frames (late by design, and global: one flashing corner would dim everything); one luminance for the whole screen (averages hide local flashes); per-pixel state (66 MB at full HD); a smooth, per-pixel share instead of one per cell (the cell's bookkeeping would no longer match what was shown, and a flash could slip through); 16-pixel cells (no fewer holds on Tide's trembling line, and blind to more).
**Cost:** GPU time 0.096 → 0.163 ms per frame at 960×540 and 0.318 → 0.529 ms maximized; whole-process CPU about 0.7 points higher (6.05% → 6.75–6.79% of a core at 75 fps), from two more passes per frame. Flicker much finer than a cell (a dot of a few pixels, a fine checkerboard swapping light and dark) and stripe patterns are not seen. Held cells show square specks of the previous frame. Resizing starts the count over. The limits have not been checked by an independent analyzer such as HardingFPA.

### D30 — Tide's sharp line follows the music slowly

**Decided:** the waveform Tide's ring draws is smoothed in three stages of 0.25 s each, and the ring's size, wave height and line thickness follow bass, energy and treble smoothed over 0.3 s. The analysis view still shows the raw waveform. Kicks still drive the flow.
**Why:** the limiter's first run held 1 to 1.5% of the screen with music, all along the ring's line: a bright line one or two pixels wide that trembled every frame and swept out and back with each kick made the pixels along it blink several times a second. One stage of smoothing halved the holds (an average updated each frame still takes a small random step every frame); three stages removed them: 0.0% over 27 s of the same music, 0.05% at most with the demo song.
**Considered:** bigger cells (didn't help, see D29); a dimmer or softer line (it would lose the crisp present moment the scene is built on); letting the limiter hold the line (it leaves specks, and a scene should not lean on its safety net).
**Cost:** the ring is calmer, a slowly breathing shape; its echoes in the tunnel are softer and less distinct than in Milestone 4. Whether to bring back texture in the trails (which are faint enough to tremble safely) is part of retuning Tide in step 5d.

### D31 — Several hand-made scenes before the generator, each trying a different idea

**Decided:** besides *Tide*, three sketches with their own shapes, motion and colors: *Orbits* (soft lights on orbits, long exposure), *Kaleido* (a spectrum rose in a six-way mirror fold, echoes cycling through a rainbow) and *Oil* (blobs of color dragged by a curl-noise current). Scenes share one interface (`Scene` in `scene/mod.rs`): `sources`, `overlay` and a new `warp` hook in WGSL, and a `Look` per frame. Palettes may loop (`palette_wrap`). The arrow keys switch scenes; each has its own feedback and composite pipelines, compiled at start.
**Why:** the developer asked to try scenes, patterns, colors and effects that are completely different, including a psychedelic side, and not to be limited by the calm look of the focus and relaxation research. Building them now, by hand, shows what the building blocks of Milestone 6 must be able to express: a mirror fold and a current both turned out to be one function changing where old paint comes from. The hard safety limits still hold, enforced by the limiter (D29); the softer design settings of the research (low saturation, slow motion) stay as targets for the focus and relaxation modes of Milestone 7.
**Considered:** waiting for the generator (it would be designed blind); separate post effects for the fold and the current (they belong inside the loop, where they compound frame after frame).
**Cost:** four pipelines more to compile at start. Switching scenes is a cut, not a crossfade. Every number in the sketches is untuned beyond one evening's look.

