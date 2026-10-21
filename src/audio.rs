// src/audio.rs
use std::fs::File;
use std::io::BufReader;
use std::path::Path;

use rodio::{Decoder, Player, Source};

pub struct TrackInfo {
    pub title: String,
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

    // 5. Extract title
    let title = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("Unknown")
        .replace('？', "?") // The bug we fixed earlier
        .to_string();

    Ok(TrackInfo { title, duration })
}
