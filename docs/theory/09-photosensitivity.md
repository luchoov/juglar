# 09 — Flashes, red and the limiter

A small share of people can have a seizure triggered by light that flashes, by saturated red that flickers, or by bold stripes. Many more get headaches or feel sick from the same things. A music visualizer is exactly the kind of program that can produce them by accident: it reacts to drums, and drums are fast. This note explains the rules Juglar follows, how the **flash limiter** enforces the first of them on every frame, and what it can't see. The code is in `crates/render/src/limiter.rs` and `limiter.wgsl`.

No set of limits makes a picture safe for every viewer, and this one hasn't been checked by an independent analyzer. Juglar never calls itself "epilepsy-safe".

## 1. The rules

The numbers come from `docs/research/focus-and-relaxation.md`, section 9, which draws on WCAG 2.2 (success criterion 2.3.1) and ITU-R BT.1702-3:

- **At most two flashes in any second**, measured over a rolling window, not per calendar second. WCAG allows three; Juglar keeps a margin.
- **A flash** is a pair of opposing changes in luminance of 0.1 or more (on a scale where black is 0 and white is 1): brighter then darker, or darker then brighter.
- **No area exception.** WCAG ignores flashes smaller than about a quarter of a 10° patch of vision. Juglar doesn't know how far away you sit, so it counts flashes of any size.
- **No saturated red.** No color may carry more than 75% of its light in red; flickering red is counted as hazardous from 80%.
- **Few stripes.** At most five light/dark pairs of stripes in a regular pattern.

The first rule is the one music pushes against, and the one the limiter enforces. The red rule is enforced by the limiter too, more simply. The stripe rule still holds only by design: scenes avoid regular stripes (note 05), and nothing measures them.

## 2. Luminance

"Brighter" here means **luminance**, how bright a color looks, in linear light (note 06): Y = 0.2126 R + 0.7152 G + 0.0722 B. Green counts most because the eye is most sensitive to it; a pure blue at full power is darker than a mid gray.

## 3. Counting flashes, not frame differences

Comparing each frame with the previous one misses slow flashes: a brightness that rises by 0.03 per frame for five frames and then falls the same way never changes much from one frame to the next, but it swung by 0.15 and back. So the limiter counts **transitions** the way the definition does: it follows the luminance, remembers its last extreme (the brightest point while it rises, the darkest while it falls), and when the luminance swings back from that extreme by 0.1 or more, that is a transition, and the extreme starts over from there. Two transitions make a flash; the limit is four transitions in any second.

Time is measured in seconds, from each frame's real duration, not in frames: at 144 frames per second the window is still one second.

## 4. Where: cells

A flash in one corner is a flash, even if the rest of the screen is calm, and the average of the whole screen would hide it. So the screen is divided into square **cells**, 8×8 pixels when the window is 1080 pixels tall (proportionally larger or smaller otherwise), and each cell is watched on its own, by its mean luminance.

Why 8 pixels: a cell can only see what changes its mean. A line one pixel wide flashing from black to white covers an eighth of its cell, so the cell swings by 0.125 and is counted. With 30-pixel cells, the first version, the same line moved its cell by 0.03 and went unseen. Smaller cells cost no more to measure (every pixel is read once either way); there are just more of them, 32,400 for a full-HD screen, each remembering eight numbers. Cells of 16 pixels were tried too, hoping they would ignore a thin line that trembles by a pixel; they didn't (see the next section), and they would miss more, so 8 stayed.

## 5. Holding back

When a cell has had four transitions in the last second and the new frame would swing it back the other way by 0.1, the limiter doesn't drop the frame. It shows, in that cell, a mix of what the cell showed last frame and the new frame, with exactly as much of the new frame as keeps the swing at 0.09. Everything else on screen is shown as drawn. When the burst is over and a second has passed since the oldest transition, the cell catches up with the picture by itself.

What that looks like: wherever cells are held, small squares keep showing a bit of what was there before, like specks of afterglow.

**What it found in Tide.** Tide was designed not to flash: kicks move things instead of brightening them. With real music the limiter still held 1 to 1.5% of the screen, all of it along the ring's sharp line, which was left dotted with those specks. The line is bright and one or two pixels wide, and its shape is the waveform, which changes every frame. The line trembled by a fraction of a pixel to a few pixels each frame, and each kick swept it out and back; the pixels along it blinked several times a second. At a distance it looks like motion; at the scale of a pixel it is flashing, and Juglar's rule has no exception for small areas.

The fix was in the scene, not the limiter. Smoothing the waveform over time once (each frame moving a share of the way toward the new one) cut the holds only by half: a one-stage average still takes a small random step every frame. Three stages in a row, each following the one before over 0.25 s, leave no frame-to-frame step at all, and the ring's size, height and thickness now follow the music over 0.3 s. With the same music for 27 seconds, the limiter held nothing (0.0%); with the synthetic demo song, 0.05% at most. The ring is calmer, a slowly breathing shape instead of a trembling one, and its echoes in the tunnel are softer. Kicks still speed up the flow.

In the analysis view (`Tab`), the spectrum bars and the raw waveform jump many times a second, and about 1% of the cells are held with music: the tops of some bars leave faint square ghosts. That view shows the data as it is, so the limiter is left to do its job there.

The window title shows the share of cells held (`held 0.0%`), counted twice a second.

## 6. Before the screen, in the same frame

The research asks for the check to happen **before** a frame is shown. Reading the picture back to the CPU, as Juglar does to measure GPU time, takes one or two frames: by the time the CPU knew a frame broke the rule, it would already be on screen. So the whole limiter runs on the GPU, inside the frame it checks:

1. Every view (the scene, the analysis view) draws its frame into a **candidate** image instead of the screen, in linear light.
2. A first pass, one thread per cell, measures each cell's mean luminance in the candidate, updates the cell's memory and decides how much of the candidate the cell may show.
3. A second pass, one thread per pixel, mixes the candidate with the last frame shown by that amount, applies the red guard (below), adds the dithering (note 06) and writes the screen. The result is also kept, in linear light, as "the last frame shown" for the next one.

Nothing reaches the screen without passing through step 3. The CPU reads the cells' decisions back only to report them, a frame or two late, never waiting.

## 7. The red guard

For each pixel, after the mix: if red carries more than 75% of its light (R / (R + G + B) > 0.75), the color is moved toward a gray of the same luminance, exactly far enough to bring red's share to 75%. Keeping the luminance means the guard never creates a flash of its own. Tide's palette never comes near it (its reddest color has under half of its light in red, and a test checks it), so this only catches mistakes.

## 8. What it doesn't see

- **Flicker much finer than a cell.** A single dot of five pixels flashing, or a fine checkerboard whose squares swap light and dark (the cell's mean stays the same), passes unseen. Scenes must not draw such patterns.
- **Stripes.** The five-pair rule is not measured.
- **The screen itself.** The limiter works on the numbers sent to the screen. How bright that is in the room depends on the monitor and its settings. HDR is out of scope.
- **Resizing.** The limiter starts over from black at a new window size. It then counts the first frame as a transition wherever it is bright, so it errs toward holding, but it forgets the second before the resize.
- **Stricter than WCAG on one point:** WCAG counts a change only when the darker side is below 0.8; Juglar counts changes between two very bright states too.

## 9. Tests and cost

The limiter is tested twice. The cell logic, in Rust, is fed square-wave strobes from 3 to 30 flashes per second and must show at most four transitions in any second; a flash per second, a small 10 Hz flicker (0.08) and a slow full-range swell must pass unchanged. Then the real shaders run on the GPU: four adversarial scenes (the whole screen at 30 Hz, half the screen at 10 Hz, a checkerboard of blocks flashing out of phase at 12 Hz, a slow fade broken by a 20 Hz flicker) go through the limiter, the screen is read back, and an independent counter (the longest chain of alternating swings of 0.1 within any second, knowing nothing of cells) checks every 8×8-pixel block and the whole screen. A gentle scene must reach the screen unchanged except for dithering, and saturated red never. With the limiter switched off in the shader, the same test sees 59 transitions in a second.

On the development PC (GTX 1060, DirectX 12, release build, music playing, 75 Hz display), the limiter costs:

| | 960×540 window | maximized, 1920×1009 |
|---|---|---|
| GPU time per frame, before | 0.096 ms | 0.318 ms |
| GPU time per frame, with the limiter | 0.163 ms | 0.529 ms |
| Whole process CPU, before | 6.05% of one core | 5.99% |
| Whole process CPU, with the limiter | 6.75–6.79% | 6.65–6.68% |

The GPU time rises by 70%, to 4% of a frame at full size: the limiter reads and writes every pixel twice more. The CPU cost rises by about 0.7 points (two more passes to record and submit each frame, about 0.09 ms). Two runs of 15 s per case, both shown where they differ. The "before" figures were measured in the same session; they are slightly higher than note 05's 5.8%, from the same build, so small differences between sessions are normal.

## Further reading

- W3C, *Understanding WCAG 2.2, Success Criterion 2.3.1: Three Flashes or Below Threshold*: what counts as a flash, and the current red-flash definition.
- ITU-R, *Recommendation BT.1702-3* (2023): the broadcast guidance, including patterns.
- Juglar's `docs/research/focus-and-relaxation.md`, sections 7 and 9: where the numbers above come from, and what Juglar must not claim.
