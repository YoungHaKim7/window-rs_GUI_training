//! e11 — The Message Loop: watching Windows talk to your window
//!
//! What you learn:
//! * What actually flows through `GetMessageW` → `DispatchMessageW`: every
//!   message, with its `wparam`/`lparam`, printed to the console
//! * `DispatchMessageW` is the bridge: it looks up the window's wndproc and
//!   calls it — the return value of wndproc travels back to the caller of
//!   `SendMessageW`
//! * Posted vs sent messages, and why the loop's *position* in main means
//!   you can intercept anything (global hotkeys, idle work)
//! * The `PeekMessageW` alternative for games / animation loops (shown in
//!   comments — used for real in e27)
//!
//! Watch the console while you: move the mouse over the window, click,
//! press keys, resize, drag another window across this one...
//!
//! Run: `cargo run -p e11_message_loop`

use windows::{Win32::*, core::*};

fn main() {
    unsafe {
        let instance = GetModuleHandleW(PCWSTR::null());
        debug_assert!(!instance.is_null());

        let wc = WNDCLASSW {
            lpfnWndProc: Some(wndproc),
            hInstance: instance,
            hCursor: LoadCursorW(None, PCWSTR(IDC_ARROW.0.cast())),
            hbrBackground: ((COLOR_WINDOW + 1) as isize) as HBRUSH,
            lpszClassName: w!("e11_class"),
            style: (CS_HREDRAW | CS_VREDRAW) as u32,
            ..Default::default()
        };
        debug_assert!(RegisterClassW(&wc) != 0);

        let hwnd = CreateWindowExW(
            0,
            w!("e11_class"),
            w!("e11 · Message Loop — watch the console"),
            (WS_OVERLAPPEDWINDOW | WS_VISIBLE) as u32,
            CW_USEDEFAULT, CW_USEDEFAULT, 640, 320,
            None, None, Some(instance), None,
        );
        debug_assert!(!hwnd.is_null());

        // The pump. Note what each stage knows:
        let mut message = MSG::default();
        while GetMessageW(&mut message, None, 0, 0).0 > 0 {
            // (1) We see EVERY message here first — even ones destined for
            //     our own window. A great interception point; e27 and e94
            //     build real features on it.
            print_message("pump", &message);

            // (2) TranslateMessage: turns WM_KEYDOWN + keyboard layout into
            //     a WM_CHAR ("A" vs "a" vs "!" — e14 digs in).
            _ = TranslateMessage(&message);

            // (3) DispatchMessageW finds the target window's wndproc and
            //     calls it. After this line returns, the message is handled
            //     (for posted messages — sent messages skip the queue
            //     entirely and call wndproc directly).
            DispatchMessageW(&message);
        }

        // ── The PeekMessageW alternative (no blocking) ────────────────
        // When you can't afford to sleep waiting for input (games,
        // animation), poll instead and use the *idle* time yourself:
        //
        //     loop {
        //         while PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE).as_bool() {
        //             if msg.message == WM_QUIT as u32 { return; }
        //             _ = TranslateMessage(&msg);
        //             DispatchMessageW(&msg);
        //         }
        //         update_and_render(); // runs flat-out between events
        //     }
    }
}

/// Human-readable name for the messages worth recognizing on sight.
fn message_name(id: u32) -> String {
    let name = match id as i32 {
        WM_PAINT => "WM_PAINT",
        WM_SIZE => "WM_SIZE",
        WM_MOVE => "WM_MOVE",
        WM_MOUSEMOVE => "WM_MOUSEMOVE",
        WM_LBUTTONDOWN => "WM_LBUTTONDOWN",
        WM_LBUTTONUP => "WM_LBUTTONUP",
        WM_KEYDOWN => "WM_KEYDOWN",
        WM_KEYUP => "WM_KEYUP",
        WM_CHAR => "WM_CHAR",
        WM_TIMER => "WM_TIMER",
        WM_SETFOCUS => "WM_SETFOCUS",
        WM_KILLFOCUS => "WM_KILLFOCUS",
        WM_ERASEBKGND => "WM_ERASEBKGND",
        WM_DESTROY => "WM_DESTROY",
        WM_CLOSE => "WM_CLOSE",
        _ => return format!("{:#06x}", id),
    };
    name.to_string()
}

fn print_message(where_: &str, msg: &MSG) {
    println!(
        "[{where_:5}] {:<18} hwnd={:?} wparam={:#010x} lparam={:#014x}",
        message_name(msg.message),
        msg.hwnd,
        msg.wParam,
        msg.lParam
    );
}

extern "system" fn wndproc(hwnd: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe {
        // The same message arrives here (via DispatchMessageW) right after
        // the pump printed it — compare the console lines to feel the path.
        if message as i32 != WM_PAINT {
            // (WM_PAINT fires constantly; printing it would drown the rest.
            // Temporarily remove this filter if you want the full firehose.)
            let m = MSG {
                hwnd,
                message,
                wParam: wparam,
                lParam: lparam,
                ..Default::default()
            };
            print_message("wnd  ", &m);
        }

        match message as i32 {
            WM_DESTROY => {
                PostQuitMessage(0);
                0
            }
            _ => DefWindowProcW(hwnd, message, wparam, lparam),
        }
    }
}
