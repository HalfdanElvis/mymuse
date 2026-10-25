use std::num::NonZeroU32;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

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
use crate::nav::{Level, NavState};
use crate::ui::{NowPlaying, UiEngine, UiFrame};

pub const WIDTH: u32 = 320;
pub const HEIGHT: u32 = 240;
pub const SCROLL_START_TIME: f32 = 1.0;
pub const SCROLL_SPEED: f32 = 30.0;
pub const MIN_LOOP_DISTANCE: f32 = 20.0;

#[derive(Debug, Clone, Copy)]
pub enum UserEvent {
    Tick,
}

struct NowPlayingState {
    title: String,
    artist: String,
    duration: f32,
    queue: Vec<PathBuf>,
    queue_index: usize,
    has_ended: bool,
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
    pub pingpong_scroll: bool,
}

impl App {
    pub fn new(
        ui: UiEngine,
        nav: NavState,
        player: Player,
        audio_handle: rodio::MixerDeviceSink,
    ) -> Self {
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
        }
    }

    fn request_redraw(&self) {
        if let Some(w) = &self.window {
            w.request_redraw();
        }
    }

    fn play_track_from_queue(&mut self, path: PathBuf, queue: Vec<PathBuf>, index: usize) {
        match audio::play_track(&self.player, &path) {
            Ok(info) => {
                println!("Playing: {}", info.title);
                self.now_playing_start_time = Instant::now();
                self.now_playing = Some(NowPlayingState {
                    title: info.title,
                    artist: info.artist,
                    duration: info.duration,
                    queue,
                    queue_index: index,
                    has_ended: false,
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

    /// Computes marquee scroll state for a single line of text. Returns
    /// `Some((offset, text_width))` when the text overflows its container
    /// and the start-delay has elapsed, otherwise `None`.
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

    // --- Key handling -----------------------------------------------------

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

        if let Some(track_path) = self.nav.enter() {
            let queue = self.nav.get_current_playlist();
            let queue_index = queue.iter().position(|p| p == &track_path).unwrap_or(0);
            self.play_track_from_queue(track_path, queue, queue_index);
        }
        self.request_redraw();
    }

    // --- Tick / queue advance --------------------------------------------

    fn on_tick(&mut self) {
        if let Some((path, queue, idx)) = self.next_track_if_ended() {
            self.play_track_from_queue(path, queue, idx);
        }
        self.request_redraw();
    }

    /// Returns the next queued track if the current one has just ended.
    fn next_track_if_ended(&mut self) -> Option<(PathBuf, Vec<PathBuf>, usize)> {
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

    // --- Rendering --------------------------------------------------------

    fn render(&mut self) {
        let clock = clock_string();
        let elapsed = self.player.get_pos().as_secs_f32();
        let items = self.nav.get_items();
        let (level, selected_index, scroll_offset) = self.nav.current().clone();
        let title = level_title(&level);

        let list_scroll = items.get(selected_index).and_then(|text| {
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
                &np.title,
                self.ui.layout.font_size_nowplay,
                self.ui.layout.nowplay_max_width,
                self.ui.layout.nowplay_scroll_gap,
                self.now_playing_start_time,
            )
        });

        let frame = UiFrame {
            items: &items,
            selected_index,
            scroll_offset,
            title: &title,
            clock: &clock,
            battery: self.battery,
            bluetooth: self.bluetooth,
            headphones: self.headphones,
            now: self.now_playing.as_ref().map(|np| NowPlaying {
                title: &np.title,
                artist: &np.artist,
                elapsed,
                duration: np.duration,
            }),
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
