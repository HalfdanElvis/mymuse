// src/app.rs
use std::num::NonZeroU32;
use std::sync::Arc;

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

#[derive(Debug, Clone, Copy)]
pub enum UserEvent {
    Tick,
}

struct NowPlayingState {
    title: String,
    artist: String,
    duration: f32,
}

pub struct App {
    window: Option<Arc<Window>>,
    surface: Option<softbuffer::Surface<Arc<Window>, Arc<Window>>>,
    ui: UiEngine,
    player: Player,
    _audio_handle: rodio::MixerDeviceSink,
    header_title: String,
    now_playing: Option<NowPlayingState>,
    battery: f32,
    bluetooth: bool,
    headphones: bool,
    nav: NavState,
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
            header_title: "MyMuse".to_string(),
            now_playing: None,
            battery: 1.0,
            bluetooth: false,
            headphones: false,
            nav,
        }
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
        self.window.as_ref().unwrap().request_redraw();
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
            } => match key_code {
                KeyCode::Escape => event_loop.exit(),
                KeyCode::Backspace => {
                    if !self.nav.back() {
                        //Maybe use this to go into sleep mode or let it be a toggle in settings
                    } else {
                        self.window.as_ref().unwrap().request_redraw();
                    }
                }
                KeyCode::ArrowDown => {
                    let (_, idx) = self.nav.current();
                    let items_len = self.nav.get_items().len();

                    if *idx < items_len.saturating_sub(1) {
                        if let Some((_, idx_mut)) = self.nav.level_stack.last_mut() {
                            *idx_mut += 1;
                        }
                        self.window.as_ref().unwrap().request_redraw();
                    }
                }
                KeyCode::ArrowUp => {
                    let (_, idx) = self.nav.current();

                    if *idx > 0 {
                        if let Some((_, idx_mut)) = self.nav.level_stack.last_mut() {
                            *idx_mut -= 1;
                        }
                        self.window.as_ref().unwrap().request_redraw();
                    }
                }
                KeyCode::Enter => {
                    // Find selected
                    //
                    if let Some(track_path) = self.nav.enter() {
                        match audio::play_track(&self.player, &track_path) {
                            Ok(info) => {
                                println!("Playing: {}", info.title);

                                // 3. Update the UI state

                                let artist = match &self.nav.current().0 {
                                    Level::TracksOf(artist, _) => artist.clone(),
                                    _ => "Unknown Artist".to_string(),
                                };

                                self.now_playing = Some(NowPlayingState {
                                    title: info.title,
                                    artist,
                                    duration: info.duration,
                                });
                            }
                            Err(e) => {
                                eprintln!("Error playing track: {}", e);
                            }
                        }
                    }
                    self.window.as_ref().unwrap().request_redraw();
                }
                _ => {}
            },
            WindowEvent::RedrawRequested => {
                if let (Some(window), Some(surface)) = (&self.window, &mut self.surface) {
                    let clock = clock_string();
                    let elapsed = self.player.get_pos().as_secs_f32();

                    // Fetch data from Navstate and generate title based on curr level
                    let items = self.nav.get_items();
                    let selected_index = self.nav.current().1;

                    let title = match &self.nav.current().0 {
                        Level::Root => "MyMuse".to_string(),
                        Level::Artists => "Artists".to_string(),
                        Level::AlbumsOf(a) => a.clone(),
                        Level::TracksOf(a, al) => {
                            format!("{} - {}", a, al)
                        }
                        Level::AllAlbums => "Albums".to_string(),
                        Level::AllSongs => "Songs".to_string(),
                    };

                    let frame = UiFrame {
                        items: &items,
                        selected_index,
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
                    };

                    let buffer = self.ui.draw(frame);

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
            _ => {}
        }
    }

    fn user_event(&mut self, _event_loop: &ActiveEventLoop, event: UserEvent) {
        if let UserEvent::Tick = event {
            // Removed the `is_some()` check so the clock ticks even when paused.
            if let Some(window) = &self.window {
                window.request_redraw();
            }
        }
    }
}
