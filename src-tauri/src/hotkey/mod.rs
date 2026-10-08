pub mod manager;
pub use manager::{HotkeyManager, parse_hotkey};

#[cfg(target_os = "windows")]
mod caps_lock;
