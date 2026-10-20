use lofty::prelude::*;
use lofty::read_from_path;
use std::collections::BTreeMap;
use std::path::PathBuf;
use walkdir::WalkDir;

// Data structures
pub type Tracklist = Vec<PathBuf>;
pub type ArtistMap = BTreeMap<String, Tracklist>; // Album name + songs
pub type Library = BTreeMap<String, ArtistMap>; // Artist name + (Artists (album name + songs)

const HANDLED_EXTENSIONS: [&str; 10] = [
    "wav", "au", "dsd", "flac", "mp3", "m4a", "m4b", "ogg", "oga", "opus",
];

pub fn scan_and_build_library(path: &str) -> Library {
    let mut library: Library = BTreeMap::new();

    for entry in WalkDir::new(path).into_iter().filter_map(|e| e.ok()) {
        if entry.file_type().is_file() {
            let path_buf = entry.path().to_path_buf();
            if let Some(ext) = path_buf.extension().and_then(|e| e.to_str()) {
                let ext = ext.to_lowercase();
                if HANDLED_EXTENSIONS.contains(&ext.as_str()) {
                    read_metadata(&mut library, path_buf);
                }
            }
        }
    }
    library
}

fn read_metadata(library: &mut Library, path: PathBuf) {
    let tagged_file = match read_from_path(&path) {
        Ok(f) => f,
        Err(_) => {
            // if no tags put in "Unkown Artist/Unkown Album"
            insert_track(
                library,
                "Unknown Artist".to_string(),
                "Unknown Album".to_string(),
                path,
            );
            return;
        }
    };
    let tag = match tagged_file.primary_tag() {
        Some(primary_tag) => primary_tag,
        None => {
            insert_track(
                library,
                "Uknown Artist".to_string(),
                "Unkown Album".to_string(),
                path,
            );
            return;
        }
    };

    // if we find metadata, extract and instert:
    let artist = tag
        .artist()
        .map(|s| s.to_string())
        .unwrap_or_else(|| "Unknown Artist".to_string());
    let album = tag
        .album()
        .map(|s| s.to_string())
        .unwrap_or_else(|| "Unknown Album".to_string());

    insert_track(library, artist, album, path);
}

fn insert_track(library: &mut Library, artist: String, album: String, path: PathBuf) {
    library
        .entry(artist)
        .or_default()
        .entry(album)
        .or_default()
        .push(path);
}
