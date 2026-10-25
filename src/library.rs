use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use lofty::prelude::*;
use lofty::read_from_path;
use walkdir::WalkDir;

#[derive(Debug, Clone)]
pub struct Track {
    pub path: PathBuf,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub track_num: u32,
}

#[derive(Debug, Clone)]
pub struct AlbumArt {
    pub width: usize,
    pub height: usize,
    pub rgba: Vec<[u8; 4]>,
}

pub struct Album {
    pub tracks: Vec<Track>,
    /// Encoded JPEG/PNG/WebP bytes, shared across all tracks of the album.
    /// Decoded lazily so scan-time memory stays small.
    pub art_bytes: Option<Arc<Vec<u8>>>,
}

pub type ArtistMap = BTreeMap<String, Album>;
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

    // Sort each album by ID3 track number; untagged tracks go to the end,
    // sorted alphabetically by path.
    for albums in library.values_mut() {
        for album in albums.values_mut() {
            album.tracks.sort_by(|a, b| {
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
    art_bytes: Option<Arc<Vec<u8>>>,
}

fn extract_metadata(path: &Path) -> TrackMeta {
    let fallback_title = path
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "Unknown".to_string());

    let unknown = || TrackMeta {
        artist: UNKNOWN_ARTIST.into(),
        album: UNKNOWN_ALBUM.into(),
        title: fallback_title.clone(),
        track_num: u32::MAX,
        art_bytes: None,
    };

    let Ok(tagged_file) = read_from_path(path) else {
        return unknown();
    };
    let Some(tag) = tagged_file.primary_tag() else {
        return unknown();
    };

    // First embedded picture wins. To strictly prefer a front cover swap
    // `.first()` for a `.find(|p| p.pic_type() == PictureType::CoverFront)`.
    let art_bytes = tag.pictures().first().map(|p| Arc::new(p.data().to_vec()));

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
        art_bytes,
    }
}

fn insert_track(library: &mut Library, meta: TrackMeta, path: PathBuf) {
    let album = library
        .entry(meta.artist.clone())
        .or_default()
        .entry(meta.album.clone())
        .or_insert_with(|| Album {
            tracks: Vec::new(),
            art_bytes: None,
        });

    if album.art_bytes.is_none() {
        album.art_bytes = meta.art_bytes;
    }

    album.tracks.push(Track {
        path,
        title: meta.title,
        artist: meta.artist,
        album: meta.album,
        track_num: meta.track_num,
    });
}

/// Decodes embedded cover art into RGBA pixels capped at 128x128.
/// Returns `None` on any decode error — the UI just skips drawing.
pub fn decode_art(bytes: &[u8]) -> Option<AlbumArt> {
    let img = image::load_from_memory(bytes).ok()?;
    let img = img.thumbnail(128, 128);
    let rgba = img.to_rgba8();
    let width = rgba.width() as usize;
    let height = rgba.height() as usize;
    let raw = rgba.into_raw();
    let mut pixels = Vec::with_capacity(raw.len() / 4);
    for chunk in raw.chunks_exact(4) {
        pixels.push([chunk[0], chunk[1], chunk[2], chunk[3]]);
    }
    Some(AlbumArt {
        width,
        height,
        rgba: pixels,
    })
}
