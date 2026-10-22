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

## Julia: a fractal, one step per frame

A Julia set comes from one rule: take a point z of the plane, square it and add a fixed number c (both as complex numbers, so squaring doubles the angle and squares the distance from the middle), and repeat. Some points run off to infinity, others stay forever; the border between them is the fractal. The usual way to draw it repeats the rule a few hundred times for every pixel, every frame.

*Julia* doesn't. Its warp *is* the rule: each pixel takes its old paint from z² + c. Points whose next step leaves the picture get fresh paint. A point one step from leaving shows that paint one frame later, a point two steps away two frames later, and so on: after a few dozen frames, every pixel shows how many steps its point needs to escape, computed by the loop itself, one step per frame, for the price of one sample. Paint moves along the palette with every step, so the steps come out as bands of color, and points that never escape stay black: the filled-in Julia set.

Anything else painted into the picture is copied back through the steps too. A ring drawn by the waveform, outside the fractal, comes back as nested, ever smaller and more crinkled copies of itself hugging the fractal's edge.

The music moves c, slowly, around a circle where the most intricate Julia sets lie (near the edge of the Mandelbrot set, which maps them all). The mids and beats speed it up, the bass moves it in and out, a change of section sends it further. Each new c reshapes the whole fractal, and the bands flow as it changes.

The fade and the color change here are per frame, not per second, since each frame is one step. At a lower frame rate the picture looks the same and settles more slowly.

Near the fractal's edge, bands get thinner than a pixel and move fast when c moves. Three things keep that from flickering: c moves slowly and smoothly; the paint fades with every step, so the deepest steps, where bands are thinnest, are dim; and the palette was chosen for colors of similar brightness (linear luminance 0.11 to 0.41), so bands sliding past a point change its hue more than its light. A first version also drew the ring inside the fractal, where its paint circulated through the black interior: beautiful, but it shimmered six to twenty times more (see "Checking for flicker finer than a cell" below).

## Crystal: a fractal of mirrors

*Crystal* uses the same trick as *Julia* with a different step. Each frame, the old picture is turned, folded into five mirrored wedges (as in *Kaleido*), moved out from the middle and shrunk to about half its size. Read backwards, as the warp is (note 05), that puts a small copy of the whole old picture in each of five places around a ring. The next frame copies those copies, and so on: a soft ring drawn by the waveform in the middle comes back as five rings, then twenty-five smaller ones, a hundred and twenty-five smaller still, a snowflake of rings that is a fractal. Mathematicians call a set of shrinking copies like this an *iterated function system*; here the loop runs it, one generation per frame.

Each generation is fainter than the one before (0.8 of its ink) and a step further along the palette, so the crystal is colored by generation: the newest ring in the middle, the oldest copies, smallest and darkest, at the rim. The mids and beats twist the fold, which turns every generation by a little more than the one inside it, so the crystal twists into spirals and back; the bass pushes the copies out and in, and the crystal breathes.

Small copies of a moving shape are where flicker finer than a cell would hide. So the ring is soft and wide, the fold changes slowly, and the generations fade fast: with them fading to 0.86 instead of 0.8, the deeper copies filled the crystal with a carpet of tiny rings and the check below counted four times more.

## Vortex: rings that turn against each other

A tunnel, like *Tide*'s, with two changes to its warp. Every ring of the picture turns by its own amount, set by two slow waves across the distance from the middle: some rings turn one way, others the other, and the pattern drifts. And a ripple of zoom travels outward, pushing each ring a little in, then out. Five lights circle the middle, each one as big as a fifth of the spectrum is loud and each its own color; the tunnel carries their trails outward, so they draw five spiral arms, and the counter-turning rings kink and bend them. A small oscilloscope line turns the other way at the center. Beats surge the flow and strengthen the ripple, the treble spins the lights faster, the mids turn the rings harder.

Five arms and not more: regular stripes of more than five light and dark pairs are on the research's list of patterns to avoid (note 09).

Rings turning against each other stretch whatever lies across them into thin filaments, the way stirring stretches cream in coffee. The first version turned them four times harder and kept the trails longer: the arms were torn into thin bright threads that slid across the pixels, and with music the limiter held up to 0.3% of the screen along them. Turning the rings a third as hard and fading the trails sooner leaves the arms whole, and the limiter holds nothing.

## Corona: flames around an eclipse

A dark disc, its rim a thin steady line, and around it a ring that gives off paint: each point of the ring as much as its band of the spectrum is loud, bass at the top and treble at the bottom, mirrored left to right like *Tide*'s ring. The warp carries the paint outward, as in a tunnel, and stirs it with turbulence, the curl of a churning noise field like *Oil*'s current, so the paint rises in tongues that curl and split, like the sun's corona seen in an eclipse. Along the ring, a slowly churning pattern of noise makes some angles give off more than others, so the flames come in tongues rather than as an even halo. The paint cools along the palette as it rises, from peach gold through pink and violet to blue, and the whole palette turns slowly, faster after a change of section. The bass makes the flames rise faster, beats surge them, the energy stirs them harder.

Real flames flicker several times a second, which on a screen would be flashing. These follow the spectrum smoothed in three stages and a turbulence that changes over seconds: they lick, but don't flicker.

## Melt: paint running down

The spectrum hangs as a band of color across the top of the picture, each point as bright as its band, bass in the middle and treble at the sides, mirrored left to right; the waveform draws a thin line along it. Nothing here zooms or turns. The warp pulls paint *down*, at a speed that changes from column to column (two layers of noise across the width, drifting), and sways the columns sideways, mirrored. So the colors run in long, wavering drips, like wax down a candle or wet paint down a wall, and change color as they run. The mirror makes a moving Rorschach blot. The bass and beats make the paint run faster, the mids sway the columns harder, a change of section reshuffles which columns run fast.

Two choices keep it calm where it could flash. The band follows the spectrum smoothed in three stages of 0.3 s, which leaves under a fiftieth of a kick's swing: a band that pulsed with every kick would make every point of every drip below it pulse too. And it is one continuous band: the first version hung a row of 64 separate drops, which painted a comb of regular stripes at the top, the kind of pattern the research warns about.

## Why nothing here flashes

The lesson of note 09 applies to every scene: a bright, thin shape that trembles makes the pixels along it blink. So the levels that move the lights, the rose and the blobs are smoothed in three stages, like *Tide*'s waveform, and beats push motion (speed, zoom, current) rather than brightness. Colors turn slowly: a looping palette that cycled several times a second would flash.

The research's settings for focus and relaxation (slow motion, low saturation) are not what these sketches are for. They remain the target for those modes, which arrive with the settings in Milestone 7.

## Checking for flicker finer than a cell

The limiter watches cells of 8×8 pixels at 1080 rows (4×4 in a 960×540 window), and flicker much finer than that, a dot or a thin line blinking, changes a cell's mean too little to be seen (note 09). Scenes must not draw it, and with fractals, folds and fine bands that is easy to get wrong. So the new scenes were checked from outside: a script captured the window as fast as it could (about 45 frames per second) for 15 seconds, and counted transitions with the limiter's own rule (swings of 0.1 back from the last extreme, at most four in any second) in every block of 2×2 pixels, and again in every limiter cell.

It counts too much rather than too little: a thin line that *moves* across a block counts the same as one that blinks in place, so every scene with sharp lines in motion scores above zero. Its numbers are for comparing scenes, not a verdict. The capture tool is not part of Juglar; it lives with the other measuring scripts outside the repository.

| Scene | 2×2 blocks ever breaking the rule, music | at the synthetic drop | cells, music / drop |
|---|---|---|---|
| Tide | 0.07% | 0.12% | 0.000% / 0.000% |
| Orbits | 0.000% | 0.000% | 0.000% / 0.000% |
| Kaleido | 2.8% | 1.1% | 0.29% / 0.24% |
| Oil | 0.002% | 0.000% | 0.000% / 0.000% |
| Plasma | 0.000% | 0.000% | 0.000% / 0.000% |
| Harmonograph | 0.95% | 0.77% | 0.17% / 0.14% |
| Julia | 0.47% | 0.16% | 0.05% / 0.006% |
| Crystal | 0.38% | 0.000% | 0.08% / 0.000% |
| Vortex | 0.08–0.12% | 0.002% | 0.03–0.04% / 0.000% |
| Corona | 0.000% | 0.000% | 0.000% / 0.000% |
| Melt | 0.002% | 0.000% | 0.000% / 0.000% |

The scenes with thin moving lines (Kaleido's rose, Harmonograph's figure, Julia's ring and its echoes) score highest, the soft ones nothing. The new scenes were tuned to stay at or below the old ones. The cell column should read zero, since the limiter never lets a cell break the rule; the few cells counted are most likely the capture's doing (it misses about two frames in five, and sees the screen after dithering and Windows' compositor), but this hasn't been proved.

## What they cost

GPU time per frame on the development PC (GTX 1060, release build, music playing, with bloom). The first six were measured once, over 20 to 22 s; the newer ones twice, over 15 s each, the range shown. Every scene costs the CPU the same (handing a frame to the driver costs the same whatever it holds): 7.8% of one core for *Tide* and for every new scene alike, measured in the same session.

| Scene | 960×540 | maximized, 1920×1009 |
|---|---|---|
| Tide | 0.222 ms | 0.722 ms |
| Orbits | 0.256 ms | |
| Kaleido | 0.245 ms | |
| Oil | 0.247 ms | |
| Plasma | 0.237 ms | |
| Harmonograph | 0.412 ms | |
| Julia | 0.242–0.250 ms | 0.796 ms |
| Crystal | 0.243–0.249 ms | 0.804 ms |
| Vortex | 0.245–0.249 ms | 0.830 ms |
| Corona | 0.260–0.264 ms | 0.926 ms |
| Melt | 0.241–0.243 ms | 0.829 ms |

## Further reading

- Ryan Geiss, the MilkDrop preset authoring guide: per-pixel warps and kaleidoscopes, as a visualizer's vocabulary.
- Robert Bridson, Jim Hourihan and Marcus Nordenstam, "Curl-Noise for Procedural Fluid Flow", SIGGRAPH 2007: why the curl of a noise field moves like a liquid.
- Inigo Quilez, "Palettes" (iquilezles.org): cosine palettes, looping and not.
- Anthony Ashton, *Harmonograph: A Visual Guide to the Mathematics of Music* (2003): the instrument, and why simple ratios draw closed figures.
