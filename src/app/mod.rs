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

        let mut app = Self {
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
            pane_art: PaneArtState::new(),
            art_cache,
            seek_hold: None,
        };

        // Populate pane art before the first render so we never draw the
        // placeholder box while waiting for the first tick.
        app.update_pane_art();

        app
    }

    fn request_redraw(&self) {
        if let Some(w) = &self.window {
            w.request_redraw();
        }
    }

    fn get_art(&self, key: &(String, String)) -> Option<Arc<AlbumArt>> {
        self.art_cache.get(key)
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
                    if repeat && matches!(key_code, KeyCode::ArrowLeft | KeyCode::ArrowRight) {
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
