use std::time::Instant;

use winit::{event_loop::ActiveEventLoop, keyboard::KeyCode};

use super::App;

impl App {
    pub(super) fn handle_key(&mut self, event_loop: &ActiveEventLoop, key_code: KeyCode) {
        match key_code {
            KeyCode::Escape => event_loop.exit(),
            KeyCode::Backspace => self.nav_back(),
            KeyCode::ArrowDown => self.nav_down(),
            KeyCode::ArrowUp => self.nav_up(),
            KeyCode::Enter => self.nav_enter(),
            KeyCode::Space => self.toggle_pause(),
            KeyCode::ArrowLeft | KeyCode::ArrowRight => self.start_seek_hold(key_code),
            _ => {}
        }
    }

    fn toggle_pause(&mut self) {
        if self.player.is_paused() {
            self.player.play();
        } else {
            self.player.pause();
        }
        self.request_redraw();
    }

    fn nav_back(&mut self) {
        if self.nav.back() {
            self.hover_start_time = Instant::now();
            self.request_redraw();
        }
    }

    fn nav_down(&mut self) {
        let (_, idx, _) = self.nav.current();
        let items_len = self.nav.get_items().len();
        if *idx >= items_len.saturating_sub(1) {
            return;
        }
        if let Some((_, idx_mut, scroll_mut)) = self.nav.level_stack.last_mut() {
            *idx_mut += 1;
            if *idx_mut >= *scroll_mut + self.ui.layout.visible_rows {
                *scroll_mut += 1;
            }
        }
        self.hover_start_time = Instant::now();
        self.request_redraw();
    }

    fn nav_up(&mut self) {
        let (_, idx, _) = self.nav.current();
        if *idx == 0 {
            return;
        }
        if let Some((_, idx_mut, scroll_mut)) = self.nav.level_stack.last_mut() {
            *idx_mut -= 1;
            if *idx_mut < *scroll_mut {
                *scroll_mut -= 1;
            }
        }
        self.hover_start_time = Instant::now();
        self.request_redraw();
    }

    fn nav_enter(&mut self) {
        self.hover_start_time = Instant::now();
        if let Some(idx) = self.nav.enter() {
            let queue = self.nav.get_current_playlist();
            if let Some(track) = queue.get(idx).cloned() {
                self.play_track_from_queue(track, queue, idx);
            }
        }
        self.request_redraw();
    }
}
