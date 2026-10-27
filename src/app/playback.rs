use std::time::Instant;

use crate::audio;
use crate::library::Track;

use super::seek::PREV_TRACK_THRESHOLD_SECS;
use super::{App, NowPlayingState};

impl App {
    pub(super) fn play_track_from_queue(&mut self, track: Track, queue: Vec<Track>, index: usize) {
        match audio::play_track(&self.player, &track.path) {
            Ok(duration) => {
                self.player.play();

                self.now_playing_start_time = Instant::now();
                let art = self.get_art(&(track.artist.clone(), track.album.clone()));
                self.now_playing = Some(NowPlayingState {
                    track,
                    duration,
                    queue,
                    queue_index: index,
                    has_ended: false,
                    art,
                });
                self.request_redraw();
            }
            Err(e) => {
                eprintln!("Error playing track: {}", e);
                if let Some(np) = &mut self.now_playing {
                    np.has_ended = true;
                }
            }
        }
    }

    pub(super) fn next_track_if_ended(&mut self) -> Option<(Track, Vec<Track>, usize)> {
        let np = self.now_playing.as_mut()?;
        if !self.player.empty() || np.has_ended {
            return None;
        }
        np.has_ended = true;
        let next_idx = np.queue_index + 1;
        if next_idx >= np.queue.len() {
            return None;
        }
        Some((np.queue[next_idx].clone(), np.queue.clone(), next_idx))
    }

    pub(super) fn tap_right(&mut self) {
        let (track, queue, next_idx) = {
            let Some(np) = &self.now_playing else { return };
            let next_idx = np.queue_index + 1;
            if next_idx >= np.queue.len() {
                return;
            }
            (np.queue[next_idx].clone(), np.queue.clone(), next_idx)
        };
        self.play_track_from_queue(track, queue, next_idx);
    }

    pub(super) fn tap_left(&mut self) {
        let pos = self.player.get_pos().as_secs_f32();

        if pos < PREV_TRACK_THRESHOLD_SECS {
            let prev = {
                let Some(np) = &self.now_playing else { return };
                if np.queue_index == 0 {
                    None
                } else {
                    let prev_idx = np.queue_index - 1;
                    Some((np.queue[prev_idx].clone(), np.queue.clone(), prev_idx))
                }
            };
            if let Some((track, queue, prev_idx)) = prev {
                self.play_track_from_queue(track, queue, prev_idx);
                return;
            }
        }

        self.seek_to_secs(0.0);
    }
}
