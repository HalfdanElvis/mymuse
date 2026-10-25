use crate::library::{Library, Track};
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq)]
pub enum Level {
    Root,
    Artists,
    AlbumsOf(String),
    TracksOf(String, String),
    AllAlbums,
    AllSongs,
}

pub struct NavState {
    pub library: Library,
    // (Current Level, selected_index, scroll_offset)
    pub level_stack: Vec<(Level, usize, usize)>,
    // Flattened song list, rebuilt on entry into AllSongs. Read-only while
    // inside the level so get_items() never re-sorts.
    all_songs_cache: Vec<Track>,
}

impl NavState {
    pub fn new(library: Library) -> Self {
        Self {
            library,
            level_stack: vec![(Level::Root, 0, 0)],
            all_songs_cache: Vec::new(),
        }
    }

    pub fn current(&self) -> &(Level, usize, usize) {
        self.level_stack.last().unwrap()
    }

    // --- Playlist / item resolution --------------------------------------

    pub fn get_current_playlist(&self) -> Vec<PathBuf> {
        match &self.current().0 {
            Level::TracksOf(artist, album) => self
                .library
                .get(artist)
                .and_then(|a| a.get(album))
                .map(|tracks| tracks.iter().map(|t| t.path.clone()).collect())
                .unwrap_or_default(),
            Level::AllSongs => self.all_songs().iter().map(|t| t.path.clone()).collect(),
            _ => vec![],
        }
    }

    fn get_all_albums(&self) -> Vec<(String, String)> {
        let mut albums: Vec<(String, String)> = self
            .library
            .iter()
            .flat_map(|(artist, albums_map)| {
                albums_map
                    .keys()
                    .map(move |album| (artist.clone(), album.clone()))
            })
            .collect();
        albums.sort_by(|a, b| a.1.cmp(&b.1).then_with(|| a.0.cmp(&b.0)));
        albums
    }

    fn all_songs(&self) -> &[Track] {
        &self.all_songs_cache
    }

    /// Flatten every track into one vector sorted by display title.
    /// Called once per entry into the Songs level, never per frame.
    fn rebuild_all_songs_cache(&mut self) {
        let mut songs: Vec<Track> = self
            .library
            .values()
            .flat_map(|albums_map| albums_map.values())
            .flat_map(|tracks| tracks.iter().cloned())
            .collect();
        songs.sort_by(|a, b| a.title.cmp(&b.title));
        self.all_songs_cache = songs;
    }

    // --- Display items ---------------------------------------------------

    pub fn get_items(&self) -> Vec<String> {
        let (level, _, _) = self.current();
        match level {
            Level::Root => Self::root_items(),
            Level::Artists => self.artist_items(),
            Level::AlbumsOf(artist) => self.album_items(artist),
            Level::TracksOf(artist, album) => self.track_items(artist, album),
            Level::AllAlbums => self.all_album_items(),
            Level::AllSongs => self.all_song_items(),
        }
    }

    fn root_items() -> Vec<String> {
        ["Artists", "Albums", "Songs", "Settings"]
            .iter()
            .map(|s| s.to_string())
            .collect()
    }

    fn artist_items(&self) -> Vec<String> {
        self.library.keys().cloned().collect()
    }

    fn album_items(&self, artist: &str) -> Vec<String> {
        self.library
            .get(artist)
            .map(|albums| albums.keys().cloned().collect())
            .unwrap_or_default()
    }

    fn track_items(&self, artist: &str, album: &str) -> Vec<String> {
        self.library
            .get(artist)
            .and_then(|albums| albums.get(album))
            .map(|tracks| tracks.iter().map(|t| t.title.clone()).collect())
            .unwrap_or_default()
    }

    fn all_album_items(&self) -> Vec<String> {
        self.get_all_albums()
            .into_iter()
            .map(|(_, album)| album)
            .collect()
    }

    fn all_song_items(&self) -> Vec<String> {
        self.all_songs().iter().map(|t| t.title.clone()).collect()
    }

    // --- Navigation ------------------------------------------------------

    pub fn enter(&mut self) -> Option<PathBuf> {
        let (level, selected_idx, _) = self.current().clone();
        let items = self.get_items();
        if items.is_empty() {
            return None;
        }
        let selected = items[selected_idx].clone();

        match level {
            Level::Root => {
                self.enter_root(&selected);
                None
            }
            Level::Artists => {
                self.level_stack.push((Level::AlbumsOf(selected), 0, 0));
                None
            }
            Level::AlbumsOf(artist) => {
                self.level_stack
                    .push((Level::TracksOf(artist, selected), 0, 0));
                None
            }
            Level::TracksOf(artist, album) => self.track_at(&artist, &album, selected_idx),
            Level::AllAlbums => {
                self.enter_all_album(selected_idx);
                None
            }
            Level::AllSongs => self.all_songs().get(selected_idx).map(|t| t.path.clone()),
        }
    }

    fn enter_root(&mut self, selected: &str) {
        match selected {
            "Artists" => self.level_stack.push((Level::Artists, 0, 0)),
            "Albums" => self.level_stack.push((Level::AllAlbums, 0, 0)),
            "Songs" => {
                // Only place the flattened song list is (re)built.
                self.rebuild_all_songs_cache();
                self.level_stack.push((Level::AllSongs, 0, 0));
            }
            _ => {} // Settings not implemented
        }
    }

    fn enter_all_album(&mut self, idx: usize) {
        if let Some((artist, album)) = self.get_all_albums().get(idx) {
            self.level_stack
                .push((Level::TracksOf(artist.clone(), album.clone()), 0, 0));
        }
    }

    fn track_at(&self, artist: &str, album: &str, idx: usize) -> Option<PathBuf> {
        self.library
            .get(artist)
            .and_then(|a| a.get(album))
            .and_then(|tracks| tracks.get(idx))
            .map(|t| t.path.clone())
    }

    pub fn back(&mut self) -> bool {
        if self.level_stack.len() > 1 {
            self.level_stack.pop();
            true
        } else {
            false
        }
    }
}
