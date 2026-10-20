mod library;
use rodio::{Decoder, DeviceSinkBuilder, Player};
use std::fs::File;
use std::io::BufReader;

fn main() {
    let music_dir = "./test_music";
    println!("scanning directory: {}", music_dir);

    let lib = library::scan_and_build_library(music_dir);

    // Visualising Lib
    if lib.is_empty() {
        println!("No music found!");
        return;
    }

    println!("--- Library Breakdown ---");
    for (artist, albums) in &lib {
        println!("Artist: {}", artist);
        for (album, tracks) in albums {
            println!("  Album: {} ({} tracks)", album, tracks.len());
        }
    }
    println!("--------------------------");

    // Playing the music
    // currently playing first track on first album

    let handle = DeviceSinkBuilder::open_default_sink().expect("Failed to get audio output");
    let player = Player::connect_new(&handle.mixer());

    if let Some((artist, albums)) = lib.iter().next() {
        if let Some((album, tracks)) = albums.iter().next() {
            let track = &tracks[0];
            println!(
                "Playing: {} - {} - {:?}",
                artist,
                album,
                track.file_name().unwrap()
            );

            let file = BufReader::new(File::open(track).expect("Failed to open file"));
            let source = Decoder::new(file).expect("Failed to decode file");
            player.append(source);
            player.sleep_until_end();
        }
    }
}
