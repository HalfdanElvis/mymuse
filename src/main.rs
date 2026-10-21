mod library;
mod ui;

use std::io::BufReader;
use std::num::NonZeroU32;
use std::path::PathBuf;
use std::sync::Arc;
use std::{fs::File, time::Duration};

use rodio::{Decoder, DeviceSinkBuilder, Player, Source, source};
use ui::UiEngine;
use winit::{
    application::ApplicationHandler,
    dpi::PhysicalSize,
    event::{ElementState, KeyEvent, WindowEvent},
    event_loop::{ActiveEventLoop, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
    window::Window,
};

const WIDTH: u32 = 320;
const HEIGHT: u32 = 240;

#[derive(Debug, Clone, Copy)]
pub enum UserEvent {
    Tick,
}

struct NowPlayingState {
    title: String,
    artist: String,
    duration: f32,
}

// Holds all the application state, including the window and surface
struct App {
    window: Option<Arc<Window>>,
    surface: Option<softbuffer::Surface<Arc<Window>, Arc<Window>>>,
    ui: UiEngine,
    menu_items: Vec<String>,
    album_tracks: Vec<Vec<PathBuf>>,
    selected_index: usize,
    player: Player,
    _audio_handle: rodio::MixerDeviceSink,
    header_title: String,
    now_playing: Option<NowPlayingState>,
    battery: f32,
    bluetooth: bool,
    headphones: bool,
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
        // Wrap the window in an Arc so both the App and softbuffer can hold it
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
                KeyCode::ArrowDown => {
                    if self.selected_index < self.menu_items.len() - 1 {
                        self.selected_index += 1;
                        self.window.as_ref().unwrap().request_redraw();
                    }
                }
                KeyCode::ArrowUp => {
                    if self.selected_index > 0 {
                        self.selected_index -= 1;
                        self.window.as_ref().unwrap().request_redraw();
                    }
                }
                KeyCode::Enter => {
                    if let Some(tracks) = self.album_tracks.get(self.selected_index) {
                        if let Some(track_path) = tracks.first() {
                            if let Ok(file) = File::open(track_path) {
                                if let Ok(source) = Decoder::new(BufReader::new(file)) {
                                    let duration = source
                                        .total_duration()
                                        .map(|d| d.as_secs_f32())
                                        .unwrap_or(0.0);

                                    println!("Playing: {:?}", track_path.file_name().unwrap());
                                    self.player.stop();
                                    self.player.append(source);

                                    let title = track_path
                                        .file_stem()
                                        .and_then(|s| s.to_str())
                                        .unwrap_or("Unkown")
                                        .to_string();

                                    let artist = self.menu_items[self.selected_index]
                                        .split(" - ")
                                        .next()
                                        .unwrap_or("")
                                        .to_string();

                                    self.now_playing = Some(NowPlayingState {
                                        title,
                                        artist,
                                        duration,
                                    });
                                    self.window.as_ref().unwrap().request_redraw();
                                }
                            }
                        }
                    }
                }
                _ => {}
            },
            WindowEvent::RedrawRequested => {
                if let (Some(window), Some(surface)) = (&self.window, &mut self.surface) {
                    let clock = clock_string();
                    let elapsed = self.player.get_pos().as_secs_f32();

                    let frame = ui::UiFrame {
                        items: &self.menu_items,
                        selected_index: self.selected_index,
                        title: &self.header_title,
                        clock: &clock,
                        battery: self.battery,
                        bluetooth: self.bluetooth,
                        headphones: self.headphones,
                        now: self.now_playing.as_ref().map(|np| ui::NowPlaying {
                            title: &np.title,
                            artist: &np.artist,
                            elapsed,
                            duration: np.duration,
                        }),
                    };

                    // 1. Get the rendered pixels from the engine
                    let buffer = self.ui.draw(frame);

                    // 2. Resize the surface (requires NonZero)
                    surface
                        .resize(
                            NonZeroU32::new(WIDTH).unwrap(),
                            NonZeroU32::new(HEIGHT).unwrap(),
                        )
                        .unwrap();

                    // 3. Copy pixels to the window's surface buffer and present
                    let mut surface_buffer = surface.buffer_mut().unwrap();
                    surface_buffer.copy_from_slice(buffer);
                    surface_buffer.present().unwrap();

                    // Keep requesting redraws for smooth behavior (can be optimized later)
                    // window.request_redraw();
                }
            }
            _ => {}
        }
    }
    fn user_event(&mut self, _event_loop: &ActiveEventLoop, event: UserEvent) {
        if self.now_playing.is_some() {
            if let Some(window) = &self.window {
                window.request_redraw();
            }
        }
    }
}

fn main() {
    let music_dir = "./test_music";
    let lib = library::scan_and_build_library(music_dir);

    let mut menu_items: Vec<String> = Vec::new();
    let mut album_tracks: Vec<Vec<PathBuf>> = Vec::new();

    if lib.is_empty() {
        println!("No music found!");
        return;
    }

    for (artist, albums) in &lib {
        for (album, tracks) in albums {
            menu_items.push(format!("{} - {}", artist, album));
            album_tracks.push(tracks.clone());
        }
    }

    // Audio setup
    let handle = DeviceSinkBuilder::open_default_sink().expect("Failed to get audio output");
    let player = Player::connect_new(&handle.mixer());

    // UI Engine setup
    let ui = UiEngine::new(WIDTH as usize, HEIGHT as usize);

    // EventLoop setup
    let event_loop = EventLoop::<UserEvent>::with_user_event().build().unwrap();
    let proxy = event_loop.create_proxy();

    // Spawn a background thread that sends a tick every 250ms
    std::thread::spawn(move || {
        loop {
            std::thread::sleep(Duration::from_millis(250));
            // Break loop if window is closed (will be changed to device turned off // sleep mode
            if proxy.send_event(UserEvent::Tick).is_err() {
                break;
            }
        }
    });

    // Winit event loop setup
    let mut app = App {
        window: None,
        surface: None,
        ui,
        menu_items,
        album_tracks,
        selected_index: 0,
        player,
        _audio_handle: handle,
        header_title: "MyMuse".to_string(),
        now_playing: None,
        battery: 1.0,
        bluetooth: false,
        headphones: false,
    };

    // This blocks and runs the app
    event_loop.run_app(&mut app).unwrap();
}
