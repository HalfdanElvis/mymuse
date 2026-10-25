use mlua::MetaMethod::Le;

// src/nav.rs
use crate::library::Library;
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq)]
pub enum Level {
    Root,                     // "Artists", "Albums", "Songs", "Settings"
    Artists,                  // List of artists
    AlbumsOf(String),         // List of albums for a specific artist
    TracksOf(String, String), // List of tracks for an artist + album
    AllAlbums,                // Flat list of all albums
    AllSongs,                 // Flat list of all songs
}

pub struct NavState {
    pub library: Library,
    pub level_stack: Vec<(Level, usize, usize)>, // (Current Level, selected_index, scroll_offset)
}

impl NavState {
    pub fn new(library: Library) -> Self {
        Self {
            library,
            // Start at Root with index and offset 0
            level_stack: vec![(Level::Root, 0, 0)],
        }
    }

    // Get the current level and selected index
    pub fn current(&self) -> &(Level, usize, usize) {
        self.level_stack.last().unwrap()
    }

    pub fn get_current_playlist(&self) -> Vec<PathBuf> {
        let (level, _, _) = self.current();
        match level {
            Level::TracksOf(artist, album) => self
                .library
                .get(artist)
                .and_then(|a| a.get(album))
                .cloned()
                .unwrap_or_default(),
            Level::AllSongs => self.get_all_songs(),
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
        // Potentially add toggle to sorting, currently sorting by album names, could be sorting by artist
        // then album name or something else.
        albums.sort_by(|a, b| a.1.cmp(&b.1).then_with(|| a.0.cmp(&b.0)));

        albums
    }

    fn get_all_songs(&self) -> Vec<PathBuf> {
        let mut songs: Vec<PathBuf> = self
            .library
            .values()
            .flat_map(|albums_map| albums_map.values())
            .flat_map(|tracks| tracks.iter().cloned())
            .collect();

        songs.sort_by(|a, b| {
            let a_name = a.file_stem().unwrap_or_default().to_string_lossy();
            let b_name = b.file_stem().unwrap_or_default().to_string_lossy();
            a_name.cmp(&b_name)
        });
        songs
    }

    // Get the list of strings to display for the current level
    pub fn get_items(&self) -> Vec<String> {
        let (level, _, _) = self.current();
        match level {
            Level::Root => vec![
                "Artists".to_string(),
                "Albums".to_string(),
                "Songs".to_string(),
                "Settings".to_string(),
            ],
            Level::Artists => self.library.keys().cloned().collect(),
            Level::AlbumsOf(artist) => self
                .library
                .get(artist)
                .map(|albums| albums.keys().cloned().collect())
                .unwrap_or_default(),
            Level::TracksOf(artist, album) => self
                .library
                .get(artist)
                .and_then(|albums| albums.get(album))
                .map(|tracks| {
                    tracks
                        .iter()
                        .map(|p| {
                            p.file_stem()
                                .unwrap_or_default()
                                .to_string_lossy()
                                .to_string()
                        })
                        .collect()
                })
                .unwrap_or_default(),
            Level::AllAlbums => self
                .get_all_albums()
                .into_iter()
                .map(|(artist, album)| album)
                .collect(),

            Level::AllSongs => self
                .get_all_songs()
                .into_iter()
                .map(|p| {
                    p.file_stem()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_string()
                })
                .collect(),
        }
    }

    // Handle the Enter key
    pub fn enter(&mut self) -> Option<PathBuf> {
        let (level, selected_idx, _) = self.current().clone();
        let items = self.get_items();

        if items.is_empty() {
            return None;
        }

        let selected_item = &items[selected_idx];

        match level {
            Level::Root => {
                match selected_item.as_str() {
                    "Artists" => self.level_stack.push((Level::Artists, 0, 0)),
                    "Albums" => self.level_stack.push((Level::AllAlbums, 0, 0)),
                    "Songs" => self.level_stack.push((Level::AllSongs, 0, 0)),
                    _ => {} // Settings, Albums...
                }
                None
            }
            Level::Artists => {
                self.level_stack
                    .push((Level::AlbumsOf(selected_item.clone()), 0, 0));
                None
            }
            Level::AlbumsOf(artist) => {
                self.level_stack
                    .push((Level::TracksOf(artist, selected_item.clone()), 0, 0));
                None
            }
            Level::TracksOf(artist, album) => {
                // We actually want to play a file now!
                self.library
                    .get(&artist)
                    .and_then(|a| a.get(&album))
                    .and_then(|tracks| tracks.get(selected_idx))
                    .cloned()
            }
            Level::AllAlbums => {
                /* find song and return path */
                let albums = self.get_all_albums();
                if let Some((artist, album)) = albums.get(selected_idx) {
                    self.level_stack
                        .push((Level::TracksOf(artist.clone(), album.clone()), 0, 0));
                }
                None
            }
            Level::AllSongs => {
                /* find song and return path */
                let songs = self.get_all_songs();
                songs.get(selected_idx).cloned()
            }
        }
    }

    // Handle the Back/Escape key
    pub fn back(&mut self) -> bool {
        if self.level_stack.len() > 1 {
            self.level_stack.pop();
            true // We went back
        } else {
            false // We are at the root, maybe exit the app
        }
    }
}
