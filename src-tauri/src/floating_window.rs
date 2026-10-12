use std::cell::Cell;
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::Accessibility::{SetWinEventHook, UnhookWinEvent, HWINEVENTHOOK};
use windows::Win32::UI::WindowsAndMessaging::{
    IsIconic, IsWindowVisible, SetWindowPos, ShowWindowAsync, EVENT_SYSTEM_FOREGROUND,
    HWND_TOPMOST, SWP_ASYNCWINDOWPOS, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_SHOWWINDOW,
    SW_SHOWNOACTIVATE, WINEVENT_OUTOFCONTEXT, WINEVENT_SKIPOWNPROCESS,
};

thread_local! {
    static FLOATING_WINDOW: Cell<usize> = const { Cell::new(0) };
}

pub fn raise_window(handle: usize) -> Result<(), String> {
    let window = HWND(handle as *mut _);
    unsafe {
        if IsIconic(window).as_bool() {
            ShowWindowAsync(window, SW_SHOWNOACTIVATE)
                .ok()
                .map_err(|error| format!("Could not restore floating window: {error}"))?;
        }
        // Queue cross-thread positioning and never activate the dictation overlay.
        SetWindowPos(
            window,
            HWND_TOPMOST,
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_SHOWWINDOW | SWP_ASYNCWINDOWPOS,
        )
        .map_err(|error| format!("Could not raise floating window: {error}"))
    }
}

pub struct ForegroundWatcher {
    hook: HWINEVENTHOOK,
}

impl ForegroundWatcher {
    // This guard lives on the hotkey thread, which already pumps Windows messages.
    pub fn new(window_handle: usize) -> Result<Self, String> {
        let hook = unsafe {
            SetWinEventHook(
                EVENT_SYSTEM_FOREGROUND,
                EVENT_SYSTEM_FOREGROUND,
                None,
                Some(foreground_changed),
                0,
                0,
                WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS,
            )
        };
        if hook.0.is_null() {
            return Err(format!(
                "Could not watch foreground window changes: {}",
                windows::core::Error::from_win32()
            ));
        }
        FLOATING_WINDOW.with(|window| window.set(window_handle));
        Ok(Self { hook })
    }
}

impl Drop for ForegroundWatcher {
    fn drop(&mut self) {
        FLOATING_WINDOW.with(|window| window.set(0));
        if !unsafe { UnhookWinEvent(self.hook) }.as_bool() {
            log::error!("Could not remove foreground window watcher");
        }
    }
}

unsafe extern "system" fn foreground_changed(
    _hook: HWINEVENTHOOK,
    _event: u32,
    _foreground: HWND,
    _object: i32,
    _child: i32,
    _thread: u32,
    _time: u32,
) {
    FLOATING_WINDOW.with(|handle| {
        let handle = handle.get();
        let window = HWND(handle as *mut _);
        // Respect intentionally hidden/minimized windows until the recording shortcut is used.
        if handle != 0 && IsWindowVisible(window).as_bool() && !IsIconic(window).as_bool() {
            if let Err(error) = raise_window(handle) {
                log::warn!("{}", error);
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};
    use windows::core::w;
    use windows::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DestroyWindow, DispatchMessageW, GetForegroundWindow, GetWindow,
        GetWindowLongPtrW, PeekMessageW, ShowWindow, TranslateMessage, GWL_EXSTYLE, GW_HWNDNEXT,
        MSG, PM_REMOVE, SW_HIDE, SW_SHOWMINNOACTIVE, WS_EX_NOACTIVATE, WS_EX_TOPMOST, WS_POPUP,
        WS_VISIBLE,
    };

    struct TestWindow(HWND);

    impl Drop for TestWindow {
        fn drop(&mut self) {
            unsafe { DestroyWindow(self.0) }.unwrap();
        }
    }

    unsafe fn window(visible: bool) -> TestWindow {
        TestWindow(
            CreateWindowExW(
                WS_EX_NOACTIVATE,
                w!("STATIC"),
                w!("FluxVoice visibility fixture"),
                WS_POPUP
                    | if visible {
                        WS_VISIBLE
                    } else {
                        Default::default()
                    },
                30,
                130,
                200,
                60,
                None,
                None,
                None,
                None,
            )
            .unwrap(),
        )
    }

    unsafe fn pump_until_restored(window: HWND) {
        let deadline = Instant::now() + Duration::from_millis(500);
        while IsIconic(window).as_bool() && Instant::now() < deadline {
            let mut message = MSG::default();
            while PeekMessageW(&mut message, None, 0, 0, PM_REMOVE).as_bool() {
                let _ = TranslateMessage(&message);
                DispatchMessageW(&message);
            }
        }
        assert!(!IsIconic(window).as_bool());
    }

    unsafe fn is_above(upper: HWND, lower: HWND) -> bool {
        let mut current = upper;
        for _ in 0..1024 {
            match GetWindow(current, GW_HWNDNEXT) {
                Ok(window) if window == lower => return true,
                Ok(window) => current = window,
                Err(_) => return false,
            }
        }
        false
    }

    #[test]
    #[ignore = "Creates native topmost windows; requires an interactive Windows desktop"]
    fn recovery_reorders_topmost_windows_without_taking_focus() {
        unsafe {
            let foreground = GetForegroundWindow();
            let floating = window(false);
            let covering = window(true);
            let handle = floating.0 .0 as usize;
            let watcher = ForegroundWatcher::new(handle).unwrap();
            raise_window(handle).unwrap();
            assert!(IsWindowVisible(floating.0).as_bool());
            assert_ne!(
                GetWindowLongPtrW(floating.0, GWL_EXSTYLE) & WS_EX_TOPMOST.0 as isize,
                0
            );
            assert_eq!(GetForegroundWindow(), foreground);

            raise_window(covering.0 .0 as usize).unwrap();
            assert!(is_above(covering.0, floating.0));
            foreground_changed(
                watcher.hook,
                EVENT_SYSTEM_FOREGROUND,
                covering.0,
                0,
                0,
                0,
                0,
            );
            assert!(is_above(floating.0, covering.0));
            assert_eq!(GetForegroundWindow(), foreground);

            let _ = ShowWindow(floating.0, SW_HIDE);
            foreground_changed(
                watcher.hook,
                EVENT_SYSTEM_FOREGROUND,
                covering.0,
                0,
                0,
                0,
                0,
            );
            assert!(!IsWindowVisible(floating.0).as_bool());
            raise_window(handle).unwrap();
            let _ = ShowWindow(floating.0, SW_SHOWMINNOACTIVE);
            foreground_changed(
                watcher.hook,
                EVENT_SYSTEM_FOREGROUND,
                covering.0,
                0,
                0,
                0,
                0,
            );
            assert!(IsIconic(floating.0).as_bool());
            raise_window(handle).unwrap();
            pump_until_restored(floating.0);
            assert_eq!(GetForegroundWindow(), foreground);
        }
    }
}
