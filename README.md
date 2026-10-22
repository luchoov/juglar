# Juglar

**Juglar** /xuˈɣlaɾ/ — Spanish for *minstrel*: the medieval performer who brought music and spectacle together.

A lightweight, GPU-driven music visualizer written from scratch in Rust.

## Start here

New to all of this? Open [`explorables/00-start-here.html`](explorables/00-start-here.html) in a browser. It explains sound, samples and what Juglar does from zero, with no music theory or programming needed, and links the rest of the documentation in reading order.

- [Vision](docs/vision.md): what makes Juglar different.
- [Decisions](docs/decisions.md): every important choice, in plain language.
- [Roadmap](docs/roadmap.md) and [theory notes](docs/theory/): what's built and how it works.

## Running it

Windows 10 or 11 with a GPU that supports Vulkan or DirectX 12, and Rust installed.

```
cargo run --release
```

Play anything on the system. Juglar listens to what the speakers play, with no setup. It changes scenes by itself when the music turns a corner (a drop, a breakdown, a new song), on the beat, choosing one that suits what is playing.

| Key | Does |
|---|---|
| `Tab` | switch between the visuals and the analysis view (what Juglar hears) |
| `←` `→` | previous / next scene, with a short transition: *tide*, *orbits*, *kaleido*, *oil*, *plasma*, *harmonograph*, *julia*, *crystal*, *vortex*, *corona*, *melt*, *lenses* |
| `Space` | keep the scene showing (no automatic changes) / let go |
| `A` | natural intensity, read from the music (the default) |
| `1` `2` `3` | fixed intensity: gentle, medium, strong |
| `M` | mode: lively (the default), focus, relax: calmer, dimmer and muted, changing scenes rarely (see below) |
| `F11`, `Alt`+`Enter` or double click | fullscreen on / off |
| `Esc` | leave fullscreen; from a window, close |

`cargo run --release -- --fullscreen` starts in fullscreen.

The window title shows the frame rate, frame intervals, the analysis (CPU) and drawing (GPU) time per frame, the share of the screen the flash limiter is holding back (`held`), beats per second, the intensity, and the scene's name, marked *(kept)* while `Space` holds it.

**Focus** and **relax** are style options for when the music matters more than the picture: motion slowed to a crawl and no camera flow, colors muted, the picture dimmed and low in contrast, scenes held for minutes and changed by a slow seep, and the music followed only faintly. They come from the limits in `docs/research/focus-and-relaxation.md` and claim nothing about attention or stress.

Environment variables, until there is a settings file: `JUGLAR_FPS=36` (or any rate) lowers the target frame rate; `JUGLAR_INTENSITY=0.3` fixes the intensity (0 to 1) instead of reading it from the music; `JUGLAR_REDUCED_MOTION=1` keeps the picture still (by default Juglar follows Windows' "Show animations" setting); `JUGLAR_SCENE=oil` (or any scene's name) starts on that scene; `JUGLAR_MODE=focus` (or `relax`) starts in that mode; `JUGLAR_DEMO=1` plays a silent synthetic song instead of listening, for working on visuals without music; `JUGLAR_STATS=1` also prints the title's numbers to the terminal once a second; `JUGLAR_GPU_API=vulkan` (or `dx12`) forces a graphics API, to compare their cost.
