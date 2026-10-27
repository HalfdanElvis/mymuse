use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use rayon::prelude::*;

use crate::library::{self, AlbumArt};
use crate::nav::NavState;

use super::App;

pub(super) struct ArtCache {
    entries: HashMap<(String, String), Arc<AlbumArt>>,
}

impl ArtCache {
    fn new() -> Self {
        Self {
            entries: HashMap::new(),
        }
    }

    pub(super) fn get(&self, key: &(String, String)) -> Option<Arc<AlbumArt>> {
        self.entries.get(key).cloned()
    }

    fn insert(&mut self, key: (String, String), art: Arc<AlbumArt>) {
        self.entries.insert(key, art);
    }
}

impl App {
    /// Decode every album's embedded cover up front, in parallel across all
    /// cores. Albums without embedded art are skipped silently.
    pub(super) fn preload_album_art(nav: &NavState) -> ArtCache {
        let albums = nav.all_albums();
        let total = albums.len();
        eprintln!(
            "Loading album art ({} albums) on {} threads...",
            total,
            rayon::current_num_threads()
        );
        let mut cache = ArtCache::new();
        if total == 0 {
            return cache;
        }

        eprintln!("Loading album art ({} albums)...", total);
        let start = Instant::now();

        let decoded: Vec<((String, String), Arc<AlbumArt>)> = albums
            .par_iter()
            .filter_map(|(artist, album)| {
                let bytes = nav.get_album_art_bytes(artist, album)?;
                let art = library::decode_art(&bytes)?;
                Some(((artist.clone(), album.clone()), Arc::new(art)))
            })
            .collect();

        let decoded_count = decoded.len();
        for (k, a) in decoded {
            cache.insert(k, a);
        }

        eprintln!(
            "  Loaded {} / {} covers in {:?}",
            decoded_count,
            total,
            start.elapsed()
        );
        if decoded_count < total {
            eprintln!("  {} albums have no embedded cover", total - decoded_count);
        }
        cache
    }
}
