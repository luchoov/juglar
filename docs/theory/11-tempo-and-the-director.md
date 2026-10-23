# 11 — Tempo, tension and the director

Until now Juglar reacted: a kick came, the picture moved. A musician, or someone running visuals at a club, does more. They know when the next beat will come and keep counting through a bar the drums leave out. They feel a build-up pulling toward its drop and wait for it. They change the picture when the music turns a corner, and pick a picture that suits what comes next. This note explains how Juglar does each of those:

- the tempo and where the beat falls, in `crates/dsp/src/tempo.rs`;
- slow summaries of the music, tension among them, in `crates/director/src/feel.rs`;
- the decisions, in the rest of `crates/director`.

Decisions D36 to D44 hold the reasons and the measurements.

## 1. Counting along: the period

The onset detector (note 07) gives the **spectral flux** every 10.7 ms (a hop): how much energy just appeared, in three regions. For the tempo the regions are added, the low and middle in full and the high at half: hi-hats help count the beat, but shouldn't decide where it falls.

Minus its average over the last second, the flux is a train of bumps, one per hit. Slide a copy of it along itself: at a lag of one beat, bumps land on bumps; at half a beat, a kick lands on nothing. Multiplying the train with its slid copy and averaging gives, for each lag, the **autocorrelation**. Juglar keeps it as a running average over about six seconds, one number per lag, updated every hop: about 190 multiplications.

The beat is a peak of the autocorrelation, and so are its multiples. Three details pick the right one:

- **Each bump is spread over three hops first.** A beat rarely lasts a whole number of hops: at 150 per minute it is 37.5. A bump one hop wide then lines up with the next one only every other beat, and the double period wins. A test at 150 per minute found 75 before this.
- **Each lag scores its own value plus half the value at twice the lag.** A real beat lines up again one beat later.
- **A weight toward 120 per minute** (falling off by one octave, one standard deviation) picks among the peaks: the double and the half of a tempo line up too, and most music people tap along to sits within an octave of 120. The weight only picks. The unweighted score places the chosen peak between hops, through a parabola. Weighted, a broad peak leans toward 120: over a minute of black metal, the tempo crept from 134 to 118.

Every quarter of a second the best lag is found again. Within 5% of the current period, it refines it; a different period must win for two seconds in a row to replace it, so a fill or a break doesn't flip the tempo.

The **confidence** is the autocorrelation at the period divided by its value at lag zero: near 1 for a metronome, near 0 for noise. On the tests, clicks give 0.74–0.88 and loud hiss 0.07. Below 0.15 the tempo is reported as unknown: `Signals::tempo` is 0.

## 2. Where the beat falls: the phase

A counter goes once round a circle per beat, at the period found. Each hop's flux is added into one of 48 bins around the circle, wherever the counter is, and older additions fade over four seconds. Wherever in the beat the music hits, a bump grows in the ring. Its center of mass, of the squared bins so the beat outweighs the hi-hats between beats, is where the beat is. `Signals::beat_phase` is 0 there and rises to 1 just before the next beat.

The counter keeps going whether or not the music hits. Through a bar with no kick and no snare, the count carries on, on time. The director uses that to land a change on a beat the music leaves out, or one the detector misses in a dense mix.

## 3. How well it counts

Tests use synthetic clicks and kicks: tempos of 90, 128 and 150 found within 2% after 14 seconds, with the phase within a tenth of a beat of every click; kicks at 100 per minute with hi-hats between them read 100, with the beat on the kicks; a change from 100 to 140 followed; a minute at 165 stays within 1% after 12 seconds; noise and silence have no tempo.

To check the whole chain, a drum file was played through the speakers and captured like any music:

| Pattern | Read | Detected beats within a tenth of a beat of the count |
|---|---|---|
| rock, 100 per minute | 100, every second after the first few | all (17 of 17 per ten seconds) |
| four on the floor with bass, 128 | 128 | all (22 of 22) |
| breakbeat, 174 | 87 | half: the snares fall between counted beats |

87 for a breakbeat at 174 is the octave many listeners tap for that style; nothing in the sound says which is right.

On real music there is no known answer to compare with. On fast black metal (Marduk), the tempo held around 130, and a third of the detected beats fell within a tenth of a beat of the count, against a fifth by chance; on a later run, after the fix for the creeping tempo, with Marduk and then death metal (Bolt Thrower), 40%. In that music the detector finds onsets everywhere, about five a second, so it counts along, but loosely. The analysis' cost didn't move: its median stayed at 0.080–0.083 ms per frame.

## 4. Slow summaries: the feel

The director doesn't look at single frames. `Listener` keeps the **feel** of the last several seconds:

- **pulse**: beats a second, counted with a three-second memory, as a share of four;
- **loudness**: rising over 1.5 s, falling over 5 s;
- **brightness**: the treble over 3 s;
- **swell**: loudness against the last half minute, 0.5 when steady;
- **rise**: the one-second average of loudness against the eight-second one.

Loudness alone says little. The analysis levels every song to its own peaks, so with music playing it sits near 0.9. **Natural intensity** (D37) is therefore made of the rest: half the pulse, three tenths the brightness, a fifth the swell, mapped to 0.15–0.95. It rises over about 2 s and falls over about 6 s. Intensity scales how fast and far the scenes move, never how bright they are.

## 5. Tension and the drop

Dance music builds up before a **drop**. For eight or sixteen bars a riser climbs, hi-hats get louder, a snare roll speeds up, and the kick and the bass line are held back. Then everything comes in at once.

**Tension** is the treble outgrowing the deep bass: the treble's one-second average above its six-second average, less however much the deep bass's does the same. It is scaled so that 0.06 is full. It rises over about two seconds and lets go in a third of one. The deep bass is the spectrum below about 120 Hz, the kick and the bass line. The first version used the `bass` level, which goes up to 250 Hz, where a snare's body is. Played through the speakers, the snare roll speeding up counted as the bass coming in, and the tension let go seven seconds before the drop.

The drop is a moment of its own: the deep bass **punching**, hitting harder than over the last six seconds, while the tension is above 0.4 or within 1.5 s after. Two things keep other hits out:

- **Half of what the middle does at the same time is taken off.** A snare has deep bass too, since it is noise, but it jumps as much in the middle; a kick barely touches the middle.
- **The punch must clear 0.3, and 1.25 times the build-up's hardest hit** from half a second to four seconds before. Some build-ups keep a kick.

The measured punches:

| Source | Snare roll or build-up's kicks | Drop's first kick |
|---|---|---|
| Played through the speakers | 0.13 at most (snare roll) | 0.62 |
| The synthetic song, whose build-up keeps a kick | 0.32–0.41 | 0.54 |

Novelty (note 07) can't be the drop's moment. In the file played through the speakers, it crossed its threshold during the build-up and stayed above it, so at the drop it had nothing left to jump with.

The rule's weak spot is a build-up that keeps its kick. In explorable 06, a build-up with a kick as hard as the drop's hits 0.47 against the drop's 0.48, and no drop is found. Even a soft kick reaches 0.41 against 0.49, because the analysis levels the spectrum to its loudest band. When no drop is found, the change waits for an ordinary moment after the tension lets go, as before.

## 6. When to change, to what, and how

The director changes scene at a **moment**:

- the analysis' novelty jumping (a new song, a section);
- the feel shifting: pulse and brightness in full and swell at half, against six seconds earlier, by more than 0.4;
- a drop.

Novelty alone wasn't enough. Over five minutes of dense metal, it jumped only when a song started, while within a song the feel shifted clearly twice.

Each scene shows at least 30 s and at most 100 s, moment or not. A drop may come after 15 s. During a build-up nothing changes: its other moments, such as the snare roll speeding up, are part of it, and a change due by time waits for the drop, at most 30 s more. A change waits up to 0.75 s to land on a beat, detected or counted.

What kind of moment it was decides the transition (note 10's scenes, D38's transitions). After an ordinary moment, the director listens for half a second: at a moment's first frame only novelty has moved, and half a second later the loudness has. A drop needs no listening, so its change lands on the drop's first beat. In the synthetic song that is 32.0 s, where it used to be 32.4–33.1.

| Moment | Transition |
|---|---|
| a drop | bloom from the middle, 1.5 s |
| the music grew suddenly | bloom, 2.5 s, pushed on by beats |
| it thinned out | iris, the old scene closing into the new, 4 s |
| it changed otherwise, or nothing happened | spiral, 3 s |
| anything, in quiet music | seep through soft patches, 6 s |

**To what:** each scene has a character, set by hand from how it moves:

- **energy**: how lively it looks;
- **pulse**: how much it moves with beats;
- **flow**: whether it moves the whole picture.

The director picks the scene closest to the music's liveliness and pulse, avoiding the one showing and the last four shown. The music's liveliness is stretched first: over 15 minutes of metal it stayed between 0.3 and 0.65, while the scenes spread from 0.2 to 0.9, and matched as it was, only calm scenes ever came up. Tuned on one genre, with the wildest scenes never chosen on real music so far, this is the part most in need of judging by eye.

## 7. Modes

**Focus** and **relax** (D41) are style options from `docs/research/focus-and-relaxation.md`, which found no evidence that visuals improve focus or calm, only what disturbs. So the modes are the absence of those things:

- motion slowed to 0.15 or 0.3, with no camera flow;
- colors muted, and the picture dimmed and low in contrast;
- scenes held for minutes and changed by a 15 or 20 s seep;
- the music followed only faintly, its beats never reaching the scenes.

Measured with optical flow on the screen, at the 90th percentile, features move 0.0009–0.0034 of the window's short side per second in focus and 0.0012–0.0084 in relax. The note's limits are 0.005 and 0.010; lively runs at 0.018–0.041.

## Further reading

- Daniel Ellis, "Beat Tracking by Dynamic Programming", *Journal of New Music Research* 36 (2007): onset strength, autocorrelation and a tempo prior centered near 120.
- Masataka Goto, "An Audio-based Real-time Beat Tracking System for Music With or Without Drum-sounds", *Journal of New Music Research* 30 (2001): counting through, and agents competing over the phase.
- Simon Dixon, "Automatic Extraction of Tempo and Beat from Expressive Performances", *Journal of New Music Research* 30 (2001).
- Karthik Yadati et al., "Detecting Drops in Electronic Dance Music", ISMIR 2014: what a drop is made of, and how hard it is to find from features alone.
- Explorable `06-tempo-and-the-drop.html`: the autocorrelation with each detail switchable, the phase ring counting through a bar with no drums, and a build-up and drop with every part of the tension and drop rule switchable.
