use global_hotkey::{
    hotkey::{Code, HotKey, Modifiers},
    GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState,
};
use std::sync::mpsc;
use std::thread;
use tauri::Emitter;

#[cfg(target_os = "windows")]
use super::caps_lock::CapsLockHook;

#[cfg(target_os = "windows")]
use windows::Win32::UI::WindowsAndMessaging::{
    DispatchMessageW, PeekMessageW, TranslateMessage, MSG, PM_REMOVE,
};

// Commands to send to the hotkey thread
#[allow(dead_code)]
enum HotkeyCommand {
    Register(Modifiers, Code, mpsc::Sender<Result<(), String>>),
    Unregister(mpsc::Sender<Result<(), String>>),
}

pub struct HotkeyManager {
    command_sender: mpsc::Sender<HotkeyCommand>,
}

// Safe because we only communicate via channels
unsafe impl Send for HotkeyManager {}
unsafe impl Sync for HotkeyManager {}

impl HotkeyManager {
    pub fn new(app_handle: tauri::AppHandle) -> Result<Self, String> {
        let (tx, rx) = mpsc::channel::<HotkeyCommand>();
        let (ready_tx, ready_rx) = mpsc::channel();

        // Spawn a dedicated thread for hotkey management
        thread::spawn(move || {
            let manager = match GlobalHotKeyManager::new() {
                Ok(m) => m,
                Err(e) => {
                    let _ =
                        ready_tx.send(Err(format!("Failed to create GlobalHotKeyManager: {}", e)));
                    return;
                }
            };
            if ready_tx.send(Ok(())).is_err() {
                return;
            }

            let event_receiver = GlobalHotKeyEvent::receiver();
            let mut current_hotkey: Option<HotKey> = None;
            let mut current_binding = None;
            let mut pressed = false;
            #[cfg(target_os = "windows")]
            let mut caps_hook: Option<CapsLockHook> = None;

            let emit_event = |state: HotKeyState| {
                let name = match state {
                    HotKeyState::Pressed => "hotkey-pressed",
                    HotKeyState::Released => "hotkey-released",
                };
                if let Err(error) = app_handle.emit(name, ()) {
                    log::error!("Failed to emit {}: {}", name, error);
                }
            };

            loop {
                // Pump Windows messages (required for global hotkeys to work)
                #[cfg(target_os = "windows")]
                unsafe {
                    let mut msg: MSG = std::mem::zeroed();
                    while PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE).as_bool() {
                        let _ = TranslateMessage(&msg);
                        DispatchMessageW(&msg);
                    }
                }

                // Check for hotkey events (non-blocking)
                while let Ok(event) = event_receiver.try_recv() {
                    if current_hotkey
                        .as_ref()
                        .is_some_and(|hotkey| hotkey.id() == event.id)
                    {
                        pressed = event.state == HotKeyState::Pressed;
                        emit_event(event.state);
                    }
                }
                #[cfg(target_os = "windows")]
                if let Some(hook) = &caps_hook {
                    while let Ok(state) = hook.events.try_recv() {
                        pressed = state == HotKeyState::Pressed;
                        emit_event(state);
                    }
                }

                // Check for commands (non-blocking)
                match rx.try_recv() {
                    Ok(HotkeyCommand::Register(modifiers, key, response_tx)) => {
                        let result = (|| {
                            if current_binding == Some((modifiers, key)) {
                                return Ok(());
                            }
                            if pressed {
                                return Err(
                                    "Release the recording shortcut before changing it".to_string()
                                );
                            }
                            let caps_lock = key == Code::CapsLock && modifiers.is_empty();
                            #[cfg(not(target_os = "windows"))]
                            if caps_lock {
                                return Err(
                                    "Caps Lock recording is currently supported only on Windows"
                                        .to_string(),
                                );
                            }
                            #[cfg(target_os = "windows")]
                            let new_caps_hook = if caps_lock {
                                Some(CapsLockHook::new()?)
                            } else {
                                None
                            };
                            let new_hotkey = if caps_lock {
                                None
                            } else {
                                let hotkey = HotKey::new(Some(modifiers), key);
                                manager.register(hotkey).map_err(|error| {
                                    format!("Failed to register hotkey: {}", error)
                                })?;
                                Some(hotkey)
                            };

                            // Keep the previous shortcut working if the replacement cannot be registered.
                            if let Some(old_hotkey) = current_hotkey {
                                if let Err(error) = manager.unregister(old_hotkey) {
                                    if let Some(hotkey) = new_hotkey {
                                        if let Err(rollback_error) = manager.unregister(hotkey) {
                                            log::error!(
                                                "Failed to unregister replacement hotkey: {}",
                                                rollback_error
                                            );
                                        }
                                    }
                                    return Err(format!(
                                        "Failed to unregister previous hotkey: {}",
                                        error
                                    ));
                                }
                            }
                            current_hotkey = new_hotkey;
                            #[cfg(target_os = "windows")]
                            {
                                caps_hook = new_caps_hook;
                            }
                            current_binding = Some((modifiers, key));
                            log::info!("Hotkey registered: {:?} + {:?}", modifiers, key);
                            Ok(())
                        })();
                        if let Err(error) = &result {
                            log::error!("{}", error);
                        }
                        let _ = response_tx.send(result);
                    }
                    Ok(HotkeyCommand::Unregister(response_tx)) => {
                        let result = (|| {
                            if pressed {
                                return Err(
                                    "Release the recording shortcut before unregistering it"
                                        .to_string(),
                                );
                            }
                            if let Some(hotkey) = current_hotkey {
                                manager.unregister(hotkey).map_err(|error| {
                                    format!("Failed to unregister hotkey: {}", error)
                                })?;
                            }
                            current_hotkey = None;
                            current_binding = None;
                            #[cfg(target_os = "windows")]
                            {
                                caps_hook = None;
                            }
                            Ok(())
                        })();
                        let _ = response_tx.send(result);
                    }
                    Err(mpsc::TryRecvError::Disconnected) => {
                        // Channel closed, exit thread
                        break;
                    }
                    Err(mpsc::TryRecvError::Empty) => {
                        // No command, continue
                    }
                }

                // Small sleep to prevent busy-waiting
                thread::sleep(std::time::Duration::from_millis(10));
            }
        });

        ready_rx
            .recv()
            .map_err(|error| format!("Failed to initialize hotkey manager: {}", error))??;
        Ok(Self { command_sender: tx })
    }

    pub async fn register(&mut self, modifiers: Modifiers, key: Code) -> Result<(), String> {
        let (response_tx, response_rx) = mpsc::channel();
        self.command_sender
            .send(HotkeyCommand::Register(modifiers, key, response_tx))
            .map_err(|e| format!("Failed to send register command: {}", e))?;

        response_rx
            .recv()
            .map_err(|e| format!("Failed to receive register response: {}", e))?
    }

    #[allow(dead_code)]
    pub async fn unregister(&mut self) -> Result<(), String> {
        let (response_tx, response_rx) = mpsc::channel();
        self.command_sender
            .send(HotkeyCommand::Unregister(response_tx))
            .map_err(|e| format!("Failed to send unregister command: {}", e))?;

        response_rx
            .recv()
            .map_err(|e| format!("Failed to receive unregister response: {}", e))?
    }
}

pub fn parse_modifier(modifier: &str) -> Option<Modifiers> {
    match modifier.to_lowercase().as_str() {
        "none" => Some(Modifiers::empty()),
        "ctrl" | "control" => Some(Modifiers::CONTROL),
        "alt" => Some(Modifiers::ALT),
        "shift" => Some(Modifiers::SHIFT),
        "super" | "win" | "cmd" | "meta" => Some(Modifiers::SUPER),
        _ => None,
    }
}

pub fn parse_key(key_str: &str) -> Option<Code> {
    match key_str.to_uppercase().as_str() {
        "F1" => Some(Code::F1),
        "F2" => Some(Code::F2),
        "F3" => Some(Code::F3),
        "F4" => Some(Code::F4),
        "F5" => Some(Code::F5),
        "F6" => Some(Code::F6),
        "F7" => Some(Code::F7),
        "F8" => Some(Code::F8),
        "F9" => Some(Code::F9),
        "F10" => Some(Code::F10),
        "F11" => Some(Code::F11),
        "F12" => Some(Code::F12),
        "A" => Some(Code::KeyA),
        "B" => Some(Code::KeyB),
        "C" => Some(Code::KeyC),
        "D" => Some(Code::KeyD),
        "E" => Some(Code::KeyE),
        "F" => Some(Code::KeyF),
        "G" => Some(Code::KeyG),
        "H" => Some(Code::KeyH),
        "I" => Some(Code::KeyI),
        "J" => Some(Code::KeyJ),
        "K" => Some(Code::KeyK),
        "L" => Some(Code::KeyL),
        "M" => Some(Code::KeyM),
        "N" => Some(Code::KeyN),
        "O" => Some(Code::KeyO),
        "P" => Some(Code::KeyP),
        "Q" => Some(Code::KeyQ),
        "R" => Some(Code::KeyR),
        "S" => Some(Code::KeyS),
        "T" => Some(Code::KeyT),
        "U" => Some(Code::KeyU),
        "V" => Some(Code::KeyV),
        "W" => Some(Code::KeyW),
        "X" => Some(Code::KeyX),
        "Y" => Some(Code::KeyY),
        "Z" => Some(Code::KeyZ),
        "SPACE" => Some(Code::Space),
        "CAPSLOCK" => Some(Code::CapsLock),
        _ => None,
    }
}

pub fn parse_hotkey(config: &crate::config::HotkeyConfig) -> Result<(Modifiers, Code), String> {
    let mut modifiers = parse_modifier(&config.modifier1)
        .ok_or_else(|| format!("Invalid shortcut modifier: {}", config.modifier1))?;
    if let Some(modifier) = &config.modifier2 {
        modifiers |= parse_modifier(modifier)
            .ok_or_else(|| format!("Invalid shortcut modifier: {}", modifier))?;
    }
    let key =
        parse_key(&config.key).ok_or_else(|| format!("Invalid shortcut key: {}", config.key))?;
    if key == Code::CapsLock && !modifiers.is_empty() {
        return Err("Use Caps Lock without modifiers for recording".to_string());
    }
    Ok((modifiers, key))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{AppConfig, HotkeyConfig};

    #[test]
    fn parses_existing_default_and_single_caps_lock() {
        assert_eq!(
            parse_hotkey(&AppConfig::default().hotkey).unwrap(),
            (Modifiers::CONTROL | Modifiers::SHIFT, Code::KeyZ)
        );
        let caps = HotkeyConfig {
            modifier1: "None".into(),
            modifier2: None,
            key: "CapsLock".into(),
        };
        assert_eq!(
            parse_hotkey(&caps).unwrap(),
            (Modifiers::empty(), Code::CapsLock)
        );
    }

    #[test]
    fn rejects_invalid_shortcuts() {
        let mut config = AppConfig::default().hotkey;
        config.modifier2 = Some("invalid".into());
        assert!(parse_hotkey(&config).is_err());
        config.modifier2 = None;
        config.key = "CapsLock".into();
        assert!(parse_hotkey(&config).is_err());
        config.modifier1 = "invalid".into();
        assert!(parse_hotkey(&config).is_err());
        config.modifier1 = "None".into();
        config.key = "invalid".into();
        assert!(parse_hotkey(&config).is_err());
    }
}
