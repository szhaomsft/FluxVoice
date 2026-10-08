use global_hotkey::HotKeyState;
use std::cell::RefCell;
use std::sync::mpsc;
use windows::Win32::Foundation::{HINSTANCE, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, SetWindowsHookExW, UnhookWindowsHookEx, HHOOK, KBDLLHOOKSTRUCT, LLKHF_INJECTED,
    WH_KEYBOARD_LL, WM_KEYDOWN, WM_KEYUP, WM_SYSKEYDOWN, WM_SYSKEYUP,
};

const VK_CAPITAL: u32 = 0x14;

#[derive(Default)]
struct KeyState {
    pressed: bool,
}

impl KeyState {
    fn handle(&mut self, key: u32, message: u32, injected: bool) -> (bool, Option<HotKeyState>) {
        if key != VK_CAPITAL || injected {
            return (false, None);
        }

        let pressed = match message {
            WM_KEYDOWN | WM_SYSKEYDOWN => true,
            WM_KEYUP | WM_SYSKEYUP => false,
            _ => return (false, None),
        };
        if pressed == self.pressed {
            return (true, None);
        }
        self.pressed = pressed;
        (
            true,
            Some(if pressed {
                HotKeyState::Pressed
            } else {
                HotKeyState::Released
            }),
        )
    }
}

thread_local! {
    static STATE: RefCell<Option<(KeyState, mpsc::Sender<HotKeyState>)>> = const { RefCell::new(None) };
}

unsafe extern "system" fn keyboard_hook(code: i32, message: WPARAM, data: LPARAM) -> LRESULT {
    if code >= 0 {
        let keyboard = &*(data.0 as *const KBDLLHOOKSTRUCT);
        let consumed = STATE.with(|state| {
            let mut state = state.borrow_mut();
            if let Some((key_state, sender)) = state.as_mut() {
                let (consumed, event) = key_state.handle(
                    keyboard.vkCode,
                    message.0 as u32,
                    keyboard.flags.contains(LLKHF_INJECTED),
                );
                if let Some(event) = event {
                    if let Err(error) = sender.send(event) {
                        log::error!("Failed to deliver Caps Lock event: {}", error);
                    }
                }
                consumed
            } else {
                false
            }
        });
        if consumed {
            // Consume both edges so Windows never changes the Caps Lock toggle state.
            return LRESULT(1);
        }
    }
    CallNextHookEx(None, code, message, data)
}

pub(super) struct CapsLockHook {
    handle: HHOOK,
    pub events: mpsc::Receiver<HotKeyState>,
}

impl CapsLockHook {
    pub fn new() -> Result<Self, String> {
        let (sender, events) = mpsc::channel();
        let module = unsafe { GetModuleHandleW(None) }
            .map_err(|error| format!("Failed to get keyboard hook module: {}", error))?;
        let handle = unsafe {
            SetWindowsHookExW(WH_KEYBOARD_LL, Some(keyboard_hook), HINSTANCE(module.0), 0)
        }
        .map_err(|error| format!("Failed to install Caps Lock keyboard hook: {}", error))?;
        STATE.with(|state| *state.borrow_mut() = Some((KeyState::default(), sender)));
        Ok(Self { handle, events })
    }
}

impl Drop for CapsLockHook {
    fn drop(&mut self) {
        if let Err(error) = unsafe { UnhookWindowsHookEx(self.handle) } {
            log::error!("Failed to remove Caps Lock keyboard hook: {}", error);
        }
        STATE.with(|state| *state.borrow_mut() = None);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn consumes_caps_lock_and_emits_one_pair_per_hold() {
        let mut state = KeyState::default();
        assert_eq!(
            state.handle(VK_CAPITAL, WM_KEYDOWN, false),
            (true, Some(HotKeyState::Pressed))
        );
        assert_eq!(state.handle(VK_CAPITAL, WM_KEYDOWN, false), (true, None));
        assert_eq!(
            state.handle(VK_CAPITAL, WM_KEYUP, false),
            (true, Some(HotKeyState::Released))
        );
        assert_eq!(state.handle(VK_CAPITAL, WM_KEYUP, false), (true, None));
        assert_eq!(
            state.handle(VK_CAPITAL, WM_SYSKEYDOWN, false),
            (true, Some(HotKeyState::Pressed))
        );
        assert_eq!(
            state.handle(VK_CAPITAL, WM_SYSKEYUP, false),
            (true, Some(HotKeyState::Released))
        );
    }

    #[test]
    fn passes_other_keys_and_injected_input_without_changing_hold() {
        let mut state = KeyState::default();
        assert_eq!(state.handle(0x41, WM_KEYDOWN, false), (false, None));
        assert_eq!(state.handle(VK_CAPITAL, WM_KEYDOWN, true), (false, None));
        assert_eq!(
            state.handle(VK_CAPITAL, WM_KEYDOWN, false),
            (true, Some(HotKeyState::Pressed))
        );
        assert_eq!(state.handle(VK_CAPITAL, WM_KEYUP, true), (false, None));
        assert_eq!(
            state.handle(VK_CAPITAL, WM_KEYUP, false),
            (true, Some(HotKeyState::Released))
        );
    }

    #[test]
    fn installs_and_removes_windows_hook() {
        let hook = CapsLockHook::new().expect("Windows keyboard hook should install");
        STATE.with(|state| assert!(state.borrow().is_some()));
        drop(hook);
        STATE.with(|state| assert!(state.borrow().is_none()));
        let hook = CapsLockHook::new().expect("Windows keyboard hook should reinstall");
        drop(hook);
    }
}
