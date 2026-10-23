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

**Decided:** the waveform Tide's ring draws is smoothed in three stages of 0.25 s each, and the ring's size, wave height and line thickness follow bass, energy and treble smoothed in three stages of 0.2 s (first one stage of 0.3 s; in step 5d the synthetic song's drop, kicks twice a second, still made the limiter hold 0.08% of the screen with it). The analysis view still shows the raw waveform. Kicks still drive the flow.
**Why:** the limiter's first run held 1 to 1.5% of the screen with music, all along the ring's line: a bright line one or two pixels wide that trembled every frame and swept out and back with each kick made the pixels along it blink several times a second. One stage of smoothing halved the holds (an average updated each frame still takes a small random step every frame); three stages removed them: 0.0% over 27 s of the same music, 0.05% at most with the demo song.
**Considered:** bigger cells (didn't help, see D29); a dimmer or softer line (it would lose the crisp present moment the scene is built on); letting the limiter hold the line (it leaves specks, and a scene should not lean on its safety net).
**Cost:** the ring is calmer, a slowly breathing shape; its echoes in the tunnel are softer and less distinct than in Milestone 4. Whether to bring back texture in the trails (which are faint enough to tremble safely) is part of retuning Tide in step 5d.

### D31 — Several hand-made scenes before the generator, each trying a different idea

**Decided:** besides *Tide*, five sketches with their own shapes, motion and colors: *Orbits* (soft lights on orbits, long exposure), *Kaleido* (a spectrum rose in a six-way mirror fold, echoes cycling through a rainbow), *Oil* (blobs of color dragged by a curl-noise current), *Plasma* (summed waves of color everywhere) and *Harmonograph* (a pendulum drawing, its figure computed on the CPU and passed in the waveform's slots). Scenes share one interface (`Scene` in `scene/mod.rs`): `sources`, `overlay` and a new `warp` hook in WGSL, and a `Look` per frame. Palettes may loop (`palette_wrap`). The arrow keys switch scenes; each has its own feedback and composite pipelines, compiled at start.
**Why:** the developer asked to try scenes, patterns, colors and effects that are completely different, including a psychedelic side, and not to be limited by the calm look of the focus and relaxation research. Building them now, by hand, shows what the building blocks of Milestone 6 must be able to express: a mirror fold and a current both turned out to be one function changing where old paint comes from. The hard safety limits still hold, enforced by the limiter (D29); the softer design settings of the research (low saturation, slow motion) stay as targets for the focus and relaxation modes of Milestone 7.
**Considered:** waiting for the generator (it would be designed blind); separate post effects for the fold and the current (they belong inside the loop, where they compound frame after frame).
**Cost:** four pipelines more to compile at start. Switching scenes is a cut, not a crossfade. Every number in the sketches is untuned beyond one evening's look.

### D32 — Bloom in three passes, from a quarter of the window

**Decided:** the scene's bright parts (luminance ramping in from 0.2 to 0.8) are drawn again at a quarter of the window's size, halved twice, and the final composite pass reads the three levels, each enlarged with a four-sample tent, averages them, and adds them with a "screen" blend, as much as the scene's `bloom` asks. Before the flash limiter. Skipped when a scene asks for none.
**Why:** a wide blur is cheapest as a chain of shrinking images. The textbook chain (five levels from half size, down and back up, nine passes) cost the GPU only 0.07 ms but the CPU 0.22 ms per frame (6.8% → 8.4% of a core): each render pass costs about 25 µs to submit, whatever its size. Three passes cost 7.33% of a core, for a halo slightly less wide.
**Considered:** the full chain (above); a compute shader (dispatches cost the same to submit); blurring in the composite pass alone from the feedback image (dozens of samples per full-size pixel).
**Cost:** GPU time 0.163 → 0.222 ms at 960×540, 0.529 → 0.73–0.74 ms maximized; CPU about +0.55 points of a core. The glow is made after tone mapping, not from high-dynamic-range values.

### D33 — Six more scenes, pushed toward the psychedelic, with fractals computed by the loop

**Decided:** six more hand-made scenes after Milestone 5: *Julia* (the warp is one step of z → z² + c, so the loop iterates a Julia set one step per frame and paint drift turns escape steps into bands), *Crystal* (the warp is a turn, a five-way mirror fold, a shift and an enlargement, so the loop runs an iterated function system and grows a snowflake of copies), *Vortex* (rings turning against each other and a traveling ripple, five lights drawing spiral arms), *Corona* (spectrum flames around a dark disc, stirred by curl noise), *Melt* (paint pulled down at column-varying speeds, mirrored) and *Lenses* (four wandering lenses bending a marbled coat every frame). In *Julia* and *Crystal* the fade and the palette step are per frame, not per second, since a frame is one step of the fractal. Their palettes were chosen for a narrow range of brightness (linear luminance within about 0.11 to 0.46).
**Why:** the developer asked for scenes much more striking and psychedelic than the first sketches, flashy in motion and color but never strobing. Fractals usually cost hundreds of steps per pixel per frame; inside a feedback loop the repetition is free, so they cost what a tunnel costs (0.24–0.25 ms per frame at 960×540). A palette with a narrow range of brightness makes bands of very different hue swing a point's brightness about half as much as a usual one, and the flash rule measures brightness.
**Considered:** drawing fractals with a fixed number of steps per pixel (dozens to hundreds of samples per pixel); per-second fades in the fractals (the look would change with the frame rate, since the structure is made of steps); drawing the ring inside *Julia*'s set (its paint circulated through the interior and shimmered six to twenty times more); separate drops in *Melt* (a comb of regular stripes); stronger shear in *Vortex* (the limiter held up to 0.3% with music).
**Cost:** each scene is two more pipelines to compile at start (twelve scenes in all). *Julia* settles more slowly at a lower frame rate. Every number is tuned from screenshots and measurements, not yet by eye on a screen. Fractals and folds make small, fast-moving detail by nature; the scenes stay calm only because their parameters move slowly, and a future change that sped them up would need the checks again (note 10).

### D34 — Scenes are accepted with a check for flicker finer than a cell

**Decided:** besides the limiter's own count, every scene is checked from outside before it is kept: the window is captured as fast as possible (about 46 frames per second) for 20 seconds with real music and with the synthetic song's drop at strong intensity, and transitions are counted with the limiter's rule in every 2×2-pixel block and every limiter cell, with a picture of where. The scripts live outside the repository, with the other measuring tools; the method and results are in note 10.
**Why:** the limiter can't see flicker finer than its cells (D29), and the new scenes (fractals, folds, fine bands) are exactly where it would hide. The check also showed that the limiter's count, read twice a second at one decimal, can read 0.0% while it is holding cells.
**Considered:** trusting the limiter's count alone (blind below a cell, coarse in time); a per-pixel limiter (rejected in D29 for its memory); reading frames back from the GPU instead of the screen (exact, but it would need code in Juglar only for measuring).
**Cost:** the check counts moving thin lines as well as blinking ones, so it gives comparisons, not verdicts. It misses about two frames in five and its clock is a frame or so off the moment frames were drawn, so a cell held exactly at the limit can look over it (it allows 0.05 s for that).

### D35 — One shared noise, hashed on integers

**Decided:** value noise lives in `common.wgsl` (`noise`), with the random value of each grid point from a PCG hash of its integer coordinates. *Oil*, *Corona*, *Melt* and *Lenses* use it.
**Why:** the usual fract(sin(x) × 43758.5) hash left a straight seam across a picture: the GPU computed some grid points slightly differently when reached from the cell on either side, and the huge multiplier turned the rounding difference into a different value. Integer arithmetic gives both cells the same value exactly.
**Considered:** patching the sin hash (any rounding difference in its input still changes its output); a noise texture (one more resource to create and bind for every scene).
**Cost:** a little more GPU time: *Oil* 0.247 → 0.264 ms per frame at 960×540. *Oil*'s current is a different random pattern than before.

### D36 — Borderless fullscreen

**Decided:** F11, Alt+Enter, a double click or `--fullscreen` fill the display the window is on with a borderless window, not an exclusive video mode. Escape leaves fullscreen first and closes only from a window. The mouse pointer hides after two seconds without moving.
**Why:** borderless switches instantly, keeps the display's refresh rate (which the frame pacer follows) and lets other windows and notifications appear on top. Escape closing a fullscreen picture at once would surprise anyone who pressed it to get out.
**Considered:** exclusive fullscreen (a mode switch with a black flash, and nothing gained for a program this light).
**Cost:** none measured beyond the larger picture: 0.80 ms of GPU per frame at 1920×1080 on the 75 Hz display, as when maximized.

### D37 — Natural intensity, read from the music, by default

**Decided:** a new crate, `director`, decides what to show from slow summaries of the signals (`Feel`: beats per second over about three seconds, treble over three, loudness compared with the last half minute, and whether the music is rising). By default it sets the intensity: half from the pulse, three tenths from the brightness, a fifth from that swell, mapped to 0.15–0.95, rising in about 2 s and falling in about 6 s. `A` returns to it; keys 1–3 and `JUGLAR_INTENSITY` fix the intensity as before.
**Why:** a fixed intensity is wrong for most of a song: an intro is not a drop. Intensity only scales how fast and far scenes move, never brightness, so following the music changes motion, and slowly. Loudness was the obvious input and turned out the worst: the analysis levels every song to its own peaks, so over two and a half minutes of real music it stayed between 0.87 and 0.98 while the pulse ran from 0.01 to 1 and the brightness from 0.01 to 0.8. The first formula, built on loudness, sat at 0.91 for the whole song. Comparing loudness with the last half minute keeps what it does say: this part is louder or quieter than the one before.
**Considered:** loudness as the main input (above); the decision inside `render` or the app (the generator and scene changes will need the same summaries, and they must build for the web).
**Cost:** a busy song with hi-hats sits at about 0.6–0.75 for most of its length, not at the strong preset; a slow, dark song stays near 0.35. Tuned on the synthetic song and five minutes of one playlist. Natural intensity was judged by numbers only, not by watching.

### D38 — Transitions: two loops, the new one taking over the old picture

**Decided:** during a transition both scenes are drawn, each with its own feedback images, glow and frame numbers (two "layers"). The new scene's loop starts from a copy of the old one's latest picture, so its warp bends the old shapes before its own paint replaces them. The composite step draws the new scene whole and the old one over it, with an alpha from the transition's shape: each point of the window gets a fixed number saying when it turns over (from the middle out, *bloom*; from the edges in, *iris*; through soft noise patches, *seep*; along two spiral arms, *spiral*), and as the transition's progress passes it the point turns from old to new over a soft band. Progress moves with the clock and, if asked, by a share per beat. The arrow keys use a 1.5 s bloom; with reduced motion, a 3 s seep.
**Why:** a cut changes the whole screen in one frame. A plain crossfade of two pictures is the usual answer and looks like a slideshow; feedback offers something only it can do: hand the old picture to the new motion. The maps are fixed while a transition lasts, so each point turns exactly once, never back and forth. With the probe for fine flicker (D34) on four styles slowed to 4 s and on two colorful pairs, the limiter held nothing; the probe counted at most 0.025% of cells over the rule, in the middle of *Crystal* folding the copied *Julia* picture.
**Considered:** a crossfade (above); compiling both scenes into one shader and mixing their warps (every pair of scenes would be a pipeline, 132 of them, and helper names in scene shaders could clash); a glowing seam along the edge (a thin moving bright line, what the flash rule is most sensitive to).
**Cost:** while a transition lasts, everything but the limiter runs twice: GPU about 0.4 ms per frame at 960×540 instead of 0.23 (averages of 0.30–0.33 ms over runs half in transition). One more pipeline per scene: 3.0–3.6 s to 4.0–4.1 s before the first frame, until D39. A transition asked for while one is under way waits for it to end (only the last one asked for is kept): cutting it short changed the whole screen at once, and seven presses of an arrow in just over a second made the limiter hold 0.1% of the screen. Two sets of feedback and glow images: about 5 MB more GPU memory at 1920×1080.

### D39 — Scene pipelines compile in the background

**Decided:** before the first frame only the first scene's pipelines are compiled; a thread ("juglar shaders") compiles the rest in order and hands them over as they are ready. A scene chosen before its turn is compiled on the spot. Each scene's two shader modules are shared by its pipelines (one for feedback, one for bloom source, composite and fade), and the pipeline layouts by all scenes. On the web, with no threads to spare, scenes compile when first chosen.
**Why:** each pipeline costs about 75 ms to compile with DirectX 12 on the development PC, and twelve scenes have 48. The window stayed empty for 4.0–4.1 s at start; now 1.65–1.9 s (five starts each). Frame intervals stayed at 13.4–14.3 ms (72 fps) while the thread worked.
**Considered:** compiling everything first (above); compiling each scene only when chosen (the first transition to each scene would stall a frame for a third of a second); a pipeline cache on disk (wgpu supports one only on Vulkan).
**Cost:** a second thread for the first seconds after start. Choosing a scene before the thread reaches it stalls one frame while it compiles.


### D40 — Automatic scene changes at the music's turns

**Decided:** the director changes scene by itself. *When:* at a moment — the analysis' novelty jumping (a new song, a drop) or the feel shifting (pulse, brightness and swell compared with six seconds earlier, by more than 0.4) — once the scene has shown 30 s; without a moment, after 100 s anyway. A moment that came while the scene was younger still counts for 4 s. The change waits up to 0.75 s to land on a beat. *What kind:* for half a second after a moment the director listens; loudness gained or lost by more than 0.12 makes it a rise or a fall, otherwise a turn. A rise opens the new scene from the middle (bloom, 2.5 s, pushed by beats), a fall closes the old one into it (iris, 4 s), a turn or a long wait sweeps it in (spiral, 3 s); quiet music always seeps (6 s). *To what:* each scene has a character (how lively it looks, how much it moves with beats, whether it moves the whole picture); the director picks the closest to the music's liveliness and pulse, avoiding the scene showing and the last four shown. `Space` keeps the scene; the arrows still choose, and the chosen scene gets its full time.
**Why:** a change on a timer, to a random scene, ignores the music; a VJ waits for a corner and changes on the beat to something that suits what comes next. Novelty alone was not enough: over five minutes of dense metal it jumped only when a song started, while within a song the feel shifted clearly twice. At the first frame of a drop only novelty has moved, so the kind of moment can't be read there; half a second later the loudness has. Over 8 minutes of real music (black and death metal), 9 changes came 30–100 s apart, the limiter held nothing in 461 s, GPU 0.27 ms mean and 0.59 ms at most — but every scene chosen was a calm one: the music's liveliness stayed between 0.3 and 0.65 while scenes spread from 0.2 to 0.9. Liveliness is now stretched before matching (0.2 and below asks for the calmest scene, 0.7 and above for the wildest). Five and a half minutes more, of slower, epic metal: 3 changes (one at a turn, two after 100 s without one), to *oil*, *plasma* and *harmonograph*. The intensity stayed at 0.45–0.57 and only 0.2–1.1 beats a second were found, so scenes that move with beats lost. The wildest scenes (*kaleido*, *vortex*, *corona*) never came up with real music, only with the synthetic drop. The limiter held 0.1% for one second there, in *Harmonograph*, which did so before (M5's open items).
**Considered:** random scenes on a timer (above); changing at every moment (on metal, the shift crosses 0.4 in about 5% of seconds, in clusters); a "fullness" measure of how many bands sound at once, to tell metal from a quiet song (0.23–0.30 on metal against 0.10–0.40 on the synthetic song: it told nothing).
**Cost:** a few dozen operations per frame on the CPU. Characters were set by hand from how each scene moves, and the matching was tuned on one genre: other music, and whether the choices look right, are still to be judged.

### D41 — Focus and relax: calm modes within the research's limits

**Decided:** `M` goes through three modes; `JUGLAR_MODE` picks one at start. *Lively* is everything above. *Focus* and *relax* apply the limits in `docs/research/focus-and-relaxation.md`: every scene's motion multiplied by 0.15 (focus) or 0.3 (relax); no zoom, turn or swirl of the whole picture, and only scenes that don't move it; colors kept at 30% or 35% of their difference from gray; luminance squeezed between black and 0.10 or 0.15 (linear light); intensity fixed at 0 or 0.1; scenes held 120–300 s or 60–240 s, changed by a 15 s or 20 s seep. The scenes hear the music faintly: levels follow it over seconds (2 s up and 5 s down in focus, 1 s and 4 s in relax), swing only a quarter or 0.4 of their range, and beats and novelty never reach them. The director still hears everything, to choose and time changes. The look fades in and out over 3 s.
**Why:** the research found no evidence that visuals improve focus or calm, only what disturbs: fast or large motion, high contrast, saturated color, flashes, sudden changes. So the modes are the absence of those, and claim nothing more. Measured with optical flow on the screen (feature speed in units of the window's short side per second, 90th percentile, over 15 s of each scene): focus 0.0009–0.0034, relax 0.0012–0.0084, lively 0.018–0.041; the note's limits are 0.005 and 0.010. Switching modes: the limiter held nothing, and the fine-flicker probe counted 0.006% of cells.
**Considered:** a gray floor of 0.10 (the first try: the picture became a dull gray panel; a dark floor keeps the look of the other modes, only dimmer); separate calm scenes (twelve more to maintain, and the calm should apply to any scene).
**Cost:** one more `vec4` of uniforms and a few instructions in the composite. Judged by numbers and screenshots only: whether the modes are pleasant to have on for an hour is still to be seen.

### D42 — Wallpaper mode on Windows, with a tray icon

**Decided:** `W` or `--wallpaper` makes Juglar the desktop's wallpaper on the display its window is on: the window, borderless and display-sized, is placed behind the desktop icons, through the undocumented message every live wallpaper uses (details in `crates/platform/src/win32/wallpaper.rs`, both the 24H2 layout and the earlier one). Keys no longer reach it, so a tray icon takes over: next scene, keep the scene, next mode, back to a window, quit. Once a second Juglar checks whether the desktop can be seen there; when a program runs in fullscreen or a maximized (or display-sized) window is in front, it stops drawing. Leaving wallpaper mode or closing puts the window back and leaves the desktop as it was. The app embeds a manifest declaring Windows 10 and 11.
**Why:** a visualizer that lives on the desktop is the M7 goal, and a wallpaper is what people can keep on all day, so it must cost nothing when covered. On Windows 11 24H2 the window ends up a layered child of `Progman` between the icons and Explorer's wallpaper layer; without the manifest, Windows treats the program as written for Windows 7 and refuses a layered child window. Measured with the synthetic song (no audio capture): 72 fps and 1.66% of one core with the desktop shown, 0 fps and 0.42% while covered; the tray's "Show in a window" and "Quit" work; after Juglar was killed by force, the desktop showed its own wallpaper again with no trace left.
**Considered:** a screensaver build (still planned; it only runs when idle); a fullscreen window kept at the bottom of the window order (it covers the icons).
**Cost:** a hidden window and a menu for the tray icon, and a few system calls a second. Not tested on several displays or on Windows before 24H2. The tray icon is Windows' generic program icon until Juglar has its own. Nothing judged by eye on a real desktop yet, only screenshots.

### D43 — Tempo and beat phase, counted from the spectral flux

**Decided:** the analysis follows the tempo and where the beat falls, and `Signals` carries both (`tempo` in beats per minute, 0 while unknown; `beat_phase`, 0 on the beat rising to 1 before the next). Once per hop, the spectral flux (low and middle regions in full, high at half) minus its last second's average is spread over three hops and autocorrelated as a running average over about six seconds. Each lag between 60 and 190 per minute is scored by its correlation plus half that at twice the lag; a weight toward 120 per minute (one octave of spread) picks among the peaks, the unweighted score places the chosen one between hops. A different period must win for 2 s before it replaces the current one. A counter runs at that period; the flux is added into 48 bins by the counter's position, fading over 4 s, and the bins' squared center of mass is where the beat is. Below a confidence of 0.15 (correlation at the period over that at lag zero) the tempo is unknown. The synthetic song reports its own 120 and phase.
**Why:** the detector says *that* a drum was hit; to land a change on a beat, or to count through a bar the drums leave out, Juglar has to know when the next one comes. Measured end to end, with a drum file played through the speakers and captured like any music: a 100 per minute rock pattern read 100 and 128 per minute four-on-the-floor read 128, every second after the first few, with every detected beat (17 of 17, 22 of 22 per ten seconds) within a tenth of a beat of the count; a breakbeat at 174 read 87, its half, which is how many people count that style. On the tests, clicks give a confidence of 0.74–0.88 and loud hiss 0.07. On fast black metal the tempo held around 130 and a third of the detected beats fell on the count, against a fifth by chance: it counts along, loosely, with music that is mostly a wall of onsets. Spreading the bumps was needed: a period between two hops (37.5 at 150 per minute) lined bumps up only every other beat, and its half tempo won. Placing the peak with the weight moved it: over a minute of metal the tempo crept from 134 to 118, toward 120.
**Considered:** the intervals between detected beats (a missed or extra beat breaks them, and dense music has onsets everywhere); a bank of resonating filters (more state for the same answer); tracking bars and downbeats (not yet: nothing uses them, and they are harder to be sure of).
**Cost:** about 190 multiplications per hop, and 48 more for the phase: the analysis' median stayed at 0.080–0.083 ms per frame. The tempo needs a few seconds to settle at the start of a song and about 6 s to follow a change. Real music was checked for steadiness and against chance, not against known tempos.

### D44 — Build-ups hold changes back; the drop gets its own moment

**Decided:** the director hears *tension*: the treble outgrowing, over several seconds, the deep bass (spectrum bands under about 120 Hz), as in a build-up where risers and snare rolls grow and the kick and bass line are held back. Above 0.4 it is a build-up: moments within it change nothing, and a change due by time waits for the drop (at most 30 s more). The drop is a moment of its own: within 1.5 s of a build-up, the deep bass hitting harder than over the last six seconds (less half of what the middle does at the same time) by more than 0.3, and by more than 1.25 times the build-up's hardest hit from half a second to four seconds before. A drop needs no listening, may come after half the scene's usual time, and brings the new scene in with a 1.5 s bloom. Changes land on a detected beat or on one the tempo counts.
**Why:** a VJ doesn't change scene halfway up a build-up, and does change on the drop's first beat. Before, the change came after half a second of listening, on the next beat (32.4–33.1 s in the synthetic song, whose drop is at 32 s); now at 32.0 s. Each part of the rule came from a failure, measured with a build-up and drop (128 per minute) played through the speakers: with the `bass` level (up to 250 Hz), the snare roll's body made the tension let go seven seconds early; with novelty as the moment, nothing happened at the drop, because novelty had stayed above its threshold since the build-up and had nothing to jump with; with the deep bass's jump alone, the snare roll's hits counted (it has deep bass too, as noise), so the middle's jump is taken off, leaving at most 0.13 for the snare roll and 0.62 for the drop's first kick; in the synthetic song the build-up keeps a kick every other beat, at 0.32–0.41, against 0.54 for the drop, hence the comparison with the build-up's own hits. With all of it, the file's drop was caught on its first kick and nothing else was.
**Considered:** waiting for the tension to let go (a beat or two late, when the point is to be on time); counting bars to predict the drop (build-ups are 8 or 16 bars often, not always, and downbeats aren't tracked).
**Cost:** a few more followers per frame. Not checked on real music with build-ups: the playlist at hand is metal, which has none. On four minutes of it the tension passed 0.4 in 18 seconds (riffs with rising cymbals), with no drop; those seconds hold changes back. Build-ups that keep their kick are the rule's weak spot: in explorable 06, a build-up kick as hard as the drop's punches 0.47 against the drop's 0.48, and no drop is found; even a soft one reaches 0.41 against 0.49, since the analysis levels the spectrum to its loudest band. Then the change waits for an ordinary moment after the tension lets go, as before.

### D45 — Kick, snare and cymbals apart, from a causal percussive/harmonic split

**Decided:** on the onset detector's transform (21 ms window, 10.7 ms hop), each band's usual level is the median of its last 0.2 s; what a band has above it is percussive, the rest harmonic. Kick, snare and cymbals are looked for in their own regions (40–120 Hz, 165–2500 Hz, 2.5–16 kHz) with the onset detector's flux, rises counted only from the usual level up. Each drum's event is 1 at a hit and falls back over 0.04 s (kick) or 0.05 s; a drum can fire again after 40 ms. `Signals` carries `kick`, `snare`, `cymbals`, and the `percussive` and `harmonic` levels, which share one automatic gain. `beat`, the tempo and novelty are unchanged.
**Why:** scenes built in layers (a pulse, an accent, a grain over a breathing background) need each drum and the held part of the music apart, and `beat`, made for a pulse, can't give them: in metal the onset detector's low region found 1% of the kicks, and its 0.15 s fall and 100 ms wait merge a double kick into one glow. A median only looking back costs no delay for the hits; counting rises from the usual level keeps tremolo picking's dips from looking like hits. Against isolated drum tracks of three metal songs (D46), kick, snare and cymbals score F 48, 51 and 63, against 3, 43 and 57 for the onset detector's three regions.
**Considered:** the textbook median split, which also looks ahead and across frequency (a delay of a tenth of a second or more on every hit); a trained drum transcriber (better on recordings like its training data, but a model to ship and run per frame, and not explainable in a page); counting hits on `beat`'s regions with shorter decays (their thresholds fail on dense drumming, below).
**Cost:** 0.014 ms more analysis per frame (0.059 to 0.073 ms over a minute of music; live, 0.082 to 0.100 ms). About half of each drum goes unheard in a loud master, the kick most (35% found); see D46 and note 12 for the rest. The tempo still counts from the onset detector's flux: counting it from the percussive flux was not tried.

### D46 — Thresholds and spill rules for metal, measured against the drummers' own tracks

**Decided:** the kick and cymbal thresholds are the median of the last second's flux plus four median absolute deviations; the snare's is 1.2 times the median of the last second's local peaks; each has a floor that keeps 30 s of white noise silent. A kick must rise at least 1.5 times as much (in decibels) as the snare's region and needs its beater's click at 2.5–6 kHz, unless its thump alone is very large; a sharp click with no snare or cymbal in it is a kick on its own. A snare must rise at least 0.6 times as much as the kick's region. A cymbal's percussive power above 6 kHz against 2.5–6 kHz must be at least what the harmonic part has there, averaged over a second. The constants were chosen against isolated drum tracks of three songs (Amon Amarth, Behemoth, Gojira; studio stems shared online), aligned with Spotify's mix by cross-correlating onsets; a plain detector on the isolated drums gives the reference, a hit within 35 ms counts.
**Why:** each rule fixed a measured failure. Mean plus deviations went deaf in a double kick, whose hits lift both above every hit; the median of a train that leaves half its hops quiet doesn't move. A snare threshold that bends to dense trains fired on every tremolo pick; the typical peak is the picking itself. With a looser click requirement, the synthetic guitar alone fired 28 kicks in 8 s, one per bass pick; the click on its own catches kicks whose thump the bass hides (in dense masters a kick's thump rises about 3 dB): under the synthetic guitar, 89 of 92 double-kick hits instead of 78. Cymbals over 6–16 kHz alone scored F 28 on the real tracks: in these mixes cymbals rise most at 2.5–6 kHz, where kick clicks and snare cracks also land, so the region was widened and the brightness test, measured against the music's own spectrum, tells them apart (F 63; in the synthetic pattern no kick or snare fires a cymbal). Synthetic drums were matched to the real mixes: a synthetic snare whose wires rang at 2.8 kHz rose 8 dB above 2.5 kHz under the guitar, where real snares rise 1.3 dB.
**Considered:** vetoing cymbals within a hop of a kick or snare (real F fell to 65 from 77 before the brightness test: in metal they are struck together); lower floors (real kick F 62, but white noise fired 38 kicks in 30 s); the snare's peak rule for every drum (a blast beat's hits are all peaks, and the rule hides them).
**Cost:** tuned on three songs and synthetic tests, so it fits those best. In a synthetic blast beat a third of the kicks are found and a quarter of the snares pass for kicks; a snare struck with a kick (a hammer blast) is mostly lost; a drum alone over digital silence fires the others, since any spill into an empty region is a large rise in decibels.

### D47 — Relative levels: each level against its own last four seconds

**Decided:** `Signals::relative` holds bass, mid, treble, energy, percussive and harmonic, each as a linear amplitude smoothed like its meter and divided by its own average over the last 4 s (over all that has been heard, until 4 s have). 1 is usual, 1.3 is 30% above, 0.5 half; 0 in silence, never above 4. Silence leaves the average where it was. The synthetic song computes the same from its own levels; the calm modes keep them near 1.
**Why:** the levels are leveled to each song's peaks, so in loud music they sit near 0.9 through the quietest riff and the heaviest; a layer that should grow when the drums get busier, or a chorus that should open up, needs *more than before*, not *loud*. Dividing by a moving average gives that in one number that means the same in any song.
**Considered:** differences in decibels (the same information, but 0 for usual reads as "nothing" to anyone wiring a scene); replacing the levels (scenes depend on them); a longer average (a section of a song lasts 10–30 s, and the point is to see it start).
**Cost:** six more followers per frame. A level coming back after a long silence is compared with the song before it, which may be another song.
