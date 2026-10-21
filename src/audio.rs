// src/audio.rs
use lofty::prelude::*;
use lofty::read_from_path;
use std::fs::File;
use std::io::BufReader;
use std::path::Path;

use rodio::{Decoder, Player, Source};

pub struct TrackInfo {
    pub title: String,
    pub artist: String, // <-- Add this
    pub duration: f32,
}

pub fn play_track(player: &Player, path: &Path) -> Result<TrackInfo, String> {
    let file = File::open(path).map_err(|e| format!("Failed to open file: {}", e))?;
    let source = Decoder::new(BufReader::new(file))
        .map_err(|e| format!("Failed to decode audio: {}", e))?;

    let duration = source.total_duration().map(|d| d.as_secs_f32()).unwrap_or(0.0);

    player.stop();
    player.append(source);

    // Extract metadata for the Now Playing bar and Autoplay
    let tagged_file = read_from_path(path).ok();
    
    let title = tagged_file.as_ref()
        .and_then(|f| f.primary_tag())
        .and_then(|t| t.title())
        .map(|s| s.to_string())
        .unwrap_or_else(|| {
            path.file_stem().unwrap_or_default().to_string_lossy().replace('？', "?").to_string()
        });

    let artist = tagged_file.as_ref()
        .and_then(|f| f.primary_tag())
        .and_then(|t| t.artist())
        .map(|s| s.to_string())
        .unwrap_or_else(|| "Unknown Artist".to_string());

    Ok(TrackInfo { title, artist, duration })
}
