//! A song's title when it starts: the cover, the artist, the title large
//! and the album, in the middle of the screen, written out letter by
//! letter over a pool of dark, held for a few seconds, then let go with the
//! title spreading as it fades. Quiet in color like the rest of the text
//! (off-white and grays, one accent line), so the scene keeps the noise.

use crate::hud::{ACCENT, GRAY, INK};
use crate::ink::{Ink, Set, Weight};

/// The song a title is shown for.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Song {
    pub title: String,
    pub artist: String,
    pub album: String,
    /// Where it plays from, as people call it: "Spotify", "YouTube".
    pub source: String,
}

/// When each part comes, in seconds from the start.
const COVER_IN: (f32, f32) = (0.0, 0.7);
const ARTIST_AT: f32 = 0.25;
const TITLE_AT: f32 = 0.5;
const ALBUM_IN: (f32, f32) = (1.1, 1.8);
const LINE_IN: (f32, f32) = (0.35, 1.0);
/// Letters a second as the artist and the title are written out.
const ARTIST_PACE: f32 = 30.0;
const TITLE_PACE: f32 = 24.0;
/// When it starts to go, and how long that takes.
const LEAVE_AT: f32 = 5.4;
const LEAVE: f32 = 0.9;

#[derive(Default)]
pub(crate) struct Intro {
    song: Option<Song>,
    /// Seconds since it started.
    age: f32,
    /// Seconds since the cover came, which may be after the title.
    cover_age: Option<f32>,
}

impl Intro {
    pub(crate) fn start(&mut self, song: Song, has_cover: bool) {
        self.song = Some(song);
        self.age = 0.0;
        self.cover_age = has_cover.then_some(0.0);
    }

    /// The cover arrived for the song showing.
    pub(crate) fn cover_arrived(&mut self) {
        if self.song.is_some() && self.cover_age.is_none() {
            self.cover_age = Some(0.0);
        }
    }

    pub(crate) fn stop(&mut self) {
        self.song = None;
    }

    pub(crate) fn showing(&self) -> bool {
        self.song.is_some()
    }

    /// Queues this frame's title, `dt` seconds after the last; `beat`
    /// brightens the accent line. With `still`, nothing moves: it fades in
    /// whole and out.
    pub(crate) fn draw(&mut self, ink: &mut Ink, queue: &wgpu::Queue, size: [f32; 2], dt: f32, beat: f32, still: bool) {
        let Some(song) = &self.song else { return };
        if !ink.has_fonts() {
            return;
        }
        self.age += dt;
        if let Some(age) = &mut self.cover_age {
            *age += dt;
        }
        let t = self.age;
        if t > LEAVE_AT + LEAVE {
            self.song = None;
            return;
        }
        let ramp = |(from, to): (f32, f32), t: f32| smooth(((t - from) / (to - from)).clamp(0.0, 1.0));
        let leaving = smooth(((t - LEAVE_AT) / LEAVE).clamp(0.0, 1.0));
        let stay = 1.0 - leaving;
        let [w, h] = size;
        let base = h.min(w * 0.62);

        // Sizes, the title shrunk to fit the width.
        let artist_size = (base * 0.03).max(11.0);
        let artist_tracking = artist_size * 0.32;
        let artist = song.artist.to_uppercase();
        let mut title_size = (base * 0.088).max(18.0);
        let room = w * 0.84;
        let title_width = ink.width(queue, &song.title, title_size, Weight::Strong);
        if title_width > room {
            title_size = (title_size * room / title_width).max(base * 0.045);
        }
        let title = ink.clip(queue, &song.title, title_size, Weight::Strong, room);
        let album_size = (base * 0.028).max(11.0);
        let album = match (song.album.is_empty(), song.source.is_empty()) {
            (false, false) => format!("{}   \u{b7}   {}", song.album, song.source),
            (false, true) => song.album.clone(),
            (true, false) => song.source.clone(),
            (true, true) => String::new(),
        };
        let album = ink.clip(queue, &album, album_size, Weight::Regular, room);
        let cover_size = if ink.picture_aspect().is_some() && self.cover_age.is_some() { base * 0.3 } else { 0.0 };

        // The stack's height, centered a little above the middle.
        let (artist_ascent, _) = ink.metrics(artist_size);
        let (title_ascent, title_descent) = ink.metrics(title_size);
        let (album_ascent, _) = ink.metrics(album_size);
        let cover_gap = if cover_size > 0.0 { base * 0.045 } else { 0.0 };
        let artist_gap = title_size * 0.32;
        let album_gap = title_size * 0.62;
        let height = cover_size + cover_gap + artist_ascent + artist_gap + title_ascent + title_descent + album_gap + album_ascent;
        let top = ((h - height) * 0.46).max(0.0);

        // A pool of dark for the words to stand on, wider than they are.
        let title_width = ink.width(queue, &title, title_size, Weight::Strong);
        let artist_width = ink.tracked_width(queue, &artist, artist_size, Weight::Strong, artist_tracking);
        let album_width = ink.width(queue, &album, album_size, Weight::Regular);
        // A cover keeps its shape: square for an album, wide for a video.
        let aspect = ink.picture_aspect().unwrap_or(1.0).clamp(0.5, 2.0);
        let wide = title_width.max(artist_width).max(album_width).max(cover_size * aspect);
        let reach = base * 0.2;
        let pool = ramp((0.0, 0.5), t) * stay;
        ink.soft_panel(
            [(w - wide) * 0.5 - reach * 1.6, top - reach, wide + reach * 3.2, height + reach * 2.0],
            reach,
            reach,
            [0.0, 0.0, 0.0],
            0.62 * pool,
        );

        let mut y = top;
        if cover_size > 0.0 {
            let came = self.cover_age.unwrap_or(0.0);
            let shown = if still { 1.0 } else { ramp(COVER_IN, came) };
            let scale = (0.94 + 0.06 * shown) * (1.0 + 0.03 * leaving);
            let (across, side) = (cover_size * aspect * scale, cover_size * scale);
            let (cx, cy) = (w * 0.5, y + cover_size * 0.5);
            ink.soft_panel(
                [cx - across * 0.5 - base * 0.03, cy - side * 0.5 - base * 0.02, across + base * 0.06, side + base * 0.07],
                base * 0.03,
                base * 0.03,
                [0.0, 0.0, 0.0],
                0.55 * shown * stay,
            );
            ink.picture([cx - across * 0.5, cy - side * 0.5, across, side], base * 0.008, shown * stay);
            y += cover_size + cover_gap;
        }

        // The artist, spaced out and written letter by letter.
        y += artist_ascent;
        let set = |at: f32, pace: f32, rise: f32| Set {
            tracking: 0.0,
            shown: if still { f32::INFINITY } else { (t - at) * pace },
            rise: if still { 0.0 } else { rise },
        };
        let fade_in = if still { ramp((0.0, 0.6), t) } else { 1.0 };
        let artist_set = Set { tracking: artist_tracking * (1.0 + 0.5 * leaving), ..set(ARTIST_AT, ARTIST_PACE, artist_size * 0.7) };
        let artist_width = ink.tracked_width(queue, &artist, artist_size, Weight::Strong, artist_set.tracking);
        ink.set_text(queue, &artist, [(w - artist_width) * 0.5, y], artist_size, Weight::Strong, GRAY, fade_in * stay, artist_set);

        // The title, large; it spreads a little as it goes.
        y += artist_gap + title_ascent;
        let title_set = Set { tracking: title_size * 0.06 * leaving, ..set(TITLE_AT, TITLE_PACE, title_size * 0.3) };
        let title_width = ink.tracked_width(queue, &title, title_size, Weight::Strong, title_set.tracking);
        ink.set_text(queue, &title, [(w - title_width) * 0.5, y], title_size, Weight::Strong, INK, fade_in * stay, title_set);

        // The accent line under it, drawn out from the middle.
        y += title_descent + album_gap * 0.45;
        let line = if still { 1.0 } else { ramp(LINE_IN, t) } * stay;
        let half = title_width.min(base * 0.5) * 0.5 * line;
        let weight = (base * 0.0035).max(1.5) * (1.0 + 0.6 * beat.clamp(0.0, 1.0));
        ink.line([w * 0.5 - half, y], [w * 0.5 + half, y], weight, ACCENT, line);

        // The album and where it plays from.
        y += album_gap * 0.55 + album_ascent;
        let album_alpha = ramp(ALBUM_IN, t) * stay;
        ink.text(queue, &album, [(w - album_width) * 0.5, y], album_size, Weight::Regular, GRAY, album_alpha);
    }
}

fn smooth(t: f32) -> f32 {
    t * t * (3.0 - 2.0 * t)
}
