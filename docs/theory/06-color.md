# 06 — Color: palettes, tone mapping and dithering

The feedback loop (note 05) leaves every pixel with an amount of **ink** and a **palette position**. The composite step turns those into the colors on screen, once per screen pixel, at full resolution. It does three things: picks a color from a palette, decides how bright a given amount of ink looks, and hides the steps between the screen's 256 levels. The code is `crates/render/src/composite.wgsl`, with the palette itself in `scene/tide.rs`.

## 1. A palette as a formula

A palette could be a list of colors to blend between. Juglar uses a formula instead, popularized by Íñigo Quílez, the **cosine palette**:

> color(t) = a + b · cos(2π · (c · t + d))

computed separately for red, green and blue. Each channel is a wave that rises and falls as `t` grows:

| Number | Sets, for each channel |
|---|---|
| `a` | its middle value: the palette's average color |
| `b` | how far it swings above and below: the contrast |
| `c` | how many swings per unit of `t`: how often colors repeat |
| `d` | where in its swing it starts: which color comes first |

Twelve numbers describe a whole smooth loop of colors, the GPU computes it in a few instructions, and changing one number shifts the whole palette coherently. Milestone 6's generator, and harmony as color (`docs/vision.md`), will need exactly that.

## 2. Tide's palette

The palette is computed in sRGB, the screen's own encoding, and converted to linear light for blending (note 04).

The first palette went through blue, teal and sand, the explorables' three band colors, brightened. On screen it failed in a way the colors alone don't show: the stretch between teal and sand passes through yellow-green, and **dim yellows and greens look muddy**: olive, khaki, brown. Trails are dim almost everywhere, so loud parts of songs turned the screen into murky fog.

The palette now in use runs from **sand** to **steel blue** and stops there. With `c = 0.5` each channel makes only half a swing between `t = 0` and `t = 1`, so it moves in one direction the whole way, and the composite clamps positions to that range:

| Position | Color | |
|---|---|---|
| 0 | #dcc69a | sand: the present, in loud music |
| 0.25 | #ccc0a2 | |
| 0.5 | #a6b0b5 | silver |
| 0.75 | #7fa1c9 | |
| 1 | #6f9bd1 | steel blue: the past, and calm music |

Between the two ends the path passes through desaturated silver, never green. In *Tide*, the line is drawn at a position set by the music's *mood* (warmer when the song is full), its trails start a quarter of the way cooler, and they keep cooling as they age. Loud passages read as warm light leaving blue smoke behind; quiet ones are cool throughout.

## 3. How bright is ink? Tone mapping

Ink has no upper limit; a screen pixel does. Clipping ink at 1 would turn every dense patch into a flat, featureless blot. **Tone mapping** squeezes any amount into 0..1 with a soft shoulder, the way film responds to light:

> light = 1 − e^(−color × ink × exposure)

A little ink gives light proportional to it; a lot approaches 1 without ever hitting a wall. It is applied **per channel**: the palette's strongest channel saturates first, the others catch up, and so very dense ink drifts toward white, like an overexposed photograph. The ring's line, drawn with the most ink, glows almost white at its core, with color at its edges.

Before that, a gentle **contrast curve** (ink² / (ink + 0.2)) lowers faint ink more than dense ink. Faint haze fades into the background, and the structure of the trails stays visible.

## 4. Hiding the steps: dithering

A screen shows 256 levels per channel. In a slow gradient across a dark area (exactly what fading trails make) the color might change by one level every 30 pixels. The eye sees those steps as **bands**.

The fix is counterintuitive: add noise. Before the final color is rounded to the screen's levels, Juglar adds a random amount between −1 and +1 level, different for every pixel and every frame. A pixel whose true value is 3.3 now rounds to 3 most of the time and to 4 some of the time. On average, the screen shows 3.3, and the hard edge between 3 and 4 dissolves into a grain far too fine to notice.

Details that matter:

- The noise is the sum of two random numbers (**triangular** noise). A single random number would make the grain stronger at some brightness levels than at others, and a smooth gradient would show faint stripes of grain.
- It is added in the screen's encoding (sRGB), where the 256 levels are, not in linear light.
- It changes every frame, so the grain never forms a fixed pattern.
- Since Milestone 5 it is the very last step, after the flash limiter (note 09), which needs the colors before rounding.

## 5. The background

The background is the explorables' near-black, #09090b, darkened by a faint **vignette** toward the corners, which keeps the eye near the middle. Most of the screen stays close to black most of the time; Juglar is meant to sit next to other things, not shout over them.

## 6. Color and safety

From the research note's limits (see also note 05, section 8):

- **No saturated red flashes.** A color counts as saturated red when red carries 80% or more of its light. The policy keeps animated colors at or under 75%. Tide's reddest color, sand, is at 45%. A test sweeps the whole palette.
- **Brightness follows mood, not beats.** The ink of the line is fixed; what the music changes quickly is shape and motion.

## Further reading

- Íñigo Quílez, "Palettes" (iquilezles.org/articles/palettes): the cosine palette, with many examples.
- Erik Reinhard et al., "Photographic Tone Reproduction for Digital Images" (SIGGRAPH 2002): why and how to compress light into a screen's range.
- Mikkel Gjøl, "Banding in Games: A Noisy Rant" (Playdead, 2016): dithering explained by someone who shipped it.
- Explorable `03-feedback-and-color.html`: drag the palette's numbers and watch banding appear and disappear.
