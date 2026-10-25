use lofty::prelude::*;
use lofty::read_from_path;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

pub type Tracklist = Vec<PathBuf>;
pub type ArtistMap = BTreeMap<String, Tracklist>; // Album name -> songs
pub type Library = BTreeMap<String, ArtistMap>; // Artist name -> albums

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
        if HANDLED_EXTENSIONS.contains(&ext.to_lowercase().as_str()) {
            read_metadata(&mut library, path_buf);
        }
    }
    library
}

fn read_metadata(library: &mut Library, path: PathBuf) {
    let (artist, album) = extract_artist_album(&path);
    insert_track(library, artist, album, path);
}

fn extract_artist_album(path: &Path) -> (String, String) {
    let Ok(tagged_file) = read_from_path(path) else {
        return (UNKNOWN_ARTIST.to_string(), UNKNOWN_ALBUM.to_string());
    };
    let Some(tag) = tagged_file.primary_tag() else {
        return (UNKNOWN_ARTIST.to_string(), UNKNOWN_ALBUM.to_string());
    };

    let artist = tag
        .artist()
        .map(|s| s.to_string())
        .unwrap_or_else(|| UNKNOWN_ARTIST.to_string());
    let album = tag
        .album()
        .map(|s| s.to_string())
        .unwrap_or_else(|| UNKNOWN_ALBUM.to_string());

    (artist, album)
}

fn insert_track(library: &mut Library, artist: String, album: String, path: PathBuf) {
    library
        .entry(artist)
        .or_default()
        .entry(album)
        .or_default()
        .push(path);
}
