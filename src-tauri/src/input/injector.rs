use enigo::{Enigo, Key, Keyboard, Settings};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

#[cfg(target_os = "windows")]
use clipboard_win::{formats, set_clipboard};

// Commands to send to the injector thread
enum InjectorCommand {
    InjectText(String, mpsc::Sender<Result<(), String>>),
}

pub struct TextInjector {
    command_sender: mpsc::Sender<InjectorCommand>,
}

// Safe because we communicate via channels
unsafe impl Send for TextInjector {}
unsafe impl Sync for TextInjector {}

impl TextInjector {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::channel::<InjectorCommand>();

        // Spawn a dedicated thread for text injection
        thread::spawn(move || {
            let settings = Settings::default();
            let mut enigo = match Enigo::new(&settings) {
                Ok(e) => e,
                Err(e) => {
                    log::error!("Failed to create Enigo instance: {}", e);
                    return;
                }
            };

            loop {
                match rx.recv() {
                    Ok(InjectorCommand::InjectText(text, response_tx)) => {
                        let result = inject_text_impl(&mut enigo, &text);
                        let _ = response_tx.send(result);
                    }
                    Err(_) => {
                        // Channel closed, exit thread
                        break;
                    }
                }
            }
        });

        Self { command_sender: tx }
    }

    pub fn inject_text(&mut self, text: &str) -> Result<(), String> {
        let (response_tx, response_rx) = mpsc::channel();
        self.command_sender
            .send(InjectorCommand::InjectText(text.to_string(), response_tx))
            .map_err(|e| format!("Failed to send inject command: {}", e))?;

        response_rx
            .recv()
            .map_err(|e| format!("Failed to receive inject response: {}", e))?
    }
}

fn inject_text_impl(enigo: &mut Enigo, text: &str) -> Result<(), String> {
    // Small delay to ensure target window is focused
    thread::sleep(Duration::from_millis(100));

    // Use clipboard approach for better reliability
    copy_to_clipboard(text)?;

    // Simulate Ctrl+V
    enigo
        .key(Key::Control, enigo::Direction::Press)
        .map_err(|e| format!("Failed to press Ctrl: {}", e))?;
    thread::sleep(Duration::from_millis(50));
    enigo
        .key(Key::Unicode('v'), enigo::Direction::Click)
        .map_err(|e| format!("Failed to press V: {}", e))?;
    thread::sleep(Duration::from_millis(50));
    enigo
        .key(Key::Control, enigo::Direction::Release)
        .map_err(|e| format!("Failed to release Ctrl: {}", e))?;

    log::info!("Text injected successfully");
    Ok(())
}

#[cfg(target_os = "windows")]
fn copy_to_clipboard(text: &str) -> Result<(), String> {
    set_clipboard(formats::Unicode, text).map_err(|e| format!("Clipboard error: {}", e))
}

#[cfg(not(target_os = "windows"))]
fn copy_to_clipboard(_text: &str) -> Result<(), String> {
    Err("Clipboard operation not supported on this platform".to_string())
}
