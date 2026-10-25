use std::fs::File;
use std::io::BufReader;
use std::path::Path;

use lofty::prelude::*;
use lofty::read_from_path;
use rodio::{Decoder, Player, Source};

pub struct TrackInfo {
    pub title: String,
    pub artist: String,
    pub duration: f32,
}

/// Opens an audio file, starts playing it, and returns the track metadata.
pub fn play_track(player: &Player, path: &Path) -> Result<TrackInfo, String> {
    let file = File::open(path).map_err(|e| format!("Failed to open file: {}", e))?;

    let source = Decoder::new(BufReader::new(file))
        .map_err(|e| format!("Failed to decode audio: {}", e))?;

    let duration = source
        .total_duration()
        .map(|d| d.as_secs_f32())
        .unwrap_or(0.0);

    player.stop();
    player.append(source);

    let (title, artist) = extract_metadata(path);

    Ok(TrackInfo {
        title,
        artist,
        duration,
    })
}

/// Reads ID3 / Vorbis tags with a filename fallback for the title.
fn extract_metadata(path: &Path) -> (String, String) {
    let fallback_title = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("Unknown")
        .replace('？', "?")
        .to_string();

    let mut title = fallback_title;
    let mut artist = "Unknown Artist".to_string();

    if let Ok(tagged_file) = read_from_path(path) {
        if let Some(tag) = tagged_file.primary_tag() {
            if let Some(t) = tag.title() {
                if !t.trim().is_empty() {
                    title = t.to_string();
                }
            }
            if let Some(a) = tag.artist() {
                if !a.trim().is_empty() {
                    artist = a.to_string();
                }
            }
        }
    }

    (title, artist)
}
