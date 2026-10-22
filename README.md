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

Play anything on the system. Juglar listens to what the speakers play, with no setup.

| Key | Does |
|---|---|
| `Tab` | switch between the visuals and the analysis view (what Juglar hears) |
| `←` `→` | previous / next scene: *tide*, *orbits*, *kaleido*, *oil*, *plasma*, *harmonograph*, *julia*, *crystal*, *vortex*, *corona*, *melt*, *lenses* |
| `1` `2` `3` | intensity: gentle, medium, strong |
| `F11`, `Alt`+`Enter` or double click | fullscreen on / off |
| `Esc` | leave fullscreen; from a window, close |

`cargo run --release -- --fullscreen` starts in fullscreen.

The window title shows the frame rate, frame intervals, the analysis (CPU) and drawing (GPU) time per frame, the share of the screen the flash limiter is holding back (`held`), beats per second, and the scene's name.

Environment variables, until there is a settings file: `JUGLAR_FPS=36` (or any rate) lowers the target frame rate; `JUGLAR_INTENSITY=0.3` sets the starting intensity (0 to 1); `JUGLAR_REDUCED_MOTION=1` keeps the picture still (by default Juglar follows Windows' "Show animations" setting); `JUGLAR_SCENE=oil` (or any scene's name) starts on that scene; `JUGLAR_DEMO=1` plays a silent synthetic song instead of listening, for working on visuals without music; `JUGLAR_STATS=1` also prints the title's numbers to the terminal once a second; `JUGLAR_GPU_API=vulkan` (or `dx12`) forces a graphics API, to compare their cost.
