use crate::nav::Level;

use super::SCROLL_SPEED;

pub(super) fn clock_string() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    format!("{:02}:{:02}", (secs / 3600) % 24, (secs / 60) % 60)
}

pub(super) fn level_title(level: &Level) -> String {
    match level {
        Level::Root => "MyMuse".to_string(),
        Level::Artists => "Artists".to_string(),
        Level::AlbumsOf(a) => a.clone(),
        Level::TracksOf(a, al) => format!("{} - {}", a, al),
        Level::AllAlbums => "Albums".to_string(),
        Level::AllSongs => "Songs".to_string(),
    }
}

pub(super) fn pseudo_random_index(max: usize) -> usize {
    use std::time::{SystemTime, UNIX_EPOCH};
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0);
    let mut x = now;
    x ^= x >> 30;
    x = x.wrapping_mul(0xBF58476D1CE4E5B9);
    x ^= x >> 27;
    x = x.wrapping_mul(0x94D049BB133111EB);
    x ^= x >> 31;
    (x as usize) % max.max(1)
}

pub(super) fn get_textloop_offset(
    pingpong_scroll: bool,
    scroll_time: f32,
    loop_dist: f32,
    pause_time: f32,
) -> f32 {
    if pingpong_scroll {
        let travel_time = loop_dist / SCROLL_SPEED;
        let cycle = (travel_time + pause_time) * 2.0;
        let t = scroll_time % cycle;
        if t < travel_time {
            t * SCROLL_SPEED
        } else if t < travel_time + pause_time {
            loop_dist
        } else if t < travel_time * 2.0 + pause_time {
            loop_dist - ((t - travel_time - pause_time) * SCROLL_SPEED)
        } else {
            0.0
        }
    } else {
        (scroll_time * SCROLL_SPEED) % loop_dist
    }
}
