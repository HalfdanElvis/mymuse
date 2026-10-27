use std::collections::HashMap;
use std::num::NonZeroU32;
use std::sync::Arc;
use std::time::{Duration, Instant};

use rayon::prelude::*;
use rodio::Player;
use winit::{
    application::ApplicationHandler,
    dpi::PhysicalSize,
    event::{ElementState, KeyEvent, WindowEvent},
    event_loop::ActiveEventLoop,
    keyboard::{KeyCode, PhysicalKey},
    window::Window,
};

use crate::audio;
use crate::library::{self, AlbumArt, Track};
use crate::nav::{Level, NavState};
use crate::ui::{NowPlaying, UiEngine, UiFrame};

pub const WIDTH: u32 = 320;
pub const HEIGHT: u32 = 240;
pub const SCROLL_START_TIME: f32 = 1.0;
pub const SCROLL_SPEED: f32 = 30.0;
pub const MIN_LOOP_DISTANCE: f32 = 20.0;

const SLIDESHOW_INTERVAL: Duration = Duration::from_secs(5);
const TRANSITION_DURATION: Duration = Duration::from_millis(350);

#[derive(Debug, Clone, Copy)]
pub enum UserEvent {
    Tick,
}

struct NowPlayingState {
    track: Track,
    duration: f32,
    queue: Vec<Track>,
    queue_index: usize,
    has_ended: bool,
    art: Option<Arc<AlbumArt>>,
}

struct PaneArtState {
    current: Option<(String, String)>,
    previous: Option<(String, String)>,
    context_artist: Option<String>,
    last_swap: Instant,
    transition_start: Instant,
}

impl PaneArtState {
    fn new() -> Self {
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

/// All album art, decoded once at startup. No eviction — bounded by
/// library size (~64 KB per album at 128×128 RGBA).
struct ArtCache {
    entries: HashMap<(String, String), Arc<AlbumArt>>,
}

impl ArtCache {
    fn new() -> Self {
        Self {
            entries: HashMap::new(),
        }
    }
    fn get(&self, key: &(String, String)) -> Option<Arc<AlbumArt>> {
        self.entries.get(key).cloned()
    }
    fn insert(&mut self, key: (String, String), art: Arc<AlbumArt>) {
        self.entries.insert(key, art);
    }
}

pub struct App {
    window: Option<Arc<Window>>,
    surface: Option<softbuffer::Surface<Arc<Window>, Arc<Window>>>,
    ui: UiEngine,
    player: Player,
    _audio_handle: rodio::MixerDeviceSink,
    now_playing: Option<NowPlayingState>,
    battery: f32,
    bluetooth: bool,
    headphones: bool,
    nav: NavState,
    hover_start_time: Instant,
    now_playing_start_time: Instant,
    pingpong_scroll: bool,
    pane_art: PaneArtState,
    art_cache: ArtCache,
}

impl App {
    pub fn new(
        ui: UiEngine,
        nav: NavState,
        player: Player,
        audio_handle: rodio::MixerDeviceSink,
    ) -> Self {
        let art_cache = Self::preload_album_art(&nav);

        Self {
            window: None,
            surface: None,
            ui,
            player,
            _audio_handle: audio_handle,
            now_playing: None,
            battery: 1.0,
            bluetooth: false,
            headphones: false,
            nav,
            hover_start_time: Instant::now(),
            now_playing_start_time: Instant::now(),
            pingpong_scroll: true,
            pane_art: PaneArtState::new(),
            art_cache,
        }
    }

    /// Decode every album's embedded cover up front, in parallel across all
    /// cores. Albums without embedded art are skipped silently.
    fn preload_album_art(nav: &NavState) -> ArtCache {
        let albums = nav.all_albums();
        let total = albums.len();
        eprintln!(
            "Loading album art ({} albums) on {} threads...",
            total,
            rayon::current_num_threads()
        );
        let mut cache = ArtCache::new();
        if total == 0 {
            return cache;
        }

        eprintln!("Loading album art ({} albums)...", total);
        let start = Instant::now();

        let decoded: Vec<((String, String), Arc<AlbumArt>)> = albums
            .par_iter()
            .filter_map(|(artist, album)| {
                let bytes = nav.get_album_art_bytes(artist, album)?;
                let art = library::decode_art(&bytes)?;
                Some(((artist.clone(), album.clone()), Arc::new(art)))
            })
            .collect();

        let decoded_count = decoded.len();
        for (k, a) in decoded {
            cache.insert(k, a);
        }

        eprintln!(
            "  Loaded {} / {} covers in {:?}",
            decoded_count,
            total,
            start.elapsed()
        );
        if decoded_count < total {
            eprintln!("  {} albums have no embedded cover", total - decoded_count);
        }
        cache
    }

    fn request_redraw(&self) {
        if let Some(w) = &self.window {
            w.request_redraw();
        }
    }

    fn get_art(&self, key: &(String, String)) -> Option<Arc<AlbumArt>> {
        self.art_cache.get(key)
    }

    fn get_pane_art(&self) -> (Option<Arc<AlbumArt>>, Option<Arc<AlbumArt>>, f32) {
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

    fn play_track_from_queue(&mut self, track: Track, queue: Vec<Track>, index: usize) {
        match audio::play_track(&self.player, &track.path) {
            Ok(duration) => {
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

    fn next_track_if_ended(&mut self) -> Option<(Track, Vec<Track>, usize)> {
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

    fn on_tick(&mut self) {
        if let Some((track, queue, idx)) = self.next_track_if_ended() {
            self.play_track_from_queue(track, queue, idx);
        }
        self.update_pane_art();
        self.request_redraw();
    }

    fn update_pane_art(&mut self) {
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

    fn scroll_state(
        &self,
        text: &str,
        font_size: f32,
        max_width: f32,
        scroll_gap: f32,
        start: Instant,
    ) -> Option<(f32, f32)> {
        let elapsed = start.elapsed().as_secs_f32();
        if elapsed <= SCROLL_START_TIME {
            return None;
        }
        let scroll_time = elapsed - SCROLL_START_TIME;
        let text_w = self.ui.measure_text(text, font_size);
        if text_w <= max_width {
            return None;
        }
        let loop_dist = if self.pingpong_scroll {
            text_w - max_width
        } else {
            text_w + scroll_gap
        };
        let pause_multiplier = if self.pingpong_scroll && loop_dist < MIN_LOOP_DISTANCE {
            1.0 + (1.0 - (loop_dist / MIN_LOOP_DISTANCE)) * 2.0
        } else {
            1.0
        };
        let dynamic_pause = SCROLL_START_TIME * pause_multiplier;
        let offset =
            get_textloop_offset(self.pingpong_scroll, scroll_time, loop_dist, dynamic_pause);
        Some((offset, text_w))
    }

    fn handle_key(&mut self, event_loop: &ActiveEventLoop, key_code: KeyCode) {
        match key_code {
            KeyCode::Escape => event_loop.exit(),
            KeyCode::Backspace => self.nav_back(),
            KeyCode::ArrowDown => self.nav_down(),
            KeyCode::ArrowUp => self.nav_up(),
            KeyCode::Enter => self.nav_enter(),
            _ => {}
        }
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

    fn render(&mut self) {
        let clock = clock_string();
        let elapsed = self.player.get_pos().as_secs_f32();
        let (level, selected_index, scroll_offset) = self.nav.current().clone();
        let title = level_title(&level);

        let (pane_art_prev, pane_art_curr, pane_art_progress) = self.get_pane_art();
        let now_art_owned = self.now_playing.as_ref().and_then(|np| np.art.clone());

        let items_slice = self.nav.get_items();

        let list_scroll = items_slice.get(selected_index).and_then(|text| {
            self.scroll_state(
                text,
                self.ui.layout.font_size_list,
                self.ui.layout.list_item_max_width,
                self.ui.layout.list_scroll_gap,
                self.hover_start_time,
            )
        });

        let now_scroll = self.now_playing.as_ref().and_then(|np| {
            self.scroll_state(
                &np.track.title,
                self.ui.layout.font_size_nowplay,
                self.ui.layout.nowplay_max_width,
                self.ui.layout.nowplay_scroll_gap,
                self.now_playing_start_time,
            )
        });

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
            battery: self.battery,
            bluetooth: self.bluetooth,
            headphones: self.headphones,
            now: now_view,
            now_art: now_art_owned.as_deref(),
            pane_art_prev: pane_art_prev.as_deref(),
            pane_art: pane_art_curr.as_deref(),
            pane_art_progress,
            list_scroll,
            now_scroll,
            is_reversing: self.pingpong_scroll,
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

fn clock_string() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    format!("{:02}:{:02}", (secs / 3600) % 24, (secs / 60) % 60)
}

fn level_title(level: &Level) -> String {
    match level {
        Level::Root => "MyMuse".to_string(),
        Level::Artists => "Artists".to_string(),
        Level::AlbumsOf(a) => a.clone(),
        Level::TracksOf(a, al) => format!("{} - {}", a, al),
        Level::AllAlbums => "Albums".to_string(),
        Level::AllSongs => "Songs".to_string(),
    }
}

fn pseudo_random_index(max: usize) -> usize {
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

fn get_textloop_offset(
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

impl ApplicationHandler<UserEvent> for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let window = Arc::new(
            event_loop
                .create_window(
                    Window::default_attributes()
                        .with_title("MyMuse UI")
                        .with_inner_size(PhysicalSize::new(WIDTH, HEIGHT))
                        .with_resizable(false),
                )
                .unwrap(),
        );
        let context = softbuffer::Context::new(window.clone()).unwrap();
        let surface = softbuffer::Surface::new(&context, window.clone()).unwrap();
        self.window = Some(window);
        self.surface = Some(surface);
        self.request_redraw();
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: winit::window::WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        physical_key: PhysicalKey::Code(key_code),
                        state: ElementState::Pressed,
                        ..
                    },
                ..
            } => self.handle_key(event_loop, key_code),
            WindowEvent::RedrawRequested => self.render(),
            _ => {}
        }
    }

    fn user_event(&mut self, _event_loop: &ActiveEventLoop, event: UserEvent) {
        match event {
            UserEvent::Tick => self.on_tick(),
        }
    }
}
