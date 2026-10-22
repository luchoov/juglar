# 07 — Onsets, beats and novelty

Until now Juglar knew how loud the bass, mids and treble were, moment by moment. It didn't know when something *happened*: a kick drum, a snare, a new note, or a drop that changes the whole song. Milestone 5 adds that. This note explains how Juglar finds the start of sounds (**onsets**), which of them count as **beats**, and how it notices that the music changed character (**novelty**). The code is in `crates/dsp/src/onset.rs`.

## 1. Loud is not the same as new

The obvious way to find a drum hit is to wait for the sound to get loud. It fails twice over. In a song with a thick wall of guitars, everything is always loud, and the kick drum barely changes the total. And a sustained note that swells slowly gets loud without anything starting.

What marks the start of a sound is **energy appearing where there was none a moment ago**. A kick drum adds a burst of low frequencies; a hi-hat a burst of high ones; a piano note a stack of harmonics. So Juglar looks at the spectrum (note 02), not the total, and asks of each part of it: *did this just get louder?*

## 2. Spectral flux

Every 10.7 ms (a **hop**), Juglar takes the newest 21 ms of sound, transforms it, and measures the level of about 18 bands in decibels, spaced like hearing: narrow in the bass, wide in the treble. For each band it subtracts the level two hops earlier. Rises count; falls count as zero, because a sound ending is not a sound starting. The rises, averaged over a group of bands, are the **spectral flux**.

Three details make it work in practice:

- **Decibels, not raw energy.** A rise from quiet to medium and from medium to loud weigh the same, as they do to the ear. A soft hi-hat in a quiet passage counts, not only the loudest drum.
- **Two hops back, not one.** Each 21 ms window overlaps the previous one by half, so an attack lands half in one window and half in the next. Comparing with two hops back compares two windows that don't overlap, and the whole attack shows up in one rise.
- **A rise is measured against how shaky its band is.** A band only a few frequency steps wide is a shaky measurement: in steady noise its level jumps around by several decibels from hop to hop. A wide band is steady. Measured, the jumps are 6 dB for a band one step wide, divided by the square root of its width. Juglar divides each band's rise by that number, so a rise is counted in "how unusual for this band". The narrow bass bands no longer drown everything in noise.

The flux is computed separately for three **regions**, the same split as the bass, mid and treble levels: low (under 250 Hz: kicks, bass notes), middle (250 Hz to 4 kHz: snares, voices, chords) and high (hi-hats, cymbals). Averaged over the whole spectrum, a kick drum fills only the lowest bands and gets diluted by everything else; in its own region it stands out.

## 3. When is a rise an onset? The adaptive threshold

Flux is never zero in real music; something is always moving a little. An onset is a hop whose flux stands out from that background:

1. **It is a peak**: higher than the hop before and no lower than the hop after. Knowing the hop after means waiting for it, so onsets are reported 10.7 ms late. Nobody can see that.
2. **It clears the threshold**: the average flux of the last second, plus three times its typical deviation. In busy music the background is high and so is the threshold; in a quiet ballad both are low. The same detector works on a solo piano and on a wall of drums, at any volume.
3. **It clears a floor.** In steady noise (rain, applause, tape hiss) the flux jumps around too, and once in a while a jump clears any threshold made from the noise's own average. The floor is set from the statistics of noise: the average rise of a band in noise plus 4.8 of its deviations, which shrink when a region averages more bands. In 30 seconds of loud white noise, no onset at all.
4. **It isn't too soon**: a region can't fire twice within 100 ms.

The band levels are measured relative to the loudest band, ignoring anything more than 45 dB below it, so the detector doesn't care how loud the music is played. The first version used a fixed floor of −60 dB, and on the development PC, where Windows' volume was low, it found no beats at all in real music.

## 4. Beats

Juglar doesn't track the tempo yet; that comes with Milestone 5.5, together with predicting where the next beat lands. For now a **beat** is an onset in the low or middle region: kicks and snares, what you tap your foot to. Hi-hats, often four to a beat, are found as onsets in the high region but don't make beats. A kick that shows up in both the low and middle regions is one beat, not two.

`beat` goes to 1 at a beat and falls back toward 0 with a time constant of 0.15 s, the same shape the synthetic song (D20) gives it.

## 5. Novelty

A drop, a chorus, a breakdown: the song changes character, and the change lasts. Juglar keeps two running averages of each band's level, one over half a second and one over four seconds. While the music keeps its character, they agree. When it changes, the short one moves first and the two disagree; how much, in decibels across all bands, is the **novelty**. It fades as the long average catches up with the new normal.

Measured on three 40-second stretches of real songs (doom metal, the developer's own playlist), the averages disagreed by under 1.5 dB nine tenths of the time and never by more than 3.4 dB. A synthetic drop (everything 14 dB louder, a bass note and kicks) reaches 4.6 dB after one second and 6 dB after two and a half. So 2.5 dB maps to novelty 0, and 6 dB to 1. Only what can be heard counts: bands more than 30 dB below the loudest are treated as being at that edge, or their swings in near-silence would look like changes. And music starting after real silence doesn't count as a change: the averages start from the first sound.

Novelty is not structure detection. It says "something changed a few seconds ago", not "this is the chorus again". Recognizing that a section repeats is Milestone 5.5's self-similarity matrix.

## 6. What it costs, and how it was tested

The detector runs about 94 transforms of 1024 samples per second, independent of the frame rate. The analysis went from 0.050 to 0.075 ms of CPU per frame. Everything runs per hop, at a fixed rate set by the audio, so the results are identical whether samples arrive 37 or 2048 at a time (a test checks it).

Tests use synthetic sound with known answers:

| Test | Result |
|---|---|
| Clicks at 120 BPM over silence | every click found, within 22 ms of it, nothing else |
| Clicks at 100 BPM over noise 30 dB quieter | every click found, nothing else |
| Kicks at 128 BPM under loud white noise (−10 dBFS) | at least 90% found, at most 2 extra |
| 30 s of loud white noise | no beat after it starts |
| Hi-hats alone | onsets in the high region, no beats |
| Silence | nothing |
| A drop after 6 s of quiet hiss | novelty above 0.5 within a second, below 0.3 seven seconds later |

On real music the detector was only checked by its statistics (beats per second, the spacing between beats), not by ear. On one of the three recordings, beats came about every 0.9 to 1.0 s, a plausible doom-metal tempo, with an occasional beat missed or doubled. How well beats match what a listener feels needs judging with a range of genres.

## Further reading

- Juan Pablo Bello et al., "A Tutorial on Onset Detection in Music Signals", *IEEE Transactions on Speech and Audio Processing* 13 (2005): spectral flux and its relatives, explained.
- Simon Dixon, "Onset Detection Revisited", DAFx 2006: peak picking with an adaptive threshold.
- Sebastian Böck and Gerhard Widmer, "Maximum Filter Vibrato Suppression for Onset Detection", DAFx 2013 (SuperFlux): why comparing with a frame further back helps.
- Jonathan Foote, "Automatic Audio Segmentation Using a Measure of Audio Novelty", ICME 2000: novelty as a change between neighborhoods in time, the idea Milestone 5.5 will build on.
- Explorable `04-beats-and-flashes.html`: build a drum pattern, switch off each part of the detector and see what breaks, and watch novelty rise at a drop.
