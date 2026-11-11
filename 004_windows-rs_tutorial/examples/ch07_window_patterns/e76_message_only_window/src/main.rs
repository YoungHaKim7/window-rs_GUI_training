//! e76 — Message-Only Windows: HWND_MESSAGE, the invisible mailboxes
//!
//! What you learn:
//! * A **message-only window**: created like any other, except its parent
//!   is `HWND_MESSAGE` (the pseudo-handle -3). Such a window is never
//!   visible, never receives user input, is skipped by window
//!   enumeration — but it HAS a wndproc and a message queue slot, so it
//!   receives everything you `PostMessage`/`SendMessage` to it.
//! * Why that combination is gold: a window is the cheapest way to give a
//!   *thing* an addressable message loop. Classic uses:
//!     - **hotkey sinks** — RegisterHotKey targets a window you don't
//!       want visible (e19 could have used one);
//!     - **device notifications** — RegisterDeviceNotification requires
//!       an HWND or a service handle;
//!     - **worker-thread mailboxes** — a thread + message-only window is
//!       a ready-made actor: PostMessage in, wndproc reacts, no locks.
//! * Cross-window posting from a **WM_TIMER** on the main window: every
//!   tick the visible window posts a WM_APP+n message to the invisible
//!   one, whose wndproc `println!`s it — watch the console, not the
//!   screen (well, both: the main window also counts).
//! * What message-only windows DON'T get: broadcasts (HWND_BROADCAST
//!   skips them), anything input-related, and visibility — by design.
//!
//! Every 500ms the main window posts to the mailbox; the console prints
//! each delivery. Close the window to stop the chatter.
//!
//! New Win32 APIs: `HWND_MESSAGE` (the pseudo-parent), `SetTimer`,
//! `WM_TIMER`, `PostMessageW` across windows.
//!
//! Run: `cargo run -p e76_message_only_window`

use windows::{Win32::*, core::*};

/// Mailbox message kinds — the WM_APP range again, as in e70/e71.
const WM_APP_PING: i32 = WM_APP + 1;
const WM_APP_NOTE: i32 = WM_APP + 2;

const TIMER_ID: usize = 1;
const TICK_MS: u32 = 500;

#[derive(Default)]
struct State {
    /// The invisible worker's HWND — needed as the post target.
    worker: HWND,
    /// How many messages we have posted so far.
    sent: u32,
}

fn main() {
    unsafe {
        let instance = GetModuleHandleW(PCWSTR::null());
        debug_assert!(!instance.is_null());

        // ---- Class 1: the message-only mailbox ------------------------
        // An ordinary class — nothing about "message-only" lives here;
        // the magic is entirely in CreateWindowExW's parent argument.
        let worker_class = WNDCLASSW {
            lpfnWndProc: Some(worker_wndproc),
            hInstance: instance,
            hCursor: LoadCursorW(None, PCWSTR(IDC_ARROW.0.cast())),
            hbrBackground: ((COLOR_WINDOW + 1) as isize) as HBRUSH,
            lpszClassName: w!("e76_worker"),
            ..Default::default() // no styles: it will never paint
        };
        debug_assert!(RegisterClassW(&worker_class) != 0);

        let wc = WNDCLASSW {
            lpfnWndProc: Some(wndproc),
            hInstance: instance,
            hCursor: LoadCursorW(None, PCWSTR(IDC_ARROW.0.cast())),
            hbrBackground: ((COLOR_WINDOW + 1) as isize) as HBRUSH,
            lpszClassName: w!("e76_class"),
            style: (CS_HREDRAW | CS_VREDRAW) as u32,
            ..Default::default()
        };
        debug_assert!(RegisterClassW(&wc) != 0);

        // ---- The mailbox FIRST, so WM_CREATE of the main window can
        // stash its handle in state (the main window needs a post target
        // from its very first timer tick).
        let worker = CreateWindowExW(
            0,
            w!("e76_worker"),
            w!("invisible mailbox"), // name findable via FindWindowEx
            0,                       // no styles — it never shows anything
            0, 0, 0, 0, // position/size meaningless for a window never drawn
            Some(HWND_MESSAGE), // <-- THE line: parent = pseudo-handle -3
            None,
            Some(instance),
            None,
        );
        debug_assert!(!worker.is_null());

        let state = Box::into_raw(Box::new(State { worker, sent: 0 }));

        let hwnd = CreateWindowExW(
            0,
            w!("e76_class"),
            w!("e76 · Message-Only Window — watch the console"),
            (WS_OVERLAPPEDWINDOW | WS_VISIBLE) as u32,
            CW_USEDEFAULT, CW_USEDEFAULT, 560, 300,
            None, None, Some(instance),
            Some(state as LPVOID),
        );
        debug_assert!(!hwnd.is_null());
        if hwnd.is_null() {
            drop(Box::from_raw(state));
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

fn to_wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

extern "system" fn wndproc(hwnd: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe {
        match message as i32 {
            WM_NCCREATE => {
                let cs = lparam as *const CREATESTRUCTW;
                SetWindowLongPtrW(hwnd, GWLP_USERDATA, (*cs).lpCreateParams as isize);
                DefWindowProcW(hwnd, message, wparam, lparam)
            }

            WM_CREATE => {
                // The metronome: one WM_TIMER per TICK_MS on this window.
                _ = SetTimer(Some(hwnd), TIMER_ID, TICK_MS, None);
                0
            }

            WM_TIMER => {
                if wparam == TIMER_ID {
                    if let Some(state) = state_of(hwnd) {
                        if !state.worker.is_null() {
                            state.sent += 1;
                            // Alternate the message id so the mailbox's
                            // match arms both get exercise. POST (async)
                            // not SEND: the point of a mailbox is that the
                            // sender never waits.
                            let msg = if state.sent % 2 == 1 { WM_APP_PING } else { WM_APP_NOTE };
                            _ = PostMessageW(
                                Some(state.worker),
                                msg as u32,
                                state.sent as WPARAM,
                                (state.sent as isize * TICK_MS as isize) as LPARAM,
                            );
                            _ = InvalidateRect(Some(hwnd), None, false);
                        }
                    }
                }
                0
            }

            WM_PAINT => {
                let mut ps = PAINTSTRUCT::default();
                let hdc = BeginPaint(hwnd, &mut ps);
                let mut rc = RECT::default();
                _ = GetClientRect(hwnd, &mut rc);

                if let Some(state) = state_of(hwnd) {
                    let wide = to_wide(&format!(
                        "posted {} messages to the invisible mailbox\n\
                         (console shows each delivery — WM_PAINT here is just the scoreboard)\n\n\
                         the mailbox: parent HWND_MESSAGE, no styles, no pixels,\n\
                         but a fully functioning wndproc for posted messages.",
                        state.sent
                    ));
                    _ = SetBkMode(hdc, TRANSPARENT);
                    DrawTextW(
                        hdc,
                        PCWSTR::from_raw(wide.as_ptr()),
                        -1,
                        &mut rc,
                        (DT_CENTER | DT_VCENTER | DT_WORDBREAK) as u32,
                    );
                }

                _ = EndPaint(hwnd, &ps);
                0
            }

            WM_NCDESTROY => {
                let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut State;
                if !ptr.is_null() {
                    SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
                    drop(Box::from_raw(ptr));
                }
                DefWindowProcW(hwnd, message, wparam, lparam)
            }

            WM_DESTROY => {
                _ = KillTimer(Some(hwnd), TIMER_ID);
                PostQuitMessage(0);
                0
            }

            _ => DefWindowProcW(hwnd, message, wparam, lparam),
        }
    }
}

// ======================================================================
// The message-only window's procedure — a full wndproc for a window
// nobody will ever see.
// ======================================================================
extern "system" fn worker_wndproc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    unsafe {
        // Same `as i32` cast as every other wndproc — our WM_APP+n
        // constants are i32, like all WM_* constants in this crate.
        match message as i32 {
            WM_APP_PING => {
                println!(
                    "[mailbox] PING  #{} ({} ms since start)",
                    wparam, lparam
                );
                0
            }
            WM_APP_NOTE => {
                println!(
                    "[mailbox] NOTE  #{} ({} ms since start) — same queue, different id",
                    wparam, lparam
                );
                0
            }
            WM_DESTROY => {
                // No PostQuitMessage here! That would end the whole
                // thread's message loop while the main window lives.
                // Quit messages belong to exactly one window: the main one.
                0
            }
            _ => DefWindowProcW(hwnd, message, wparam, lparam),
        }
    }
}
