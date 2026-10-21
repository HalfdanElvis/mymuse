// src/nav.rs
use std::path::PathBuf;
use crate::library::Library;

#[derive(Debug, Clone, PartialEq)]
pub enum Level {
    Root,                        // "Artists", "Albums", "Songs", "Settings"
    Artists,                     // List of artists
    AlbumsOf(String),            // List of albums for a specific artist
    TracksOf(String, String),    // List of tracks for an artist + album
    AllSongs,                    // Flat list of all songs
}

pub struct NavState {
    pub library: Library,
    pub level_stack: Vec<(Level, usize)>, // (Current Level, selected_index)
}

impl NavState {
    pub fn new(library: Library) -> Self {
        Self {
            library,
            // Start at Root with index 0
            level_stack: vec![(Level::Root, 0)],
        }
    }

    // Get the current level and selected index
    pub fn current(&self) -> &(Level, usize) {
        self.level_stack.last().unwrap()
    }

    // Get the list of strings to display for the current level
    pub fn get_items(&self) -> Vec<String> {
        let (level, _) = self.current();
        match level {
            Level::Root => vec![
                "Artists".to_string(), 
                "Albums".to_string(), 
                "Songs".to_string(), 
                "Settings".to_string()
            ],
            Level::Artists => self.library.keys().cloned().collect(),
            Level::AlbumsOf(artist) => {
                self.library.get(artist)
                    .map(|albums| albums.keys().cloned().collect())
                    .unwrap_or_default()
            },
            Level::TracksOf(artist, album) => {
                self.library.get(artist)
                    .and_then(|albums| albums.get(album))
                    .map(|tracks| {
                        tracks.iter()
                            .map(|p| p.file_stem().unwrap_or_default().to_string_lossy().to_string())
                            .collect()
                    })
                    .unwrap_or_default()
            },
            Level::AllSongs => { /* flatten and return all song names */ vec![] }
        }
    }

    // Handle the Enter key
    pub fn enter(&mut self) -> Option<PathBuf> {
        let (level, selected_idx) = self.current().clone();
        let items = self.get_items();

        if items.is_empty() { return None; }
        
        let selected_item = &items[selected_idx];

        match level {
            Level::Root => {
                match selected_item.as_str() {
                    "Artists" => self.level_stack.push((Level::Artists, 0)),
                    "Songs" => self.level_stack.push((Level::AllSongs, 0)),
                    _ => {} // Settings, Albums...
                }
                None
            }
            Level::Artists => {
                self.level_stack.push((Level::AlbumsOf(selected_item.clone()), 0));
                None
            }
            Level::AlbumsOf(artist) => {
                self.level_stack.push((Level::TracksOf(artist, selected_item.clone()), 0));
                None
            }
            Level::TracksOf(artist, album) => {
                // We actually want to play a file now!
                self.library.get(&artist)
                    .and_then(|a| a.get(&album))
                    .and_then(|tracks| tracks.get(selected_idx))
                    .cloned()
            }
            Level::AllSongs => { /* find song and return path */ None }
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
