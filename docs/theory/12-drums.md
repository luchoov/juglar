# 12 — The drums apart

Until now Juglar heard one kind of hit: `beat`, an onset in the low or middle range (note 07). Picture the scenes in layers instead: a background that breathes with the chords, a pulse on every kick, an accent on every snare, a grain of sparks with the cymbals. Each layer needs its own signal. This note explains how Juglar gets them, in `crates/dsp/src/percussion.rs`:

- `kick`, `snare` and `cymbals`: each drum's hits, apart from the others;
- `percussive` and `harmonic`: how loud the hits are and how loud everything held is;
- `relative`: each level against its own last few seconds.

Metal comes first, because it is the hardest case. Distorted guitars fill the whole spectrum all the time, the bass guitar plays the same notes as the kick, a double kick reaches 16 hits a second, and a blast beat alternates kick and snare even faster. Decisions D45 to D47 hold the reasons and the measurements.

## 1. Held and struck: percussive and harmonic

A guitar chord holds its level in each frequency band for a while. A drum hit jumps above that level for a few hundredths of a second, then falls back. So, every hop (10.7 ms, on the onset detector's transform), each band's **usual level** is taken as the median of its last 0.2 s: about 19 values, sorted, the middle one. A hit is too short to move a median, and a held chord is its own median.

Whatever a band has above its usual level is **percussive**; the rest is **harmonic**. Splitting a sound this way, with medians, is a standard method (see Further reading); the textbook version also looks ahead in time and across frequency. Juglar's only looks back, so it needs no future: the percussive part is known as soon as the hit is, and only the harmonic part trails, by about half the median's reach.

Both parts are powers. `Signals::percussive` and `harmonic` share one automatic gain, as the spectrum bars do. So the louder of the two can reach 1, and the other shows how far below it is: a guitar alone is mostly harmonic (in the tests, 64% of its power, the rest being its picks), drums alone are 98% percussive.

## 2. Where each drum is

Each drum is looked for in its own **region** of the spectrum:

| Drum | Region | What sounds there |
|---|---|---|
| kick | 40–120 Hz | the thump (and, in metal, the bass guitar) |
| snare | 165–2500 Hz | the body and the crack |
| cymbals | 2.5–16 kHz | hi-hats, rides, crashes, chinas |

In each region the **flux** is computed as in note 07: how much each band got louder than two hops ago, in decibels, divided by how much that band jumps around in steady noise, averaged over the region's bands. One thing changes. A rise counts only from the band's usual level up. Tremolo picking dips a chord sixteen times a second, and climbing back out of each dip is not a hit.

## 3. Thresholds that survive a double kick

The onset detector's threshold is the last second's mean flux plus three deviations. A double kick at 16 hits a second breaks it: the hits lift the mean and the deviation above every hit, and the detector goes quiet exactly when the drummer is busiest.

The kick and the cymbals use the **median** plus four **median absolute deviations** instead. Even the fastest double kick leaves more than half the hops between hits, so the median stays down there, among the quiet hops.

The snare has the opposite problem. Tremolo picking puts sixteen small attacks a second in its region, and a threshold that bends to dense trains would fire on every one. Its threshold is 1.2 times the **typical peak** of the last second: the median of the flux's local peaks. With only picking, that is the picking itself, and a snare has to stand above it.

Every threshold has a floor, high enough that 30 seconds of loud white noise fires nothing.

## 4. Telling the drums apart

Every drum spills into the others' regions. A hit only counts when it isn't the spill of a louder drum:

- **Kick against snare.** A kick doesn't count when the snare's region rose, at the same moment, more than two thirds as much as the kick's (in decibels). A snare doesn't count when the kick's region rose more than 1.7 times as much as the snare's.
- **The kick's click.** In a loud mix the bass guitar fills the kick's region too, and each new bass note looks like a kick. So a kick also needs its *click*, the beater's tick at 2.5–6 kHz, unless its thump is unmistakable on its own. The other way round, a sharp click with no snare and no cymbal in it is a kick even without a thump. In dense metal the click is often all of the kick that gets through.
- **Cymbals are bright.** A kick's click and a snare's crack also land above 2.5 kHz, but mostly below 6 kHz; a cymbal reaches 16 kHz. So a cymbal counts only when its percussive power above 6 kHz, against its power at 2.5–6 kHz, is at least what the harmonic part has in the same two places, averaged over the last second. Measuring against the music itself makes the test the same for a bright master and a dull one.

## 5. Events: short, and fast to fall

Each drum's event is 1 at a hit and falls back to 0 at a constant rate, with a time constant of 0.04 s for the kick and 0.05 s for the snare and cymbals. `beat` falls over 0.15 s, which is fine for a pulse but would merge a double kick into one long glow. Between two kicks 62 ms apart, the kick's event falls below a quarter. A drum can fire again after 40 ms (the onset detector waits 100 ms, a blast beat's hits come every 68 ms). As with onsets, a hit is confirmed one hop later, so it arrives 10.7 ms late.

## 6. Relative levels

The levels are leveled to the song's own peaks: in loud music they sit near 0.9 whether the band is playing its quietest riff or its heaviest. `Signals::relative` says whether something is *more* than it was. Each level (bass, mid, treble, energy, percussive, harmonic), as a linear amplitude, smoothed like its meter, is divided by its own average over the last four seconds. 1 is usual, 1.3 stands out, 0.5 has dropped to half. In silence the average waits, so music coming back after a pause is compared with how it was before.

## 7. How well it hears

**Synthetic tests.** Kicks are a falling tone with a beater click, snares a body plus a crack around 1.2 kHz (in real mixes a snare's hit rises most below 2.5 kHz), the guitar a tremolo-picked power chord through hard saturation, with a bass and a mastering limiter.

| Test | Result |
|---|---|
| kick, snare, hi-hats, 120 per minute | every hit found, each only in its own event |
| the same under the guitar, limited | snares 6 of 7, none from the guitar; every kick and hi-hat, plus 10 kicks from bass notes and 6 cymbals; harmonic energy rises |
| the guitar alone, 128 picks | at most 2 snares |
| double kick at 17 per second under the guitar | 89 of 92, none extra; the event falls below a third between kicks |
| blast beat at 220 per minute under the guitar | snares 38 of 40, cymbals 40 of 40 (3 extra), kicks 13 of 40 (and 15 extra, 9 of them on snares) |
| 2 s of silence, then 30 s of white noise | nothing |

**Real music, against the drummer's own track.** Isolated drum tracks of three songs (studio stems shared online) were aligned with Spotify's mix by cross-correlating their onsets. A plain detector on the isolated drums gave the reference hits; each hit in the mix counts if it is within 35 ms of a reference hit of the same drum. Recall is the share of reference hits found, precision the share of found hits that are real, F the balance of both (0 to 100). Each drum is compared with the onset detector region nearest to it:

| | Kick R / P / F | Snare R / P / F | Cymbals R / P / F |
|---|---|---|---|
| onset regions (low, middle, high) | 1% / 87% / 3 | 32% / 62% / 43 | 42% / 90% / 57 |
| this note | 35% / 80% / 48 | 45% / 58% / 51 | 50% / 84% / 63 |

(Amon Amarth, Behemoth and Gojira; a minute of each, 1,400 kicks.) The onset detector's low region found almost no kicks at all in this music.

**Played live**, 30 s of each, hits per second:

| Song | Kick | Snare | Cymbals | `beat` |
|---|---|---|---|---|
| Morbid Angel, *Chapel of Ghouls* | 7.1 | 1.5 | 7.9 | 1.8 |
| Behemoth, *Slaves Shall Serve* (blast beats) | 2.8 | 4.6 | 3.9 | 2.7 |
| Dark Funeral, *The Secrets of the Black Arts* (blast beats) | 1.4 | 2.0 | 7.1 | 1.0 |
| Slayer, *Angel of Death* | 1.4 | 4.6 | 4.7 | 3.6 |
| Amon Amarth, *Twilight of the Thunder God* (double kick) | 5.5 | 2.2 | 8.8 | 1.4 |
| Gojira, *Flying Whales* | 4.1 | 3.7 | 2.2 | 2.9 |
| Electric Wizard, *Funeralopolis* (doom) | 0.3 | 1.6 | 1.5 | 0.6 |
| Parkway Drive, *Carrion* (metalcore) | 2.1 | 1.6 | 3.2 | 1.6 |

On the same excerpts, the drummers' tracks have 12.4 kicks a second (Amon Amarth), 6.4 (Behemoth) and 5.8 (Gojira).

**Cost.** Over a minute of recorded music, the whole analysis went from 0.059 to 0.073 ms per frame at 60 frames a second, from 0.35% to 0.44% of one core. Live, in the app's own stats, from 0.082 to 0.100 ms.

## 8. What it can't do

- **Masked drums.** In a loud master, about half of each drum is buried under the guitars and the other drums, and no rule here brings it back. The kick suffers most: about a third of the real kicks are found. Four in five found kicks are real, so the pulse layer misses beats more often than it invents them.
- **Blast beats.** Kicks between snares, under hi-hats and tremolo-picked bass are the weak spot: in the synthetic blast a third of the kicks are found, and a quarter of the snares pass for kicks. Dark Funeral's blasts gave 1.4 kicks a second.
- **Kick and snare together.** The guard that keeps the kick's thump from passing for a snare also hides a snare struck with a kick, as in a hammer blast.
- **A bass note is a kick.** A new bass note with a pick attack has a thump and a click, like a kick: in the synthetic riff, whose notes change on the beat, 10 extra kicks in 8 seconds.
- **A drum alone over silence.** The faintest spill into a region that holds nothing is a huge rise in decibels, so a double kick alone fires snares. Real recordings always have something underneath.
- **The tempo is unchanged.** It still counts from the onset detector's flux (note 11); feeding it the percussive flux instead was not tried.

## Further reading

- Derry FitzGerald, "Harmonic/Percussive Separation using Median Filtering", DAFx 2010: the split this note's first section makes causal.
- Jonathan Driedger, Meinard Müller and Sascha Disch, "Extending Harmonic-Percussive Separation of Audio Signals", ISMIR 2014: a margin between the two, and a third part, the residual.
- Chih-Wei Wu et al., "A Review of Automatic Drum Transcription", IEEE/ACM Transactions on Audio, Speech, and Language Processing 26 (2018): how kick, snare and hi-hat are told apart, mostly with learned models this note avoids.
- Sebastian Böck and Gerhard Widmer, "Maximum Filter Vibrato Suppression for Onset Detection", DAFx 2013: rises measured against more than the previous frame, the idea behind counting rises from the usual level.
