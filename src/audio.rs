use std::fs::File;
use std::io::BufReader;
use std::path::Path;

use rodio::{Decoder, Player, Source};

/// Opens an audio file, starts playback, and returns the track duration.
/// Title / artist are no longer returned here — those come from the library
/// scan so playback never re-reads tags.
pub fn play_track(player: &Player, path: &Path) -> Result<f32, String> {
    let file = File::open(path).map_err(|e| format!("Failed to open file: {}", e))?;
    let source = Decoder::new(BufReader::new(file))
        .map_err(|e| format!("Failed to decode audio: {}", e))?;
    let duration = source
        .total_duration()
        .map(|d| d.as_secs_f32())
        .unwrap_or(0.0);
    player.stop();
    player.append(source);
    Ok(duration)
}
