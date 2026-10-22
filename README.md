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

Play anything on the system. Juglar listens to what the speakers play, with no setup. `Esc` closes the window. The window title shows the frame rate, frame intervals, and the analysis (CPU) and drawing (GPU) time per frame. `JUGLAR_FPS=36` (or any rate) lowers the target frame rate.
