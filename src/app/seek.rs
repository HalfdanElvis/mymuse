use std::time::{Duration, Instant};

use winit::keyboard::KeyCode;

use crate::audio;

use super::{App, NowPlayingState};

pub(super) const HOLD_THRESHOLD: Duration = Duration::from_millis(500);
pub(super) const HOLD_REPEAT_INTERVAL: Duration = Duration::from_millis(500);
pub(super) const SEEK_STEP_SECS: f32 = 5.0;
pub(super) const PREV_TRACK_THRESHOLD_SECS: f32 = 3.0;

pub(super) struct SeekHold {
    key: KeyCode,
    started: Instant,
    last_action: Option<Instant>,
    hold_triggered: bool,
}

impl App {
    pub(super) fn start_seek_hold(&mut self, key: KeyCode) {
        self.seek_hold = Some(SeekHold {
            key,
            started: Instant::now(),
            last_action: None,
            hold_triggered: false,
        });
    }

    pub(super) fn process_seek_hold(&mut self) {
        let dir = {
            let Some(hold) = &mut self.seek_hold else {
                return;
            };
            if hold.started.elapsed() < HOLD_THRESHOLD {
                return;
            }
            let due = match hold.last_action {
                None => true,
                Some(t) => t.elapsed() >= HOLD_REPEAT_INTERVAL,
            };
            if !due {
                return;
            }
            hold.hold_triggered = true;
            hold.last_action = Some(Instant::now());
            if hold.key == KeyCode::ArrowRight {
                1.0
            } else {
                -1.0
            }
        };
        self.seek_relative(dir * SEEK_STEP_SECS);
        self.request_redraw();
    }

    pub(super) fn handle_seek_release(&mut self, key: KeyCode) {
        let Some(hold) = &self.seek_hold else { return };
        if hold.key != key {
            return;
        }
        let hold = self.seek_hold.take().unwrap();
        let held_for = hold.started.elapsed();

        if held_for < HOLD_THRESHOLD {
            match key {
                KeyCode::ArrowRight => self.tap_right(),
                KeyCode::ArrowLeft => self.tap_left(),
                _ => {}
            }
        } else if !hold.hold_triggered {
            let dir = if key == KeyCode::ArrowRight { 1.0 } else { -1.0 };
            self.seek_relative(dir * SEEK_STEP_SECS);
            self.request_redraw();
        }
    }

    pub(super) fn seek_to_secs(&mut self, secs: f32) {
        if self.now_playing.is_none() {
            return;
        }
        let target = secs.max(0.0);

        if let Err(_e) = self.player.try_seek(Duration::from_secs_f32(target)) {
            self.reload_current_at(target);
        }
        self.request_redraw();
    }

    fn seek_relative(&mut self, delta_secs: f32) {
        let duration = match &self.now_playing {
            Some(np) if np.duration > 0.0 => np.duration,
            _ => return,
        };
        let pos = self.player.get_pos().as_secs_f32();
        let target = (pos + delta_secs).clamp(0.0, duration);
        self.seek_to_secs(target);
    }

    fn reload_current_at(&mut self, position: f32) {
        let (track, queue, index, art) = {
            let Some(np) = &self.now_playing else { return };
            (
                np.track.clone(),
                np.queue.clone(),
                np.queue_index,
                np.art.clone(),
            )
        };

        match audio::play_track(&self.player, &track.path) {
            Ok(duration) => {
                self.player.play();

                self.now_playing = Some(NowPlayingState {
                    track,
                    duration,
                    queue,
                    queue_index: index,
                    has_ended: false,
                    art,
                });

                if position > 0.0 {
                    let target = position.clamp(0.0, duration);
                    if let Err(e) = self.player.try_seek(Duration::from_secs_f32(target)) {
                        eprintln!("Forward seek after reload failed ({}); starting from 0", e);
                    }
                }

                self.now_playing_start_time = Instant::now();
                self.request_redraw();
            }
            Err(e) => eprintln!("Reload failed: {}", e),
        }
    }
}
