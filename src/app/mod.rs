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

use crate::library::{AlbumArt, Track};
use crate::nav::NavState;
use crate::system::SystemInfo;
use crate::ui::UiEngine;

mod art_cache;
mod nav_events;
mod playback;
mod render;
mod seek;
mod slideshow;
mod util;

use art_cache::ArtCache;
use seek::SeekHold;
use slideshow::PaneArtState;

pub const WIDTH: u32 = 320;
pub const HEIGHT: u32 = 240;
pub(super) const SCROLL_START_TIME: f32 = 1.0;
pub(super) const SCROLL_SPEED: f32 = 30.0;
pub(super) const MIN_LOOP_DISTANCE: f32 = 20.0;

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

pub struct App {
    window: Option<Arc<Window>>,
    surface: Option<softbuffer::Surface<Arc<Window>, Arc<Window>>>,
    ui: UiEngine,
    player: Player,
    _audio_handle: rodio::MixerDeviceSink,
    now_playing: Option<NowPlayingState>,
    system_info: Box<dyn SystemInfo>,
    nav: NavState,
    hover_start_time: Instant,
    now_playing_start_time: Instant,
    header_scroll_start: Instant,
    last_header_title: Option<String>,
    pingpong_scroll: bool,
    pane_art: PaneArtState,
    art_cache: ArtCache,
    seek_hold: Option<SeekHold>,
}

impl App {
    pub fn new(
        ui: UiEngine,
        nav: NavState,
        player: Player,
        audio_handle: rodio::MixerDeviceSink,
        system_info: Box<dyn SystemInfo>,
    ) -> Self {
        let art_cache = Self::preload_album_art(&nav);

        Self {
            window: None,
            surface: None,
            ui,
            player,
            _audio_handle: audio_handle,
            now_playing: None,
            system_info,
            nav,
            hover_start_time: Instant::now(),
            now_playing_start_time: Instant::now(),
            header_scroll_start: Instant::now(),
            last_header_title: None,
            pingpong_scroll: true,
            pane_art: PaneArtState::new(),
            art_cache,
            seek_hold: None,
        }
    }

    fn request_redraw(&self) {
        if let Some(w) = &self.window {
            w.request_redraw();
        }
    }

    fn get_art(&self, key: &(String, String)) -> Option<Arc<AlbumArt>> {
        self.art_cache.get(key)
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
            util::get_textloop_offset(self.pingpong_scroll, scroll_time, loop_dist, dynamic_pause);
        Some((offset, text_w))
    }

    fn on_tick(&mut self) {
        self.process_seek_hold();
        if let Some((track, queue, idx)) = self.next_track_if_ended() {
            self.play_track_from_queue(track, queue, idx);
        }
        self.update_pane_art();
        self.request_redraw();
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
                        state,
                        repeat,
                        ..
                    },
                ..
            } => match state {
                ElementState::Pressed => {
                    if repeat
                        && matches!(key_code, KeyCode::ArrowLeft | KeyCode::ArrowRight)
                    {
                        return;
                    }
                    self.handle_key(event_loop, key_code);
                }
                ElementState::Released => self.handle_seek_release(key_code),
            },
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
