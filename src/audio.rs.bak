// src/audio.rs
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
    // 1. Open the file
    let file = File::open(path).map_err(|e| format!("Failed to open file: {}", e))?;

    // 2. Decode it
    let source = Decoder::new(BufReader::new(file))
        .map_err(|e| format!("Failed to decode audio: {}", e))?;

    // 3. Get duration before we consume the source
    let duration = source.total_duration().map(|d| d.as_secs_f32()).unwrap_or(0.0);

    // 4. Play it
    player.stop();
    player.append(source);

    // 5. Extract Metadata: Tag first, Filename fallback
    let mut title = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("Unknown")
        .replace('？', "?")
        .to_string();
        
    let mut artist = "Unknown Artist".to_string();

    // Try to read the ID3 tag
    if let Ok(tagged_file) = read_from_path(path) {
        if let Some(tag) = tagged_file.primary_tag() {
            // Title
            if let Some(tag_title) = tag.title() {
                if !tag_title.trim().is_empty() {
                    title = tag_title.to_string();
                }
            }
            // Artist
            if let Some(tag_artist) = tag.artist() {
                if !tag_artist.trim().is_empty() {
                    artist = tag_artist.to_string();
                }
            }
        }
    }

    Ok(TrackInfo {
        title,
        artist,
        duration,
    })
}
