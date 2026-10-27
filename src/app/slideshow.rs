use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::library::AlbumArt;
use crate::nav::Level;

use super::util::pseudo_random_index;
use super::App;

pub(super) const SLIDESHOW_INTERVAL: Duration = Duration::from_secs(4);
pub(super) const TRANSITION_DURATION: Duration = Duration::from_millis(500);

pub(super) struct PaneArtState {
    current: Option<(String, String)>,
    previous: Option<(String, String)>,
    context_artist: Option<String>,
    last_swap: Instant,
    transition_start: Instant,
}

impl PaneArtState {
    pub(super) fn new() -> Self {
        Self {
            current: None,
            previous: None,
            context_artist: None,
            last_swap: Instant::now(),
            transition_start: Instant::now(),
        }
    }

    fn progress(&self) -> f32 {
        (self.transition_start.elapsed().as_secs_f32() / TRANSITION_DURATION.as_secs_f32())
            .clamp(0.0, 1.0)
    }
}

impl App {
    pub(super) fn get_pane_art(&self) -> (Option<Arc<AlbumArt>>, Option<Arc<AlbumArt>>, f32) {
        let progress = self.pane_art.progress();
        let curr = self.pane_art.current.as_ref().and_then(|k| self.get_art(k));
        let prev = if progress < 1.0 {
            self.pane_art
                .previous
                .as_ref()
                .and_then(|k| self.get_art(k))
        } else {
            None
        };
        (prev, curr, progress)
    }

    pub(super) fn update_pane_art(&mut self) {
        let level = self.nav.current().0.clone();

        let desired = if matches!(level, Level::Root | Level::Artists) {
            self.slideshow_pick(&level)
        } else {
            self.nav.current_album_context()
        };

        if desired != self.pane_art.current {
            self.pane_art.previous = self.pane_art.current.take();
            self.pane_art.current = desired;
            self.pane_art.transition_start = Instant::now();
        }
    }

    fn slideshow_pick(&mut self, level: &Level) -> Option<(String, String)> {
        let context_artist = match level {
            Level::Artists => self.nav.hovered_artist(),
            _ => None,
        };

        let context_changed = self.pane_art.context_artist != context_artist;
        let fresh_enough = !context_changed
            && self.pane_art.current.is_some()
            && self.pane_art.last_swap.elapsed() < SLIDESHOW_INTERVAL;
        if fresh_enough {
            return self.pane_art.current.clone();
        }

        let pool: Vec<(String, String)> = match &context_artist {
            Some(artist) => self
                .nav
                .all_albums()
                .iter()
                .filter(|(a, _)| a == artist)
                .cloned()
                .collect(),
            None => self.nav.all_albums().to_vec(),
        };

        self.pane_art.context_artist = context_artist;
        self.pane_art.last_swap = Instant::now();

        if pool.is_empty() {
            return None;
        }
        if pool.len() == 1 {
            return Some(pool[0].clone());
        }

        let mut idx = pseudo_random_index(pool.len());
        if let Some(cur) = &self.pane_art.current {
            if &pool[idx] == cur {
                idx = (idx + 1) % pool.len();
            }
        }
        Some(pool[idx].clone())
    }
}
