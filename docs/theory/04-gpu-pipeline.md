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

When the window is minimized or hidden, nothing can be drawn, and Juglar checks again only ten times a second. The audio analysis keeps running at that rate so the envelopes stay current.

## 5. Measuring the GPU

A stopwatch on the CPU can't time GPU work. The CPU only records commands, and the GPU runs them later, in parallel. Instead the GPU writes its own clock (a **timestamp**) into a small buffer at the start and at the end of the drawing. The difference, times the length of one GPU clock tick, is the GPU time.

Reading that buffer back takes a frame or two. Waiting for it would make the CPU idle until the GPU finished, which is exactly the coupling the design avoids. So the timer starts a measurement, keeps going, and picks the result up on a later frame once it's ready. Frames drawn while a measurement is in flight just aren't timed.

The window title shows, once a second: frames per second, the shortest and longest frame interval, CPU time spent on analysis, and GPU time. It is a stand-in until the HUD exists (Milestone 7).

## 6. What it costs

Measured on the development PC (GTX 1060 6 GB, 8 cores, Windows 11), release build, with music playing. The first round ran while a heavy test suite kept the machine busy; the numbers below come from a second round on a quiet machine, with the display at 75 Hz, so Juglar drew 75 frames per second. Each figure is the average of several 15–20 second runs.

| | |
|---|---|
| GPU time per frame | **0.038 ms** with DirectX 12 (0.029 ms with Vulkan), out of 13.3 ms available: 0.3% of the frame budget |
| Frame intervals | 12.8–13.8 ms (target 13.3) |
| Audio analysis | 0.05 ms CPU per frame |
| Whole process | **6.3% of one core** with DirectX 12, 7.7% with Vulkan, which is under 1% of the whole 8-core CPU. 3.1% at half the frame rate |
| Memory | 100–115 MB with DirectX 12, ~120 MB with Vulkan, almost all of it the GPU driver |

What came out of it:

- **Drawing is nearly free; talking to the driver is not.** The GPU needs 0.04 ms. The CPU needs about 0.85 ms per frame, almost all of it to submit the work and present the image, whatever is drawn. So the frame rate, not the picture, sets the CPU cost: halving the frame rate halved it. This is why the roadmap pauses rendering on silence and lowers the rate while a game runs.
- **Measure the whole process, not one thread.** The first round timed each step of the frame on Juglar's own thread, and Vulkan looked two to three times cheaper than DirectX 12. That was true for that thread only. Counting every thread showed that NVIDIA's Vulkan driver runs a helper thread of its own that does the rest of the work, and in total Vulkan cost about a fifth *more* CPU than DirectX 12, in every one of six alternating pairs of runs. Juglar now prefers DirectX 12 on Windows and keeps Vulkan as the fallback (decision D19, which replaces D17). It still prefers an integrated GPU when there is one, so a dedicated GPU stays free for games.
- **The noise was the other program.** In the first round the same build measured anywhere from 1.3% to 15% of a core. On the quiet machine, runs agreed within about one point. The cost now tracks the frame rate closely and no longer jumps from run to run. Whether a small part of each frame is the driver waiting actively instead of sleeping can't be told from these numbers, but it is not what made the first round so noisy.
- **Memory is the weak spot.** Milestone 2 used 12 MB; opening the GPU adds 90–110 MB, mostly the driver's own allocations. Still far below an Electron app, and worth revisiting.

All of this is one GPU from one vendor. AMD and Intel drivers split the work differently, so the comparison has to be repeated there.

## Further reading

- *Learn Wgpu* (sotrh.github.io/learn-wgpu): the standard tutorial for the pieces in section 2.
- The WebGPU and WGSL specifications (gpuweb.github.io): the model wgpu follows.
- Alen Ladavac, "The Elusive Frame Timing" (GDC 2019): why steady frame intervals matter more than a high frame rate.
