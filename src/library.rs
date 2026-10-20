use lofty::{Accessor, Itemkey, read_from_path};
use std::collections::BTreeMap;
use std::path::PathBuf;
use walkdir::WalkDir;

// Data structures
pub type Tracklist = Vec<PathBuf>;
pub type AlbumMap = BTreeMap<String, Tracklist>;
pub type ArtistMap = BTreeMap<String, AlbumMap>;
pub type Library = BTreeMap<String, ArtistMap>;

const HANDLED_EXTENSTIONS: [&str; 10] = [
    "wav", "au", "dsd", "flac", "mp3", "m4a", "m4b", "ogg", "oga", "opus",
];

pub fn scan_and_build_library(path: &str) -> Library {
    let mut library: Library = BTreeMap::new();

    for entry in WalkDir::new(path).into_iter().filter_map(|e| e.ok()) {
        if entry.file_type().is_file() {
            let path_buf = entry.path().to_path_buf();
            if let Some(ext) = path_buf.extension().and_then()(|e| e.to_str()) {
                let ext = ext.to_lowercase();
                if HANDLED_EXTENSTIONS.contains(ext) {
                    read_metadata();
                }
            }
        }
    }
}

fn read_metadata(library: &mut Library, path: PathBuf) {
    let tagged_file = match read_from_path(&path) {
        Ok(f) => f,
        Err(_) => {
            // if no tags put in "Unkown Artist/Unkown Album"
            insert_track(
                library,
                "Unkown Artist".to_string(),
                "Unkown Album".to_string(),
                path,
            );
            return;
        }
    };
    let tag = match tagged_file.primary_tag() {
        Some(primary_tag) => primary_tag,
        None => {
            insert_track(
                &mut library,
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
        .unwrap_or_else(|| "Unkown Artist".to_string());
    let album = tag
        .album()
        .map(|s| s.to_string())
        .unwrap_or_else(|| "Unkown Album".to_string());
}

fn insert_track(library: &mut Library, artist: String, album: String, path: PathBuf) {
    library
        .entry(artist)
        .or_insert_with(BTreeMap::new)
        .entry(album)
        .or_insert_with(Vec::new)
        .push(path);
}
