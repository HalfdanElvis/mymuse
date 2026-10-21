// src/main.rs
mod app;
mod audio;
mod library;
mod ui;

use std::path::PathBuf;
use std::time::Duration;

use rodio::{DeviceSinkBuilder, Player};
use winit::event_loop::EventLoop;

use app::{App, UserEvent, HEIGHT, WIDTH};
use ui::UiEngine;

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
            if proxy.send_event(UserEvent::Tick).is_err() {
                break;
            }
        }
    });

    // Create the App using our new constructor
    let mut app = App::new(ui, menu_items, album_tracks, player, handle);

    // Run the app
    event_loop.run_app(&mut app).unwrap();
}
