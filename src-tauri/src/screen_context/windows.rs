use super::{Capture, PhraseCollector, MAX_PHRASES};
use ::windows::core::Interface;
use ::windows::Win32::Foundation::{HWND, RECT};
use ::windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED,
};
use ::windows::Win32::UI::Accessibility::{
    CUIAutomation8, IUIAutomation, IUIAutomation2, IUIAutomationElement, IUIAutomationTextPattern,
    IUIAutomationTreeWalker, UIA_TextPatternId, UIA_E_NOTSUPPORTED,
};
use ::windows::Win32::UI::WindowsAndMessaging::{
    GetForegroundWindow, GetWindowRect, GetWindowThreadProcessId, IsWindow,
};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
use tokio::sync::oneshot;

static CAPTURE_BUSY: AtomicBool = AtomicBool::new(false);
const MAX_ELEMENTS: usize = 256;
const MAX_DEPTH: usize = 64;
const MAX_TEXT_CHARS: usize = 12_000;
const WORK_BUDGET: Duration = Duration::from_millis(750);

struct BusyGuard;

impl Drop for BusyGuard {
    fn drop(&mut self) {
        CAPTURE_BUSY.store(false, Ordering::Release);
    }
}

struct ComGuard;

impl Drop for ComGuard {
    fn drop(&mut self) {
        unsafe { CoUninitialize() };
    }
}

pub(super) fn start_capture() -> Result<Capture, String> {
    let started = Instant::now();
    let window = unsafe { GetForegroundWindow() };
    let mut process_id = 0;
    unsafe { GetWindowThreadProcessId(window, Some(&mut process_id)) };
    if window.0.is_null() || process_id == 0 {
        return Err("No active window is available for screen context".into());
    }
    // Never collect FluxVoice's settings, which can contain service credentials.
    if process_id == std::process::id() {
        println!("[screen-context] capture_skipped=fluxvoice_foreground");
        let (sender, receiver) = oneshot::channel();
        let _ = sender.send(Ok(Vec::new()));
        return Ok(Capture { receiver, started });
    }
    if CAPTURE_BUSY
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        return Err("Previous screen context capture is still running".into());
    }
    let guard = BusyGuard;
    let (sender, receiver) = oneshot::channel();
    let handle = window.0 as usize;
    std::thread::Builder::new()
        .name("screen-context".into())
        .spawn(move || {
            let _guard = guard;
            let result = capture_window(HWND(handle as *mut _), process_id, started);
            println!(
                "[screen-context] capture_ms={:.1} success={} phrase_count={}",
                started.elapsed().as_secs_f64() * 1000.0,
                result.is_ok(),
                result.as_ref().map_or(0, Vec::len),
            );
            if let Err(error) = &result {
                log::warn!("Screen context capture failed: {}", error);
            }
            let _ = sender.send(result);
        })
        .map_err(|error| format!("Could not start screen context worker: {error}"))?;
    Ok(Capture { receiver, started })
}

fn capture_window(window: HWND, process_id: u32, started: Instant) -> Result<Vec<String>, String> {
    unsafe {
        CoInitializeEx(None, COINIT_MULTITHREADED)
            .ok()
            .map_err(|error| format!("Could not initialize UI Automation COM: {error}"))?;
        let _com = ComGuard;
        let automation2: IUIAutomation2 =
            CoCreateInstance(&CUIAutomation8, None, CLSCTX_INPROC_SERVER)
                .map_err(|error| format!("Could not initialize UI Automation: {error}"))?;
        automation2
            .SetConnectionTimeout(200)
            .map_err(|error| error.to_string())?;
        automation2
            .SetTransactionTimeout(100)
            .map_err(|error| error.to_string())?;
        let automation: IUIAutomation = automation2.cast().map_err(|error| error.to_string())?;
        let mut current_process = 0;
        GetWindowThreadProcessId(window, Some(&mut current_process));
        if !IsWindow(window).as_bool() || current_process != process_id {
            return Err("The active window closed before screen context capture".into());
        }
        let mut bounds = RECT::default();
        GetWindowRect(window, &mut bounds).map_err(|error| error.to_string())?;
        let root = automation
            .ElementFromHandle(window)
            .map_err(|error| error.to_string())?;
        let walker = automation
            .RawViewWalker()
            .map_err(|error| error.to_string())?;
        let mut reader = Reader {
            walker,
            bounds,
            started,
            visited: 0,
            text_chars: 0,
            collector: PhraseCollector::default(),
            diagnostics: Diagnostics::default(),
        };
        let focused = automation
            .GetFocusedElement()
            .map_err(|error| error.to_string())?;
        let mut ancestor = focused.clone();
        let mut focused_in_window = false;
        for _ in 0..=MAX_DEPTH {
            if reader.exhausted() {
                return Err("Screen context capture exceeded its time budget".into());
            }
            if ancestor
                .CurrentIsPassword()
                .map_err(|error| error.to_string())?
                .as_bool()
            {
                println!("[screen-context] capture_skipped=password_focus");
                return Ok(Vec::new());
            }
            if automation
                .CompareElements(&ancestor, &root)
                .map_err(|error| error.to_string())?
                .as_bool()
            {
                focused_in_window = true;
                break;
            }
            match reader.walker.GetParentElement(&ancestor) {
                Ok(parent) => ancestor = parent,
                Err(error) if error.code() == ::windows::Win32::Foundation::S_OK => break,
                Err(error) => return Err(error.to_string()),
            }
        }
        if focused_in_window {
            reader
                .visit(&focused, 0)
                .map_err(|error| error.to_string())?;
        }
        reader.visit(&root, 0).map_err(|error| error.to_string())?;
        println!(
            "[screen-context] focused_in_window={} visited={} text_chars={} budget_exhausted={} diagnostics={:?}",
            focused_in_window, reader.visited, reader.text_chars, reader.exhausted(), reader.diagnostics,
        );
        log::info!(
            "Screen context captured {} phrase hints",
            reader.collector.phrases.len()
        );
        Ok(reader.collector.phrases)
    }
}

struct Reader {
    walker: IUIAutomationTreeWalker,
    bounds: RECT,
    started: Instant,
    visited: usize,
    text_chars: usize,
    collector: PhraseCollector,
    diagnostics: Diagnostics,
}

#[derive(Debug, Default)]
struct Diagnostics {
    depth_skipped: usize,
    offscreen_skipped: usize,
    bounds_skipped: usize,
    protected_skipped: usize,
    leaves: usize,
    text_patterns: usize,
}

impl Reader {
    fn exhausted(&self) -> bool {
        self.started.elapsed() >= WORK_BUDGET
            || self.visited >= MAX_ELEMENTS
            || self.text_chars >= MAX_TEXT_CHARS
            || self.collector.phrases.len() >= MAX_PHRASES
    }

    fn collect(&mut self, text: &str) {
        let text: String = text
            .chars()
            .take(MAX_TEXT_CHARS - self.text_chars)
            .collect();
        self.text_chars += text.chars().count();
        self.collector.collect(&text);
    }

    unsafe fn visit(
        &mut self,
        element: &IUIAutomationElement,
        depth: usize,
    ) -> ::windows::core::Result<()> {
        if self.exhausted() {
            return Ok(());
        }
        if depth > MAX_DEPTH {
            self.diagnostics.depth_skipped += 1;
            return Ok(());
        }
        self.visited += 1;
        // Fail closed: do not read names, text, or children of protected elements.
        if element.CurrentIsPassword()?.as_bool() {
            self.diagnostics.protected_skipped += 1;
            return Ok(());
        }
        if element.CurrentIsOffscreen()?.as_bool() {
            self.diagnostics.offscreen_skipped += 1;
            return Ok(());
        }
        let rect = element.CurrentBoundingRectangle()?;
        let has_bounds = rect.right > rect.left && rect.bottom > rect.top;
        if has_bounds
            && (rect.right <= self.bounds.left
                || rect.left >= self.bounds.right
                || rect.bottom <= self.bounds.top
                || rect.top >= self.bounds.bottom)
        {
            self.diagnostics.bounds_skipped += 1;
            return Ok(());
        }
        // windows 0.58 represents a successful null COM interface as Err(S_OK).
        let child = match self.walker.GetFirstChildElement(element) {
            Ok(child) => Some(child),
            Err(error) if error.code() == ::windows::Win32::Foundation::S_OK => None,
            Err(error) => return Err(error),
        };
        // Container text patterns can include hidden or password descendants.
        // Read only visible text ranges of leaves; never read ValuePattern.
        // Non-rendered web accessibility containers may have visible descendants.
        if child.is_none() && has_bounds {
            self.diagnostics.leaves += 1;
            match element.GetCurrentPatternAs::<IUIAutomationTextPattern>(UIA_TextPatternId) {
                Ok(pattern) => {
                    self.diagnostics.text_patterns += 1;
                    let ranges = pattern.GetVisibleRanges()?;
                    for index in 0..ranges.Length()?.min(8) {
                        if self.exhausted() {
                            break;
                        }
                        self.collect(&ranges.GetElement(index)?.GetText(2048)?.to_string());
                    }
                }
                Err(error)
                    if error.code() == ::windows::core::HRESULT(UIA_E_NOTSUPPORTED as i32)
                        || error.code() == ::windows::Win32::Foundation::E_NOINTERFACE
                        || error.code() == ::windows::Win32::Foundation::S_OK => {}
                Err(error) => return Err(error),
            }
            if !self.exhausted() {
                self.collect(&element.CurrentName()?.to_string());
            }
        }
        let mut child = child;
        while let Some(current) = child {
            if self.exhausted() {
                break;
            }
            self.visit(&current, depth + 1)?;
            child = match self.walker.GetNextSiblingElement(&current) {
                Ok(sibling) => Some(sibling),
                Err(error) if error.code() == ::windows::Win32::Foundation::S_OK => None,
                Err(error) => return Err(error),
            };
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ::windows::core::w;
    use ::windows::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DestroyWindow, DispatchMessageW, PeekMessageW, TranslateMessage,
        ES_PASSWORD, MSG, PM_REMOVE, WINDOW_STYLE, WS_CHILD, WS_EX_NOACTIVATE, WS_POPUP,
        WS_VISIBLE,
    };

    #[test]
    #[ignore = "Reads foreground UI Automation text; requires explicit interactive debugging"]
    fn diagnose_foreground_capture_without_logging_text() {
        let window = unsafe { GetForegroundWindow() };
        let mut process_id = 0;
        unsafe { GetWindowThreadProcessId(window, Some(&mut process_id)) };
        let started = Instant::now();
        let result = capture_window(window, process_id, started);
        println!(
            "[screen-context] foreground_probe_ms={:.1} success={} phrase_count={}",
            started.elapsed().as_secs_f64() * 1000.0,
            result.is_ok(),
            result.as_ref().map_or(0, Vec::len),
        );
        if let Err(error) = &result {
            println!("[screen-context] foreground_probe_error={}", error);
        }
        assert!(result.is_ok(), "Foreground capture failed");
    }

    #[test]
    #[ignore = "Creates a non-activating native window; requires an interactive Windows desktop"]
    fn reads_visible_native_text_but_excludes_passwords_and_hidden_controls() {
        let (ready_tx, ready_rx) = std::sync::mpsc::channel();
        let (stop_tx, stop_rx) = std::sync::mpsc::channel();
        let ui_thread = std::thread::spawn(move || unsafe {
            let window = CreateWindowExW(
                WS_EX_NOACTIVATE,
                w!("STATIC"),
                w!("UI Automation fixture"),
                WS_POPUP | WS_VISIBLE,
                10,
                10,
                400,
                180,
                None,
                None,
                None,
                None,
            )
            .unwrap();
            CreateWindowExW(
                Default::default(),
                w!("STATIC"),
                w!("Rehaan Kumatani"),
                WS_CHILD | WS_VISIBLE,
                10,
                10,
                350,
                30,
                window,
                None,
                None,
                None,
            )
            .unwrap();
            CreateWindowExW(
                Default::default(),
                w!("EDIT"),
                w!("FluxVoice"),
                WS_CHILD | WS_VISIBLE,
                10,
                45,
                350,
                30,
                window,
                None,
                None,
                None,
            )
            .unwrap();
            CreateWindowExW(
                Default::default(),
                w!("EDIT"),
                w!("PasswordSecret"),
                WS_CHILD | WS_VISIBLE | WINDOW_STYLE(ES_PASSWORD as u32),
                10,
                80,
                350,
                30,
                window,
                None,
                None,
                None,
            )
            .unwrap();
            CreateWindowExW(
                Default::default(),
                w!("STATIC"),
                w!("HiddenSecret"),
                WS_CHILD,
                10,
                115,
                350,
                30,
                window,
                None,
                None,
                None,
            )
            .unwrap();
            ready_tx.send(window.0 as usize).unwrap();
            while matches!(
                stop_rx.try_recv(),
                Err(std::sync::mpsc::TryRecvError::Empty)
            ) {
                let mut message = MSG::default();
                while PeekMessageW(&mut message, None, 0, 0, PM_REMOVE).as_bool() {
                    let _ = TranslateMessage(&message);
                    DispatchMessageW(&message);
                }
                std::thread::sleep(Duration::from_millis(5));
            }
            DestroyWindow(window).unwrap();
        });
        let handle = ready_rx.recv_timeout(Duration::from_secs(5)).unwrap();
        let result = unsafe {
            CoInitializeEx(None, COINIT_MULTITHREADED).ok().unwrap();
            let _com = ComGuard;
            let automation: IUIAutomation =
                CoCreateInstance(&CUIAutomation8, None, CLSCTX_INPROC_SERVER).unwrap();
            let window = HWND(handle as *mut _);
            let root = automation.ElementFromHandle(window).unwrap();
            let mut bounds = RECT::default();
            GetWindowRect(window, &mut bounds).unwrap();
            let mut reader = Reader {
                walker: automation.RawViewWalker().unwrap(),
                bounds,
                started: Instant::now(),
                visited: 0,
                text_chars: 0,
                collector: PhraseCollector::default(),
                diagnostics: Diagnostics::default(),
            };
            // Model a native subtree below 17 web accessibility wrappers.
            let result = reader.visit(&root, 17);
            println!(
                "[screen-context-fixture] visited={} text_chars={} budget_exhausted={} diagnostics={:?}",
                reader.visited, reader.text_chars, reader.exhausted(), reader.diagnostics,
            );
            result.map(|()| reader.collector.phrases)
        };
        stop_tx.send(()).unwrap();
        ui_thread.join().unwrap();
        let phrases = result.unwrap();
        assert!(phrases.contains(&"Rehaan Kumatani".into()));
        assert!(phrases.contains(&"FluxVoice".into()));
        assert!(phrases.iter().all(|phrase| !phrase.contains("Secret")));
    }
}
