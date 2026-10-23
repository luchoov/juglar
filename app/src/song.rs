//! From what a player says to the title Juglar shows. Spotify says it
//! well; a browser says what the page says, and on YouTube that is often a
//! video's title ("Pearl Jam - Black (Official Audio)") under the channel's
//! name ("PearlJamVEVO", "Pearl Jam - Topic"). This tidies those, and names
//! where the song plays from as people call it.

use juglar_platform::{AudioApp, Track};
use juglar_render::Song;

/// Browsers, by a piece of the name Windows gives their media sessions
/// (Firefox's is a number), and what to call them.
const BROWSERS: [(&str, &str); 6] = [
    ("chrome", "Chrome"),
    ("msedge", "Edge"),
    ("firefox", "Firefox"),
    ("308046b0af4a39cb", "Firefox"),
    ("brave", "Brave"),
    ("opera", "Opera"),
];

/// Words that mark a bracketed part of a video's title as noise.
const NOISE: [&str; 10] = ["official", "video", "audio", "lyric", "visualizer", "visualiser", "hd", "4k", "hq", "mv"];

/// The song to show for `track`, with the programs making sound (for
/// naming players Juglar doesn't know).
pub(crate) fn song(track: &Track, apps: &[AudioApp]) -> Song {
    let id = track.player.to_lowercase();
    let browser = BROWSERS.iter().find(|(piece, _)| id.contains(piece)).map(|&(_, name)| name);
    let mut title = track.title.trim().to_owned();
    let mut artist = track.artist.trim().to_owned();
    let mut youtube = false;
    if browser.is_some() {
        // A channel's name: "Pearl Jam - Topic", "PearlJamVEVO".
        if let Some(name) = artist.strip_suffix(" - Topic") {
            artist = name.to_owned();
            youtube = true;
        } else if artist.ends_with("VEVO") {
            youtube = true;
        }
        // "Artist - Title" in the title: the artist is the left part.
        if track.album.is_empty()
            && let Some((left, right)) = title.split_once(" - ").or_else(|| title.split_once(" \u{2013} "))
            && !left.trim().is_empty()
            && !right.trim().is_empty()
        {
            (artist, title) = (left.trim().to_owned(), right.trim().to_owned());
            youtube = true;
        }
        title = without_noise(&title);
    }
    let source = match browser {
        Some(_) if youtube => "YouTube".to_owned(),
        Some(name) => name.to_owned(),
        None if id.contains("spotify") => "Spotify".to_owned(),
        None => player_name(&track.player, apps),
    };
    Song { title, artist, album: track.album.trim().to_owned(), source }
}

/// A player's name for people: the program's own description when it is
/// making sound, else Windows' name for it without its path or `.exe`.
fn player_name(player: &str, apps: &[AudioApp]) -> String {
    let stem = player.rsplit(['\\', '/', '!']).next().unwrap_or(player);
    let stem = stem.strip_suffix(".exe").or_else(|| stem.strip_suffix(".EXE")).unwrap_or(stem);
    apps.iter()
        .find(|app| app.exe.to_lowercase().trim_end_matches(".exe") == stem.to_lowercase())
        .map_or_else(|| stem.to_owned(), |app| app.name.clone())
}

/// `title` without bracketed noise: "(Official Video)", "[HD]".
fn without_noise(title: &str) -> String {
    let mut out = String::new();
    let mut rest = title;
    while let Some(open) = rest.find(['(', '[']) {
        let close = if rest[open..].starts_with('(') { ')' } else { ']' };
        let Some(length) = rest[open..].find(close) else { break };
        let inside = rest[open + 1..open + length].to_lowercase();
        let noisy = inside.split(|c: char| !c.is_alphanumeric()).any(|word| NOISE.contains(&word));
        out.push_str(&rest[..open]);
        if !noisy {
            out.push_str(&rest[open..=open + length]);
        }
        rest = &rest[open + length + 1..];
    }
    out.push_str(rest);
    let tidy = out.split_whitespace().collect::<Vec<_>>().join(" ");
    if tidy.is_empty() { title.to_owned() } else { tidy }
}

/// The song in a line: "Black — Pearl Jam".
pub(crate) fn line(song: &Song) -> String {
    if song.artist.is_empty() { song.title.clone() } else { format!("{} \u{2014} {}", song.title, song.artist) }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn track(title: &str, artist: &str, album: &str, player: &str) -> Track {
        Track { title: title.into(), artist: artist.into(), album: album.into(), player: player.into(), playing: true, id: 1 }
    }

    #[test]
    fn spotify_is_taken_as_it_says() {
        let song = song(&track("Jezebel", "Sepultura", "Chaos A.D.", "Spotify.exe"), &[]);
        assert_eq!((song.title.as_str(), song.artist.as_str(), song.source.as_str()), ("Jezebel", "Sepultura", "Spotify"));
    }

    #[test]
    fn a_video_title_is_split_and_cleaned() {
        let song = song(&track("Pearl Jam - Black (Official Audio) [HD]", "PearlJamVEVO", "", "Chrome"), &[]);
        assert_eq!((song.title.as_str(), song.artist.as_str(), song.source.as_str()), ("Black", "Pearl Jam", "YouTube"));
    }

    #[test]
    fn a_topic_channel_is_its_artist() {
        let song = song(&track("Outshined", "Soundgarden - Topic", "", "MSEdge"), &[]);
        assert_eq!((song.title.as_str(), song.artist.as_str(), song.source.as_str()), ("Outshined", "Soundgarden", "YouTube"));
    }

    #[test]
    fn meaningful_brackets_stay() {
        assert_eq!(without_noise("Breed (Remastered 2021) (Official Video)"), "Breed (Remastered 2021)");
    }

    #[test]
    fn unknown_players_are_named_by_their_program() {
        let apps = [AudioApp { exe: "foobar2000.exe".into(), name: "foobar2000".into(), playing: true }];
        assert_eq!(song(&track("A", "B", "C", "C:\\Apps\\foobar2000.exe"), &apps).source, "foobar2000");
    }
}
