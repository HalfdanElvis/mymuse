use std::num::NonZeroU32;
use std::time::Instant;

use crate::ui::{NowPlaying, UiFrame};

use super::util::{clock_string, level_title};
use super::{App, HEIGHT, WIDTH};

impl App {
    pub(super) fn render(&mut self) {
        let clock = clock_string();
        let elapsed = self.player.get_pos().as_secs_f32();
        let (level, selected_index, scroll_offset) = self.nav.current().clone();
        let title = level_title(&level);

        // Reset header-scroll timer whenever the header text changes (level change).
        if self.last_header_title.as_deref() != Some(title.as_str()) {
            self.last_header_title = Some(title.clone());
            self.header_scroll_start = Instant::now();
        }

        let (pane_art_prev, pane_art_curr, pane_art_progress) = self.get_pane_art();
        let now_art_owned = self.now_playing.as_ref().and_then(|np| np.art.clone());

        let items_slice = self.nav.get_items();

        let now_view = self.now_playing.as_ref().map(|np| NowPlaying {
            title: &np.track.title,
            artist: &np.track.artist,
            elapsed,
            duration: np.duration,
        });

        let frame = UiFrame {
            items: items_slice,
            selected_index,
            scroll_offset,
            title: &title,
            clock: &clock,
            battery: self.system_info.battery(),
            bluetooth: self.system_info.bluetooth(),
            headphones: self.system_info.headphones(),
            now: now_view,
            now_art: now_art_owned.as_deref(),
            pane_art_prev: pane_art_prev.as_deref(),
            pane_art: pane_art_curr.as_deref(),
            pane_art_progress,
            list_scroll_elapsed: self.hover_start_time.elapsed().as_secs_f32(),
            now_scroll_elapsed: self.now_playing_start_time.elapsed().as_secs_f32(),
            header_scroll_elapsed: self.header_scroll_start.elapsed().as_secs_f32(),
            artist_scroll_elapsed: self.now_playing_start_time.elapsed().as_secs_f32(),
            is_reversing: self.pingpong_scroll,
            is_paused: self.player.is_paused() || self.player.empty(),
        };

        let buffer = self.ui.draw(frame);

        let Some(surface) = &mut self.surface else {
            return;
        };
        surface
            .resize(
                NonZeroU32::new(WIDTH).unwrap(),
                NonZeroU32::new(HEIGHT).unwrap(),
            )
            .unwrap();
        let mut surface_buffer = surface.buffer_mut().unwrap();
        surface_buffer.copy_from_slice(buffer);
        surface_buffer.present().unwrap();
    }
}
