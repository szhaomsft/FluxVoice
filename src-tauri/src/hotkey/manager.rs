use global_hotkey::{
    hotkey::{Code, HotKey, Modifiers},
    GlobalHotKeyEvent, GlobalHotKeyManager,
};
use std::sync::mpsc;
use std::thread;
use tauri::Emitter;

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

        // Spawn a dedicated thread for hotkey management
        thread::spawn(move || {
            let manager = match GlobalHotKeyManager::new() {
                Ok(m) => m,
                Err(e) => {
                    log::error!("Failed to create GlobalHotKeyManager: {}", e);
                    return;
                }
            };

            let event_receiver = GlobalHotKeyEvent::receiver();
            let mut current_hotkey: Option<HotKey> = None;

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
                if let Ok(_event) = event_receiver.try_recv() {
                    println!(">>> HOTKEY PRESSED! <<<");
                    log::info!("Hotkey triggered");
                    if let Err(e) = app_handle.emit("hotkey-triggered", ()) {
                        log::error!("Failed to emit hotkey event: {}", e);
                    }
                }

                // Check for commands (non-blocking)
                match rx.try_recv() {
                    Ok(HotkeyCommand::Register(modifiers, key, response_tx)) => {
                        // Unregister current hotkey if exists
                        if let Some(hotkey) = current_hotkey.take() {
                            if let Err(e) = manager.unregister(hotkey) {
                                log::warn!("Failed to unregister previous hotkey: {}", e);
                            }
                        }

                        // Create and register new hotkey
                        let hotkey = HotKey::new(Some(modifiers), key);
                        match manager.register(hotkey) {
                            Ok(()) => {
                                current_hotkey = Some(hotkey);
                                log::info!("Hotkey registered: {:?} + {:?}", modifiers, key);
                                let _ = response_tx.send(Ok(()));
                            }
                            Err(e) => {
                                let err = format!("Failed to register hotkey: {}", e);
                                log::error!("{}", err);
                                let _ = response_tx.send(Err(err));
                            }
                        }
                    }
                    Ok(HotkeyCommand::Unregister(response_tx)) => {
                        if let Some(hotkey) = current_hotkey.take() {
                            match manager.unregister(hotkey) {
                                Ok(()) => {
                                    let _ = response_tx.send(Ok(()));
                                }
                                Err(e) => {
                                    let _ = response_tx.send(Err(format!(
                                        "Failed to unregister hotkey: {}",
                                        e
                                    )));
                                }
                            }
                        } else {
                            let _ = response_tx.send(Ok(()));
                        }
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
        _ => None,
    }
}
