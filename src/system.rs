// Hardware state that the UI displays but the player doesn't depend on.
//
// `LinuxSystemInfo` reads the battery level from sysfs. On Pop!_OS / Ubuntu
// this is `/sys/class/power_supply/BAT*/capacity` (an integer 0..100).
// No caching: a sysfs read is ~4 bytes from a virtual file, cheap enough at
// 30 fps. Add a TTL cache later if you ever need it.
//
// `MockSystemInfo` stays for UI previews and tests.
//
// Still stubbed (need the real device to test):
//   - bluetooth:  BlueZ over D-Bus, or a check like `bluetoothctl show`
//   - headphones: ALSA jack status (format varies per SoC / codec)

use std::fs;
use std::path::PathBuf;

pub trait SystemInfo {
    /// 0.0 ..= 1.0.
    fn battery(&self) -> f32;
    fn bluetooth(&self) -> bool;
    fn headphones(&self) -> bool;
}

// --- Linux ------------------------------------------------------------------

pub struct LinuxSystemInfo {
    /// `/sys/class/power_supply/BAT*/capacity` if a battery is present.
    /// `None` on a desktop / a system with no readable battery.
    capacity_path: Option<PathBuf>,
}

impl LinuxSystemInfo {
    pub fn new() -> Self {
        Self {
            capacity_path: find_battery_capacity_path(),
        }
    }
}

impl Default for LinuxSystemInfo {
    fn default() -> Self {
        Self::new()
    }
}

impl SystemInfo for LinuxSystemInfo {
    fn battery(&self) -> f32 {
        let Some(path) = &self.capacity_path else {
            // No battery (desktop). Report "full" so the icon is at least
            // not misleadingly empty; if you want a "no battery" visual,
            // that's a UI change, not a system-info one.
            return 1.0;
        };
        match fs::read_to_string(path) {
            Ok(s) => s
                .trim()
                .parse::<f32>()
                .map(|v| (v / 100.0).clamp(0.0, 1.0))
                .unwrap_or(1.0),
            Err(_) => 1.0,
        }
    }

    fn bluetooth(&self) -> bool {
        // TODO: read from BlueZ once the target device is available.
        false
    }

    fn headphones(&self) -> bool {
        // TODO: read ALSA jack status once the target device is available.
        false
    }
}

/// Find the first `/sys/class/power_supply/BAT*/capacity` file on the system.
/// Returns `None` if there is no battery (desktop) or the directory can't be
/// read for any reason.
fn find_battery_capacity_path() -> Option<PathBuf> {
    let entries = fs::read_dir("/sys/class/power_supply").ok()?;
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name.starts_with("BAT") {
            let cap = entry.path().join("capacity");
            if cap.exists() {
                return Some(cap);
            }
        }
    }
    None
}
