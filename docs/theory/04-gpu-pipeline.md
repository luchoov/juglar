# 04 — The GPU pipeline, frame pacing and measuring

Milestone 3 puts the analysis on screen: a waveform, the 32-band spectrum and the four band meters, drawn in a window by the GPU. Nothing about it is pretty yet. It is the plumbing every later effect runs through, so this note covers how a frame reaches the screen, how often frames are drawn, and how their cost is measured. The code is in `crates/render/` (`lib.rs`, `timer.rs`, `frame.wgsl`) and the loop in `app/src/main.rs`.

## 1. CPU and GPU divide the work

The **CPU** runs the program one step at a time. The **GPU** runs one small program, a **shader**, on thousands of pixels at once. Drawing a 960×540 window means deciding the color of about half a million pixels, 60 times a second. That job belongs to the GPU.

So each frame the CPU does very little:

1. Drain the new audio, run the FFT and bands (Milestone 2): under 0.1 ms.
2. Copy the results into one block of about 1 KB: 4 band levels, 32 spectrum values, 256 waveform points, the time and the window size.
3. Tell the GPU "draw one triangle with this data", and hand the finished image to the screen.

The GPU then runs the shader once for every pixel.

## 2. The pieces wgpu needs

`wgpu` is a library that speaks each platform's GPU language: Vulkan, DirectX 12, Metal, or WebGPU in a browser. Juglar writes its shaders once, in **WGSL** (the WebGPU shading language), and the same code runs everywhere. That matters for the web version on juglar.dev.

| Piece | What it is |
|---|---|
| **Adapter** | One GPU plus one API to talk to it, e.g. "GTX 1060 via Vulkan" |
| **Device and queue** | The open connection to it, and the line where work waits to be run |
| **Surface** | The window's drawable area. Each frame you borrow an image from it, draw, and **present** it back |
| **Shader module** | The compiled WGSL program |
| **Pipeline** | The shader plus how its output is written (pixel format, blending). Built once at startup |
| **Uniform buffer** | The ~1 KB block of numbers, in GPU memory, rewritten every frame |
| **Bind group** | The link that tells the shader "your `frame` variable is that buffer" |

## 3. One triangle, every pixel

GPUs draw triangles. To run a shader on every pixel, Juglar draws **one triangle twice the size of the window**, so its visible part is exactly the window. A rectangle made of two triangles would also work, but the pixels along their shared diagonal would be shaded twice.

The **fragment shader** (`fs_main`) then answers one question per pixel: "what am I?" It knows its own position, so it checks:

- **Is it near the waveform line?** It works out which two of the 256 points this pixel column lies between and how far the pixel is from the line joining them. Close means light, far means background. On steep parts of the wave the distance is measured across the slope, not straight up and down, so the line keeps the same thickness everywhere.
- **Is it inside a spectrum bar or a meter?** It works out which bar from the x position, then checks whether the pixel is below that bar's top. The top edge is **antialiased**: a pixel half covered by the bar gets half its color. A bar rising by a fraction of a pixel still visibly moves.

The bars are colored by the musical band they belong to (bass, mid, treble), using the same palette as the explorables.

**Color is blended in linear light.** Screen values (sRGB) are not proportional to light: 50% gray on screen emits about 21% of white's light. Mixing colors in screen values makes edges look dark and thin. So the shader turns its palette into **linear** values, mixes there, and the result is encoded back to sRGB on the way out. Native windows usually offer an sRGB format that encodes for free. Browsers usually don't, so the shader then does the encoding itself.

## 4. Frame pacing

A display refreshes at a fixed rate (60, 144 Hz…) and can only show a new image at a refresh. If frames arrive at a rate unrelated to that, some stay on screen for one refresh and others for two, and motion stutters even at a high average frame rate.

Juglar aims at 60 fps but snaps to the display: it draws on every refresh, or every second, third… refresh, whichever whole number gives a rate closest to 60.

| Display | Draws every | Frame rate |
|---|---|---|
| 60 Hz | refresh | 60 |
| 75 Hz | refresh | 75 |
| 120 Hz | 2nd refresh | 60 |
| 144 Hz | 2nd refresh | 72 |
| 165 Hz | 3rd refresh | 55 |

The timer that wakes the program for each frame advances from the **previous deadline**, not from "now". Otherwise every small delay would push all later frames back, and the rate would drift below the target. After a long stall (the window was being dragged) it starts over instead of rushing out the missed frames. Between frames the program sleeps. Windows' high-resolution timers wake it within a fraction of a millisecond.

**Why not just use vsync?** Vsync makes presenting wait for the next refresh, so in theory a loop that just draws frames as fast as possible would be paced by the display. On the development PC it wasn't: with Juglar asking for vsync, the loop ran at **1,400 frames per second** and used 80% of a CPU core. GPU drivers let users force vsync off (gamers often do), and then nothing stops the loop. So Juglar still asks for vsync, which avoids tearing when the driver honors it, but sets its own pace with a timer.

The rate follows the window: dragged from a 144 Hz display to a 75 Hz one, Juglar switches from 72 to 75 frames per second as soon as the window arrives.

When the window is minimized, Juglar stops drawing and checks again only ten times a second. The audio analysis keeps running at that rate so the envelopes stay current. It has to ask the window itself: DirectX 12 happily accepts frames for a minimized window, and until this was measured (section 6) Juglar kept drawing 72 invisible frames a second at full cost. A window covered by other windows is still drawn; it costs about the same as a visible one.

## 5. Measuring the GPU

A stopwatch on the CPU can't time GPU work. The CPU only records commands, and the GPU runs them later, in parallel. Instead the GPU writes its own clock (a **timestamp**) into a small buffer at the start and at the end of the drawing. The difference, times the length of one GPU clock tick, is the GPU time.

Reading that buffer back takes a frame or two. Waiting for it would make the CPU idle until the GPU finished, which is exactly the coupling the design avoids. So the timer starts a measurement, keeps going, and picks the result up on a later frame once it's ready. Frames drawn while a measurement is in flight just aren't timed.

The window title shows, once a second: frames per second, the shortest and longest frame interval, CPU time spent on analysis, and GPU time. It is a stand-in until the HUD exists (Milestone 7).

## 6. What it costs

Measured on the development PC (GTX 1060 6 GB, Intel i7-7700 with 4 cores and 8 threads, Windows 11, DirectX 12), release build, with music playing, both monitors on: the primary at 144 Hz, where Juglar draws 72 frames per second, and the secondary at 75 Hz, where it draws 75. Each figure is the average of three 15-second runs; runs agreed within 0.1 points.

| | 144 Hz display (72 fps) | 75 Hz display (75 fps) |
|---|---|---|
| Whole process, window visible | **5.6% of one core** | **5.8% of one core** |
| of which the main thread | 5.4% | 5.6% |
| of which the audio thread | 0.13% | 0.13% |
| Window maximized (1920×1009) | 5.5% | 5.8% |
| Window covered by another | 5.2% | 5.4% |
| Window minimized | 0.4% | 0.3% |
| Half the frame rate (36 / 37.5 fps) | 3.0% | 3.1% |
| CPU per frame | 0.78 ms | 0.77 ms |
| GPU time per frame (Milestone 4) | 0.095 ms | 0.095 ms |
| Memory | 112 MB | 112 MB |

5.7% of one core is 0.7% of the whole 8-thread CPU. In silence, or in demo mode, the cost is the same within half a point: the work is in presenting frames, not in listening.

What came out of it:

- **Drawing is nearly free; talking to the driver is not.** The GPU needs a tenth of a millisecond. The CPU needs about 0.8 ms per frame, almost all of it on the main thread to record, submit and present the work, whatever is drawn. So the frame rate, not the picture, sets the CPU cost: halving the frame rate halved it. This is why the roadmap pauses rendering on silence and lowers the rate while a game runs.
- **Measure the whole process, not one thread.** An earlier round timed each step of the frame on Juglar's own thread, and Vulkan looked two to three times cheaper than DirectX 12. Counting every thread showed that NVIDIA's Vulkan driver runs a helper thread of its own that does the rest of the work, and in total Vulkan cost *more* CPU than DirectX 12 (decision D19, which replaces D17). Counted in cycles, with the monitors on, it is 6.4% against 5.8% of a core at 75 frames per second, DirectX 12 lower in each of three alternating pairs of runs; Vulkan also used 7–36 MB more memory. `JUGLAR_GPU_API=vulkan` forces it, to repeat the comparison. With DirectX 12, on this GPU, the driver's own threads use almost nothing: the main thread is the cost.
- **Measure in cycles, not in clock ticks** (decision D26). Windows' usual CPU-time figure (what Task Manager and `TotalProcessorTime` report) isn't timed: at every tick of the system clock, every 15.6 ms, Windows charges the whole tick to whichever thread happens to be running. That averages out for work that runs at random moments. Juglar's work doesn't: it runs in short bursts locked to the display, and at 72 frames per second on the 144 Hz display its bursts can stay in step with the ticks for many seconds. The same visible window, drawing the same 72 frames per second, then measured anywhere from **0% to 12.6%** of a core, depending on luck. Windows also counts every thread's processor cycles exactly, and that count gave 5.6% in every run. On the 75 Hz display the two methods happened to agree within half a point.
- **That explains the mysterious cheap runs.** Earlier rounds, with the monitors switched off, sometimes measured 0.2–0.3% while drawing 72 frames per second, and the guess was a hidden window that Windows didn't compose. With the monitors on, a window covered by another costs only 0.4 points less than a visible one, so that guess is ruled out. The cheap readings were the tick counting above. The earlier round's 6.3% at 75 frames per second is close to today's 5.8%; at 75 Hz the tick counting was reliable.
- **Minimized windows were drawn at full cost.** Tested with cycles, a minimized Juglar still used 5.4% of a core: DirectX 12 never reports a minimized window as hidden, so frames kept coming. Juglar now checks the window itself and drops to 0.3–0.4%.
- **Memory is the weak spot.** Milestone 2 used 12 MB; opening the GPU adds about 100 MB, mostly the driver's own allocations. Still far below an Electron app, and worth revisiting.

A separate test showed why the pacer snaps to the display: forced to draw 72 frames per second on the 75 Hz display, in plain view, Juglar got intervals from 4.8 to 22.7 ms instead of a steady 13.9.

All of this is one GPU from one vendor. AMD and Intel drivers split the work differently, so the comparison has to be repeated there.

## Further reading

- *Learn Wgpu* (sotrh.github.io/learn-wgpu): the standard tutorial for the pieces in section 2.
- The WebGPU and WGSL specifications (gpuweb.github.io): the model wgpu follows.
- Alen Ladavac, "The Elusive Frame Timing" (GDC 2019): why steady frame intervals matter more than a high frame rate.
