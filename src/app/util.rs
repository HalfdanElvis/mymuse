use crate::nav::Level;

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
