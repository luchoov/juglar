# 05 — The feedback loop

Milestone 3 drew what Juglar hears. Milestone 4 draws something to look at. The technique is the oldest trick in music visualizers: **every frame starts as a slightly changed copy of the previous one.** Draw one simple shape per frame, and the copies of its past selves pile up into trails, tunnels and spirals that nobody drew. This note explains how that loop works, why it is built the way it is, and what the first scene, *Tide*, does with it. The code is in `crates/render/src/` (`feedback.rs`, `feedback.wgsl`, `composite.wgsl`, `scene/`).

## 1. A camera pointed at its own screen

Point a video camera at the monitor showing its own picture, zoom in a little, tilt it a little, and the screen fills with an endless tunnel that twists and breathes. That is **video feedback**: the output becomes the next input. Physicists studied it in the 1980s as a simple machine that produces complex patterns. Ryan Geiss built the same loop into software for the Winamp visualizers *Geiss* (1998) and *MilkDrop* (2001), and nearly every music visualizer since uses it.

In software, the "camera" is a shader that reads last frame's picture and writes this frame's. Each frame does three things:

1. **Warp:** copy the old picture, slightly zoomed, turned and swirled.
2. **Fade:** multiply it by a number just under 1, so old paint slowly disappears.
3. **Paint:** draw this moment's shapes on top.

Step 1 moves the past, step 2 forgets it, step 3 adds the present. The balance between them sets the whole character of the picture.

## 2. The warp works backwards

The obvious way to zoom an image is to take every old pixel and move it outward. It doesn't work on a grid: spread out, the moved pixels leave holes between them, and squeezed together, several land on the same spot.

So the shader asks the opposite question, once for every **new** pixel: *where was my paint one frame ago?* To zoom in by 2%, each new pixel looks 2% closer to the center in the old picture. To turn the picture by an angle, it looks back by the opposite angle. Every new pixel gets exactly one answer, so there are no holes and no collisions.

The answer rarely lands on a pixel center. The GPU blends the four nearest old pixels (**bilinear filtering**), so the picture can move by a fraction of a pixel per frame and still move smoothly.

| Warp | What the pixel looks back at | What you see |
|---|---|---|
| **Zoom** | a point slightly closer to the center | paint flows outward, a tunnel |
| **Rotation** | a point turned back by a small angle | the whole picture spins |
| **Swirl** | a rotation that is strong at the center and fades to none at the edges | straight trails bend into spirals |
| **Center** | the warp's middle wanders slowly | the tunnel's mouth drifts, never static |

Nothing comes in from outside the old picture: a pixel that would look past its edge gets black.

## 3. Rates, not steps

Juglar draws 60, 72 or 75 frames per second, or 37 while a game runs (see note 04). If the warp zoomed by 2% *per frame*, the picture would move twice as fast at 75 fps as at 37. So every motion is set **per second** and converted to this frame's step using the time since the last frame:

- Zoom per frame = e^(zoom rate × frame time). Two frames of 1/60 s zoom exactly as much as one frame of 1/30 s.
- Fade per frame = e^(−frame time / persistence). The **persistence** is the time it takes paint to fall to about a third (37%) of its brightness.

A test checks that two short frames and one long frame give the same zoom.

## 4. Two images that take turns

A GPU can't read an image and write it in the same step: pixels would read neighbors that were already overwritten. So there are two images. Each frame reads one (the last picture) and writes the other; the next frame swaps them. This is called **ping-pong**.

Both images are **half the window's width and height**, a quarter of its pixels. The trails are soft anyway, and the feedback step is the one that reads and writes every pixel each frame. The sharp parts of the picture are drawn later at full resolution (section 7).

**Why 16-bit floats.** A screen stores 256 levels per color channel. In an image like that, fading breaks: a dim pixel at level 3, multiplied by 0.98, is 2.94, which rounds back to 3. It never reaches 0, and a ghost of every trail stays on screen forever. The feedback images store 16-bit floating-point numbers instead, which keep fading smoothly down to nothing and can hold values above 1 where paint piles up.

## 5. Ink, not color

The feedback images don't hold colors. Each pixel holds two numbers:

- **ink**: how much paint is there (0 is none; there is no upper limit);
- **ink × palette position**: which color that paint is, weighted by how much of it there is.

Colors are chosen only at the very end, in the composite step (note 06). This has three advantages:

- Fading, blending and warping treat both numbers alike, so where two trails of different colors overlap, the color in between is the ink-weighted average, and dividing the second number by the first recovers it.
- **Paint can age.** Each frame, old paint's palette position moves a little, so trails change color as they get older: in *Tide*, the present is warm and the past turns cool.
- The whole picture's colors can change at once (a calm section in blue, a loud one in sand) without repainting anything.

New paint goes **over** the old, the way a brush stroke covers what is under it: where the new paint covers 30% of a pixel, 70% of the old paint remains. Simply adding paint each frame would pile up without limit wherever a shape stays still.

## 6. Painting twice

A shape painted fully into the feedback every frame smears into a solid band: it is redrawn while its copy from last frame has moved by less than a pixel. The first version of *Tide* did exactly that, and its loud parts turned into fog that covered the screen.

So the scene paints each shape twice:

- **faintly into the feedback**, 5–11% of its ink, where the trails build up from many faint copies;
- **sharply on top of the final picture**, at full resolution, in the composite step.

The present moment stays crisp, in front of its own blurred history.

## 7. Tide, the first scene

*Tide* is one ring. Its edge is the waveform from the oscilloscope (note 04), wrapped around a circle and mirrored left to right, so its two ends meet and it looks like a symmetrical, living shape rather than a line bent into a loop. The warp carries its faint copies outward, and the tunnel they form is a record of the last second or so of sound.

| Signal | What it moves |
|---|---|
| bass | how fast the tunnel flows outward, the ring's size, and how much of the line the trails keep, so kicks leave echoes |
| mid | how fast the picture turns (it reverses slowly, every 80 seconds) |
| treble | the swirl that bends trails into spirals, and the line's thickness |
| energy | the height of the wave on the ring |
| *mood* | the energy smoothed over three seconds: how long trails last and how warm the colors are |

*Mood* exists because the louder parts of a song should *feel* different, not just twitch more. It changes over a section, not a beat.

The **intensity** (keys 1, 2, 3: gentle, medium, strong) scales every movement. The window's `Tab` key switches to the analysis view of Milestone 3, to see what Juglar is hearing.

Every number in *Tide* is a matter of taste, tuned by eye on real music and on the synthetic song (`JUGLAR_DEMO=1`, decision D20). They live in one place, `scene/tide.rs`, and the shapes in `scene/tide.wgsl`, so the scene can change without touching the loop. Milestone 6's generator will compose scenes from building blocks the same way.

## 8. Safety

The research note (`docs/research/focus-and-relaxation.md`) sets limits that apply in every mode. What M4 does about them:

- **Kicks move things instead of flashing them.** The bass speeds up the flow and grows the ring; it doesn't brighten the screen. The overall brightness follows *mood*, which changes over seconds.
- **No saturated red.** The palette's reddest color has under half of its light in red; the limit is 75%. A test checks the whole palette.
- **Few stripes.** Echoes fade within about a second, so at most a few rings are visible at once; the limit is five light/dark pairs.
- **Reduced motion.** When Windows' "Show animations" setting is off, the scene sees silence and no time passing: a still ring. Juglar checks the setting once a second, so turning it off applies right away. `JUGLAR_REDUCED_MOTION=1` or `0` overrides it.

What is **not** done yet: nothing measures the final picture. A limiter that counts actual flashes in what is about to be shown, and holds back a frame that would break the limit, comes with the beat detection in Milestone 5. Until then, these limits hold by design, not by measurement.

## 9. What it costs

Measured on the development PC (GTX 1060, Windows 11, DirectX 12), release build, real music, both monitors on. CPU is counted in processor cycles, which note 04 explains is the only reliable way for a program that works in short bursts locked to the display:

| | 960×540 window | maximized, 1920×1009 |
|---|---|---|
| GPU time per frame | 0.095 ms | 0.32 ms |
| Whole process CPU, 75 Hz display (75 fps) | 5.8% of one core | 5.8% of one core |
| Whole process CPU, 144 Hz display (72 fps) | 5.6% of one core | 5.5% of one core |
| Frame intervals at 75 fps | 12.8–13.8 ms (target 13.3) | 13.0–13.7 ms |

The GPU does three times more than in Milestone 3 and is still busy for under 3% of each frame at full size. The CPU cost didn't change and doesn't depend on the window's size: as note 04 found, it is the fixed cost of handing a frame to the driver, whatever the frame contains.

An earlier version of this table, measured with both monitors switched off and with Windows' tick-based CPU figure, gave 5.8–6.8% and warned that the CPU numbers might change with the monitors on. They didn't in any way that matters; the spread came from the measuring method (note 04, section 6).

## Further reading

- James P. Crutchfield, "Space-time dynamics in video feedback", *Physica D* 10 (1984): the camera-at-its-own-screen experiment, studied as physics.
- Ryan Geiss, the MilkDrop preset authoring guide (geisswerks.com): how per-frame warps and per-pixel motion became a visualizer's vocabulary.
- Explorable `03-feedback-and-color.html`: the loop and the palette, with the knobs in your hands.
