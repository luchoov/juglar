# Explorables

Interactive explanations of the techniques behind Juglar. Each page runs the real algorithm (a faithful JavaScript mirror of the Rust code today; the actual Rust compiled to WebAssembly later) with controls you can move, next to the code that does it.

They exist to learn from, to show people how Juglar works, and eventually to live on juglar.dev.

Each page is a single self-contained HTML file with no dependencies: open it in a browser.

| # | Page | Explains |
|---|---|---|
| 00 | [Start here](00-start-here.html) | Sound, frequency and amplitude (with audio), samples, why music is hard to read, Juglar's pipeline, glossary, reading order. No prior knowledge needed |
| 01 | [How Juglar hears](01-how-juglar-hears.html) | Sums of waves, the FFT, windowing, the time–frequency trade-off, log bands, attack/release envelopes |
| 02 | [How Juglar draws](02-how-juglar-draws.html) | One shader for every pixel, mixing in linear light, the oscilloscope trigger, frame pacing, what a frame costs |
| 03 | [Feedback and color](03-feedback-and-color.html) | The feedback loop running live (Tide on the synthetic song), painting twice, backward warping, float storage, cosine palettes and the red check, tone mapping, dithering |

Planned (see `docs/vision.md`): chroma wheel and key detection, harmonic/percussive separation, the self-similarity matrix filling in as a song plays, beat prediction and the build-up tension curve, procedural composition.

## Conventions

- **Understandable without reading any code.** The prose and the interactive figures carry the whole explanation; code excerpts are collapsed, optional extras for the curious.
- One concept per page, numbered in the order you'd learn them.
- Show the algorithm working on something you control, not a recording of it.
- Quote the real Rust next to the visualization it drives; keep the JavaScript structured the same way so the two can be compared line by line.
- Respect `prefers-reduced-motion` (animations start paused) and pause anything off screen.
- Visual style: dark and sober. Near-black background, 1 px lines, square corners, one monospace typeface, off-white traces, muted colors only where categories must be told apart. No orange, no serif/sans editorial pairing, no gradients, glows or decorative labels.
