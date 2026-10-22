# 08 — Bloom

Look at a street lamp at night, or a bright window from a dark room: the light seems to spill past its edges into a soft halo. Some of it happens in the eye, some in a camera's lens: a little of the light from every bright point scatters sideways. A picture on a screen can't be bright enough to make the eye do it, so renderers paint the halo in. That is **bloom**. In Juglar it makes *Orbits*' lights look like neon instead of colored dots, and gives *Tide*'s ring a faint corona. The code is in `crates/render/src/bloom.rs`, `bloom.wgsl` and `composite.wgsl`.

## 1. A glow is a very wide blur

Bloom is three steps: keep only what is bright, blur it a lot, add it back. The middle step is the expensive one. A halo a tenth of the screen wide, done directly, would make every pixel average a hundred or so pixels around it in each direction: millions of reads per pixel row. No.

## 2. Blurring by shrinking

The trick is that shrinking an image is a blur. Halve an image's width and height, each new pixel averaging a 2×2 block of the old, and you've blurred it by a couple of pixels. Halve it again: each pixel now stands for a 4×4 block of the original. Shrinking costs almost nothing, because each step has a quarter of the pixels of the one before. Enlarging the small image smoothly afterwards gives a blur as wide as the blocks were.

Juglar's chain:

1. The scene's light is drawn once more, at a quarter of the window's width and height, keeping only what is bright: nothing below a luminance of 0.2, everything above 0.8, a smooth ramp between.
2. That image is halved twice: an eighth and a sixteenth of the window. Each pixel averages a 4×4 block of the level above, read as four filtered samples (the GPU's filtering blends four pixels per read for free).
3. When the final picture is put together, all three are read, enlarged smoothly, and averaged. The quarter-size one gives a tight halo, the sixteenth a wide haze; together they look like a real glow, bright near the source and fading far out.

Enlarging an image sixteen times with the GPU's plain filtering shows a pattern of diamond-shaped steps. Each blur is therefore read as four filtered samples half a pixel apart, which amounts to a tent-shaped filter over 3×3 of its pixels: smooth.

## 3. The cost of a pass

The first version was the textbook one: five sizes, from a half down to a thirty-second, then back up, each level enlarged onto the one above in a pass of its own. It looked fine and the GPU barely noticed (0.07 ms). The CPU did: the whole process went from 6.8% to 8.4% of a core.

The reason is that every render pass costs the CPU about 25 µs to hand to the graphics driver, however small its image. Nine passes added 0.22 ms to each frame, more than all the rest of the drawing. Cutting to three sizes starting at a quarter, and doing the enlarging inside the final picture's pass instead of in passes of their own, leaves three passes. The halo is a little less wide; nobody would notice.

| Measured on the development PC (GTX 1060, DirectX 12, release, music, 75 Hz) | 960×540 | maximized, 1920×1009 |
|---|---|---|
| GPU time per frame, without bloom | 0.163 ms | 0.529 ms |
| GPU time per frame, first version (9 passes) | 0.230 ms | 0.71–0.73 ms |
| GPU time per frame, now (3 passes) | 0.222 ms | 0.73–0.74 ms |
| Whole process CPU, without bloom | 6.75–6.79% of a core | 6.65–6.68% |
| Whole process CPU, first version | 8.38–8.41% | 8.31–8.32% |
| Whole process CPU, now | 7.33% | 7.29–7.36% |

Two runs of 15 s per case. The GPU time now is slightly higher than the first version's, because the final pass reads twelve more samples per pixel; it is still under 6% of a frame at full size. A scene that asks for no glow skips the chain entirely.

## 4. Adding light without passing white

Screens stop at white. Adding a glow on top of a picture that is already bright would clip into flat white patches. Juglar combines them the way light through two stacked slides combines (photo editors call it **screen**): the result is 1 − (1 − picture) × (1 − glow). Over black it adds the glow in full; near white it adds almost nothing; it never passes white.

Each scene says how much glow it wants (`bloom` in its look): *Tide* a little (0.35), *Orbits* the most (0.9).

## 5. HDR, and why not yet

In many renderers the picture is computed in **high dynamic range**: a lamp can be ten or a hundred times brighter than a white wall, numbers well above 1. The glow is made from those values, before they are squeezed into the screen's range, so the brightest things bloom the most. Juglar's ink already goes above 1 (note 05), and the tone-mapping curve squeezes it (note 06); the glow is made after the squeeze. That is simpler, keeps every number between 0 and 1, and with bright parts chosen by luminance it looks right. Real HDR output, to screens that can show it, is out of scope; note 09 explains why it would also change the flash rules.

## 6. Safety

A glow spreads light into its surroundings, so it can make a change in brightness cover more of the screen. It is computed before the flash limiter (note 09), which checks the final picture with the glow in it. With music, 20 s per scene, the limiter held back nothing in any of the four scenes (the first, wider version held 0.02% of *Tide* on average, 0.1% at most).

## A display oddity, not Juglar's

While looking at *Orbits* closely, screenshots showed single black pixels near moving lights, a few dozen per frame. Frames read back from the GPU right before they are handed to Windows have none (25 frames checked, zero black pixels), while screenshots of the same run had 43 to 67. They appear somewhere after Juglar, in the composition of the desktop or in capturing it. Whether they are visible on the monitor itself needs a human eye.

## Further reading

- Jorge Jimenez, "Next Generation Post Processing in Call of Duty: Advanced Warfare", SIGGRAPH 2014: the downsample/upsample chain most games use.
- Masaki Kawase, "Frame Buffer Postprocessing Effects in DOUBLE-S.T.E.A.L", GDC 2003: blurring by repeated small filtered reads.
- Erik Reinhard et al., *High Dynamic Range Imaging*: tone mapping and why bright light needs it.
