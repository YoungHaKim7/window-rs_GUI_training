//! e94 — Worker Threads: long work off the UI thread, results back by message
//!
//! What you learn:
//! * The one rule of responsive GUIs: **never do slow work on the thread
//!   that owns a window** — while your wndproc is busy, the window pumps no
//!   messages, so it stops painting, stops reacting, and Windows stamps it
//!   "Not Responding". Slow work goes to a `std::thread`; the UI thread
//!   stays free to dispatch messages
//! * The **message-as-mailbox** pattern: the worker may not touch any
//!   window (handles belong to the creating thread's message queue), so it
//!   reports back with `PostMessageW` — `WM_APP + n` messages whose wparam
//!   carries the payload (here: progress percent, and a "done" flag).
//!   PostMessageW is one of the very few window APIs that is *designed* to
//!   be called from another thread: it just appends to the queue and
//!   returns; your wndproc picks the mail up on the UI thread, where
//!   touching windows IS allowed
//! * The `Send` reality of raw handles: `HWND` is a raw pointer alias and
//!   raw pointers are `!Send`, so the closure could not capture it even
//!   though passing it through is perfectly safe in practice — the idiom is
//!   to smuggle it as an `isize` (integer → `Send`) and cast back inside
//!   the thread
//! * Cooperative cancellation with `Arc<AtomicBool>`: the Cancel button
//!   flips the flag, the worker checks it between chunks — no thread is
//!   ever killed from outside (that leaks locks and half-written state)
//!
//! Click "Generate...": the progress bar fills over ~2 seconds while the
//! window stays fully responsive (drag it, resize it). Click Cancel
//! mid-run: the worker stops at the next chunk boundary.
//!
//! New Win32 APIs: `PostMessageW` across threads, `WM_APP` (winuser);
//! `InitCommonControlsEx` + `msctls_progress32` recap from e50 (commctrl);
//! `EnableWindow` for the button state machine.
//!
//! Run: `cargo run -p e94_worker_thread`

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use windows::{Win32::*, core::*};

const IDC_GENERATE: i32 = 1001;
const IDC_CANCEL: i32 = 1002;

/// Private messages for worker → UI-thread mail. WM_APP (0x8000) is the
/// documented base for app-private messages; anything below it belongs to
/// the system. Keep the `wparam` meaning documented right here.
const WM_APP_PROGRESS: i32 = WM_APP + 1; // wparam = percent 0..100
const WM_APP_DONE: i32 = WM_APP + 2; // wparam = 1 if cancelled, 0 if finished

/// How many chunks of "work" the simulated job has. Each chunk sleeps and
/// then reports (step * 100 / CHUNKS) percent — the sleep stands in for any
/// real blocking work (file hash, network fetch, image encode...).
const CHUNKS: u32 = 20;
const CHUNK_MS: u64 = 100;

#[derive(Default)]
struct State {
    hwnd_progress: HWND,
    hwnd_generate: HWND,
    hwnd_cancel: HWND,
    /// Shared with the live worker; a fresh Arc per run.
    cancel: Arc<AtomicBool>,
}

fn main() {
    unsafe {
        // Common controls first (e50): the progress-bar class must be
        // registered by comctl32 before CreateWindowExW can find it.
        let icc = INITCOMMONCONTROLSEX {
            dwSize: std::mem::size_of::<INITCOMMONCONTROLSEX>() as u32,
            dwICC: ICC_PROGRESS_CLASS as u32,
        };
        _ = InitCommonControlsEx(&icc);

        let instance = GetModuleHandleW(PCWSTR::null());
        debug_assert!(!instance.is_null());

        let wc = WNDCLASSW {
            lpfnWndProc: Some(wndproc),
            hInstance: instance,
            hCursor: LoadCursorW(None, PCWSTR(IDC_ARROW.0.cast())),
            hbrBackground: ((COLOR_WINDOW + 1) as isize) as HBRUSH,
            lpszClassName: w!("e94_class"),
            style: (CS_HREDRAW | CS_VREDRAW) as u32,
            ..Default::default()
        };
        debug_assert!(RegisterClassW(&wc) != 0);

        let hwnd = CreateWindowExW(
            0,
            w!("e94_class"),
            w!("e94 · Worker Thread — idle"),
            (WS_OVERLAPPEDWINDOW | WS_VISIBLE) as u32,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            520,
            200,
            None,
            None,
            Some(instance),
            None,
        );
        debug_assert!(!hwnd.is_null());

        let state = Box::into_raw(Box::new(State {
            cancel: Arc::new(AtomicBool::new(false)),
            ..State::default()
        }));
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, state as isize);

        let generate = CreateWindowExW(
            0,
            w!("BUTTON"),
            w!("Generate..."),
            (WS_CHILD | WS_VISIBLE | WS_TABSTOP) as u32,
            20,
            20,
            120,
            32,
            Some(hwnd),
            Some(IDC_GENERATE as HMENU),
            Some(instance),
            None,
        );
        debug_assert!(!generate.is_null());

        // Cancel starts DISABLED — there is nothing to cancel yet.
        // EnableWindow is the whole button state machine.
        let cancel = CreateWindowExW(
            0,
            w!("BUTTON"),
            w!("Cancel"),
            (WS_CHILD | WS_VISIBLE) as u32,
            150,
            20,
            120,
            32,
            Some(hwnd),
            Some(IDC_CANCEL as HMENU),
            Some(instance),
            None,
        );
        debug_assert!(!cancel.is_null());
        _ = EnableWindow(cancel, false);

        let progress = CreateWindowExW(
            0,
            w!("msctls_progress32"),
            w!(""),
            (WS_CHILD | WS_VISIBLE) as u32,
            20,
            80,
            460,
            24,
            Some(hwnd),
            None,
            Some(instance),
            None,
        );
        debug_assert!(!progress.is_null());
        SendMessageW(progress, PBM_SETRANGE32 as u32, 0, 100);

        if let Some(s) = state_of(hwnd) {
            s.hwnd_generate = generate;
            s.hwnd_cancel = cancel;
            s.hwnd_progress = progress;
        }

        let mut message = MSG::default();
        while GetMessageW(&mut message, None, 0, 0).0 > 0 {
            _ = TranslateMessage(&message);
            DispatchMessageW(&message);
        }
    }
}

unsafe fn state_of(hwnd: HWND) -> Option<&'static mut State> {
    unsafe {
        let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut State;
        (!ptr.is_null()).then(|| &mut *ptr)
    }
}

/// Spawn the "work". Runs entirely on its own thread; the ONLY bridge back
/// to the UI is PostMessageW to `hwnd`.
fn start_work(hwnd: HWND, cancel: Arc<AtomicBool>) {
    // HWND is a raw-pointer alias: `!Send`, so a spawned thread (whose
    // closure must be Send) cannot capture it directly. An isize is a
    // plain integer — Send — and round-trips the pointer value exactly.
    // This is the standard windows-rs idiom for handing windows to threads.
    let target = hwnd as isize;

    std::thread::spawn(move || {
        unsafe {
            let hwnd = target as HWND; // ...and back again, inside the thread
            let mut cancelled = false;

            for step in 0..=CHUNKS {
                // Cooperative cancel: check the flag between chunks. If set,
                // stop here — and still report DONE so the UI can re-enable
                // its buttons (a worker that dies silently strands the UI).
                if cancel.load(Ordering::Relaxed) {
                    cancelled = true;
                    break;
                }

                // The "work". Replace the sleep with anything blocking.
                std::thread::sleep(Duration::from_millis(CHUNK_MS));

                // Mailbox drop #1: progress percent rides wparam. PostMessage
                // returns immediately; if the window is mid-paint or busy,
                // the message simply waits in its queue. Never SendMessageW
                // from a worker — that BLOCKS until the UI thread answers.
                let percent = step * 100 / CHUNKS;
                _ = PostMessageW(Some(hwnd), WM_APP_PROGRESS as u32, percent as usize, 0);
            }

            // Mailbox drop #2: finished (or cancelled — wparam says which).
            _ = PostMessageW(Some(hwnd), WM_APP_DONE as u32, cancelled as usize, 0);
        }
    });
}

extern "system" fn wndproc(hwnd: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe {
        let Some(state) = state_of(hwnd) else {
            return DefWindowProcW(hwnd, message, wparam, lparam);
        };

        match message as i32 {
            WM_COMMAND => {
                // As since e15: LOWORD(wparam) = control id, HIWORD = code.
                let id = (wparam & 0xffff) as u16 as i32;
                match id {
                    IDC_GENERATE => {
                        // Fresh run: new flag, buttons flip, bar resets, go.
                        state.cancel = Arc::new(AtomicBool::new(false));
                        _ = EnableWindow(state.hwnd_generate, false);
                        _ = EnableWindow(state.hwnd_cancel, true);
                        SendMessageW(state.hwnd_progress, PBM_SETPOS as u32, 0, 0);
                        _ = SetWindowTextW(hwnd, w!("e94 · Worker Thread — working..."));
                        start_work(hwnd, Arc::clone(&state.cancel));
                        0
                    }
                    IDC_CANCEL => {
                        // Just flip the flag. The worker notices at the next
                        // chunk and posts WM_APP_DONE itself — the UI never
                        // waits on the thread.
                        state.cancel.store(true, Ordering::Relaxed);
                        _ = SetWindowTextW(hwnd, w!("e94 · Worker Thread — cancelling..."));
                        0
                    }
                    _ => DefWindowProcW(hwnd, message, wparam, lparam),
                }
            }

            // ---- Mail pickup: these arrive on the UI thread, where it is
            // ---- legal to touch windows again. That's the whole trick.
            WM_APP_PROGRESS => {
                let percent = wparam as i32;
                SendMessageW(state.hwnd_progress, PBM_SETPOS as u32, percent as usize, 0);
                _ = SetWindowTextW(
                    hwnd,
                    &HSTRING::from(format!("e94 · Worker Thread — {percent}%")),
                );
                0
            }
            WM_APP_DONE => {
                let cancelled = wparam != 0;
                _ = EnableWindow(state.hwnd_generate, true);
                _ = EnableWindow(state.hwnd_cancel, false);
                _ = SetWindowTextW(
                    hwnd,
                    &HSTRING::from(if cancelled {
                        "e94 · Worker Thread — cancelled"
                    } else {
                        "e94 · Worker Thread — done (window stayed responsive)"
                    }),
                );
                0
            }

            WM_DESTROY => {
                // If a worker is still running it will find its flag set at
                // the next chunk and exit; its PostMessageW to a dead window
                // simply fails harmlessly. We don't join — the UI thread
                // leaving the message loop must not block on the worker.
                state.cancel.store(true, Ordering::Relaxed);

                let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut State;
                if !ptr.is_null() {
                    SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
                    drop(Box::from_raw(ptr));
                }
                PostQuitMessage(0);
                0
            }

            _ => DefWindowProcW(hwnd, message, wparam, lparam),
        }
    }
}
