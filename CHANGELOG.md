# Changelog

## 1.1.0 — 2026-10-23

Three scenes that are places, an ear for doom's fuzz, an installer, and a quieter program: no console, notes on screen, the wallpaper on every display.

### Added

- No console window in release: messages go to `%LOCALAPPDATA%\Juglar\juglar.log`, and `--console` shows them (D56).
- Text on screen: a note for each change a key makes, the keys on `H`, `?` or `F1`; the tray's menu shows and sets the scene, mode and intensity (D57).
- The wallpaper on every display: a scene on each, one across all, or one display, laid out again when a display comes or goes; `Ctrl`+`Alt` keys that work while Juglar is the wallpaper (D58).
- `JUGLAR_VIEW=analysis` starts on the analysis view.
- An installer beside the zip: per user, Start menu shortcuts, an optional start with Windows as the wallpaper, a clean uninstall (D60).
- Citadel, a scene that is a place: a flight through a city carved out of a fractal, raymarched, with a camera that finds its own way (D61).
- Bore: a tunnel drilled through fractal rock toward a white-hot core; snares reshape the rock, impacts blast it open (D63).
- Barrens: a low flight down a valley of jagged ridges under a storm, for doom; an impact is lightning that lights the land (D64).

### Changed

- Doom's fuzz now reads as gritty: Electric Wizard's loud parts go from grit 0.09 to 0.78, so they get heavy pacing (D62).
- `Esc` asks first: from a window, press it twice to close (D57).
- The window's title names the scene; the numbers go there only with `JUGLAR_STATS=1`.
- The analysis view (`Tab`) is redone: what Juglar hears, labeled, eight seconds of the heavy ear and the drums, and why the scene last changed (D59).

## 1.0.0 — 2026-10-23

The first release: aggressive scenes for metal, rock and grunge.

### Added

- An ear for heavy music: loudness against the song's peaks, grit (distortion), hits and their density, chaos, and impacts (the band slamming back in after a break) (D49).
- Hits drive stiff springs instead of smoothed levels; hard cuts on impacts and on hard hits in wild music; screen effects: shake, sliced glitch, color split, relief lighting, flash, negative, grounds of color (D51).
- Material: the old picture can sharpen itself into texture, reaction-diffusion style; curl noise to stir paint like a liquid (D53).
- Ten scenes for loud music: Etch, Liquid, Shrapnel, Furnace, Static, Maelstrom, Hammer, Wire, Datamosh, Fracture.
- The director changes scene on impacts, with a near cut, and picks the wildest scenes for heavy music, for shorter times (D50).
- `JUGLAR_FILE`, `JUGLAR_RECORD` and `JUGLAR_KEEP`, to play a WAV file silently and record frames (D52).
- An icon, for the program, its window and its tray icon, and the version in the program's file properties (D55).
- Kick, snare and cymbals heard apart, a percussive/harmonic split and relative levels (D45–D47), from before this release cycle.

### Changed

- The twelve older scenes sit on a dim ground of color instead of black, and the three liveliest leave heavy music to the new ones.
- Paint coverage is per second, so scenes look the same at any frame rate.

### Removed

- The flash limiter and its red guard (D48). The lively scenes flash and cut on purpose; see the README's warning.
