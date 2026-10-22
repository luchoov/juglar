# 10 — Six scenes, six ideas

Milestone 4 had one scene, *Tide*. The feedback loop (note 05) and the palettes (note 06) can do much more than one calm ring, so before the generator of Milestone 6 starts combining building blocks, a few hand-made scenes try ideas that are deliberately different from each other: in what they draw, how the picture moves, and their colors. The left and right arrow keys switch between them; `JUGLAR_SCENE=oil` (or any name) picks the first one. The code is in `crates/render/src/scene/`, one `.rs` and one `.wgsl` file per scene.

They are sketches, tuned by eye on one evening of music. Each one passes through the flash limiter like everything else (note 09), and with music it held back nothing in any of them (0.0% of the screen over 18 to 22 s each, in their final versions).

## What a scene is

Every scene runs on the same two steps (note 05): the **feedback** step warps and fades last frame's picture and lets the scene paint into it faintly, and the **composite** step turns the result into colors and lets the scene paint sharp shapes on top. A scene supplies:

- `sources`: what it paints into the trails;
- `overlay`: what it paints sharply on top;
- `warp`: a chance to change where each point's old paint comes from, after the usual zoom and turn. *Tide* and *Orbits* leave it alone; *Kaleido* folds it into mirror images; *Oil* adds a current.
- each frame, a **look**: the zoom, turn and fade, eight numbers its shader reads ("knobs"), and its palette.

## Looping palettes

*Tide*'s palette is a path from sand to blue: positions below 0 or above 1 stop at the ends. The new scenes use palettes that come back to where they started, so a position of 1.3 is the same color as 0.3, and the colors can keep turning forever. With ink that walks along the palette as it ages, the picture's history turns into rainbow bands.

A cosine palette (note 06) loops when each channel makes a whole number of swings. The three new ones were found by a small search: as colorful as possible (the largest average gap between a color's strongest and weakest channel) while no color carries more than 73% of its light in red. The research's limit is 75% (note 09), and a test checks every scene's palette.

## Orbits: long exposure

Nine soft lights travel on tilted elliptical orbits, three per ring. Nothing warps the picture; it only fades, slowly, so each light leaves an arc behind it, like a photograph of car lights at night. The inner ring follows the bass, the middle one the mids, the outer one the treble: louder means faster and brighter. A beat pushes all of them ahead for a moment. A new section (novelty) turns the palette faster.

## Kaleido: a mirror and a rainbow

The spectrum, smoothed, draws a six-petal rose: each petal is the spectrum read from bass in the middle to treble at the edges, so the rose's shape is what the music sounds like. Every frame, the old picture is zoomed outward, turned, and **folded**: each point takes its old paint from the matching point in one 60° wedge, reflected about the wedge's middle. After a few frames everything is mirrored six ways, like the inside of a kaleidoscope. Echoes walk around the looping palette as they age (0.6 of the palette per second), so they fly outward in rainbow bands.

Folding keeps a point's distance from the center and lands near the horizontal axis. The screen's corners are further out than its sides, so they would fold to points off the image and stay black; they read from the side's edge instead.

## Oil: a liquid light show

In the 1960s, light shows for concerts were made by projecting colored oil and water swirled on a glass plate. *Oil* imitates that: four soft blobs of color well up (one each for bass, mids, treble and the overall level), and a current drags their color into marbled streaks. Nothing is sharp.

The current is the interesting part. It comes from a smooth **noise field**, a landscape of gentle hills that changes slowly. Moving downhill would gather all the color in the valleys. Instead, each point moves *along* the hillside, following a contour line, which is the gradient turned a quarter turn (its **curl**). Water flowing like that never piles up nor leaves holes, which is why it looks like a stirred liquid. Energy and beats make the current stronger; a change of section makes the landscape itself change faster.

## Plasma: color everywhere

The "plasma" of 1990s demos: no shapes at all, only color. Four waves are added up at every point, two straight and two circling around centers that wander, and the sum picks a color from a looping palette of hot pink, lavender, aqua and teal. Each frame lays a thin coat of that over the old picture, which the warp turns and swirls, so the waves smear into each other like dye in water. The bass makes the waves travel faster, a beat pushes them, the overall level sets how bright the coat is.

Waves of very different colors passing over a point are a flash risk: pink and teal differ in brightness. The waves are wide (about two across the screen) and slow: even at full intensity, with loud bass and a beat, under 0.7 waves pass a point each second.

## Harmonograph: a drawing by pendulums

A Victorian instrument: one pendulum swings the pen, another the paper, and the pen draws loops that slowly shrink as the swings die down. Each direction is the sum of two swings at a simple ratio of frequencies (2:3, 3:4, 1:2…), slightly out of tune, so the figure turns and folds as if seen in three dimensions. The mids make the tuning drift faster, the bass makes the figure bigger, and a change of section crossfades to the next ratio over three seconds (at most every 12 s, at least every 40).

The figure is 128 points computed on the CPU and sent in the slots the waveform normally uses. Every pixel measures its distance to all 127 segments, which is the most expensive drawing in Juglar: drawn at full size on top of the trails as well, it took 1.2 ms of GPU time per frame. Drawn only into the half-size trails it takes 0.41 ms, and the bloom gives the soft line its glow back. The first version drew nine turns of the figure: the strands crowded the middle, the line looked like a polygon, and the limiter held 0.5% of the screen where many strands crossed back and forth. Four turns, drifting more slowly, hold nothing.

## Why nothing here flashes

The lesson of note 09 applies to every scene: a bright, thin shape that trembles makes the pixels along it blink. So the levels that move the lights, the rose and the blobs are smoothed in three stages, like *Tide*'s waveform, and beats push motion (speed, zoom, current) rather than brightness. Colors turn slowly: a looping palette that cycled several times a second would flash.

The research's settings for focus and relaxation (slow motion, low saturation) are not what these sketches are for. They remain the target for those modes, which arrive with the settings in Milestone 7.

| Scene | GPU time per frame (960×540, with music, with bloom, one run of 20 to 22 s) |
|---|---|
| Tide | 0.222 ms |
| Orbits | 0.256 ms |
| Kaleido | 0.245 ms |
| Oil | 0.247 ms |
| Plasma | 0.237 ms |
| Harmonograph | 0.412 ms |

## Further reading

- Ryan Geiss, the MilkDrop preset authoring guide: per-pixel warps and kaleidoscopes, as a visualizer's vocabulary.
- Robert Bridson, Jim Hourihan and Marcus Nordenstam, "Curl-Noise for Procedural Fluid Flow", SIGGRAPH 2007: why the curl of a noise field moves like a liquid.
- Inigo Quilez, "Palettes" (iquilezles.org): cosine palettes, looping and not.
- Anthony Ashton, *Harmonograph: A Visual Guide to the Mathematics of Music* (2003): the instrument, and why simple ratios draw closed figures.
