use lofty::prelude::*;
use lofty::read_from_path;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

#[derive(Debug, Clone)]
pub struct Track {
    pub path: PathBuf,
    pub title: String,
    // Only meaningful at scan time for album sorting. Not displayed.
    pub(crate) track_num: u32,
}

pub type Tracklist = Vec<Track>;
pub type ArtistMap = BTreeMap<String, Tracklist>;
pub type Library = BTreeMap<String, ArtistMap>;

const UNKNOWN_ARTIST: &str = "Unknown Artist";
const UNKNOWN_ALBUM: &str = "Unknown Album";

const HANDLED_EXTENSIONS: [&str; 10] = [
    "wav", "au", "dsd", "flac", "mp3", "m4a", "m4b", "ogg", "oga", "opus",
];

pub fn scan_and_build_library(path: &str) -> Library {
    let mut library: Library = BTreeMap::new();

    for entry in WalkDir::new(path).into_iter().filter_map(|e| e.ok()) {
        if !entry.file_type().is_file() {
            continue;
        }
        let path_buf = entry.path().to_path_buf();
        let Some(ext) = path_buf.extension().and_then(|e| e.to_str()) else {
            continue;
        };
        if !HANDLED_EXTENSIONS.contains(&ext.to_lowercase().as_str()) {
            continue;
        }
        let meta = extract_metadata(&path_buf);
        insert_track(&mut library, meta, path_buf);
    }

    // Sort each album by ID3 track number; untagged tracks fall to the end
    // sorted alphabetically by path.
    for albums in library.values_mut() {
        for tracks in albums.values_mut() {
            tracks.sort_by(|a, b| {
                a.track_num
                    .cmp(&b.track_num)
                    .then_with(|| a.path.cmp(&b.path))
            });
        }
    }

    library
}

struct TrackMeta {
    artist: String,
    album: String,
    title: String,
    track_num: u32,
}

fn extract_metadata(path: &Path) -> TrackMeta {
    // Filename stem is the display fallback — verbatim, no stripping.
    let fallback_title = path
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "Unknown".to_string());

    let unknown = || TrackMeta {
        artist: UNKNOWN_ARTIST.into(),
        album: UNKNOWN_ALBUM.into(),
        title: fallback_title.clone(),
        track_num: u32::MAX,
    };

    let Ok(tagged_file) = read_from_path(path) else {
        return unknown();
    };
    let Some(tag) = tagged_file.primary_tag() else {
        return unknown();
    };

    TrackMeta {
        artist: tag
            .artist()
            .map(|s| s.to_string())
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| UNKNOWN_ARTIST.to_string()),
        album: tag
            .album()
            .map(|s| s.to_string())
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| UNKNOWN_ALBUM.to_string()),
        title: tag
            .title()
            .map(|s| s.to_string())
            .filter(|s| !s.trim().is_empty())
            .unwrap_or(fallback_title),
        track_num: tag.track().unwrap_or(u32::MAX),
    }
}

fn insert_track(library: &mut Library, meta: TrackMeta, path: PathBuf) {
    library
        .entry(meta.artist)
        .or_default()
        .entry(meta.album)
        .or_default()
        .push(Track {
            path,
            title: meta.title,
            track_num: meta.track_num,
        });
}
