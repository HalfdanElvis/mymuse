// src/main.rs
mod app;
mod audio;
mod library;
mod nav;
mod system;
mod ui;

use std::time::Duration;

use rodio::{DeviceSinkBuilder, Player};
use winit::event_loop::EventLoop;

use app::{App, HEIGHT, UserEvent, WIDTH};
use nav::NavState;
use system::{LinuxSystemInfo, SystemInfo};
use ui::UiEngine;

fn main() {
    let music_dir = "./test_music";
    let lib = library::scan_and_build_library(music_dir);

    if lib.is_empty() {
        println!("No music found!");
        return;
    }

    // Audio setup
    let handle = DeviceSinkBuilder::open_default_sink().expect("Failed to get audio output");
    let player = Player::connect_new(&handle.mixer());

    // UI Engine setup
    let ui = UiEngine::new(WIDTH as usize, HEIGHT as usize);

    // Navigation setup (Build the state tree!)
    let nav = NavState::new(lib);

    // Hardware info. Swap back to `MockSystemInfo` for UI previews.
    let system_info: Box<dyn SystemInfo> = Box::new(LinuxSystemInfo::new());

    // EventLoop setup
    let event_loop = EventLoop::<UserEvent>::with_user_event().build().unwrap();
    let proxy = event_loop.create_proxy();

    std::thread::spawn(move || {
        loop {
            std::thread::sleep(Duration::from_millis(33));
            if proxy.send_event(UserEvent::Tick).is_err() {
                break;
            }
        }
    });

    // Create the App
    let mut app = App::new(ui, nav, player, handle, system_info);

    // Run the app
    event_loop.run_app(&mut app).unwrap();
}
