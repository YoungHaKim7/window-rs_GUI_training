//! e18 — Shift State: asking "is that key down right now?"
//!
//! What you learn:
//! * `GetKeyState(vk)` — the key's state *as of the message being
//!   processed*. Bit 15 (the sign bit) = down, bit 0 = toggled (Caps Lock,
//!   Num Lock, Scroll Lock). This is the right call inside a handler.
//! * `GetAsyncKeyState(vk)` — the physical state *this instant*, ignoring
//!   the message queue. For polling loops (games), not for handlers.
//! * Why the difference matters: a handler running late still sees the
//!   modifiers that were held when the event happened — with GetKeyState.
//! * Combining modifiers with mouse clicks (Shift+click, Ctrl+click)
//!
//! Hold Shift / Ctrl / Alt, toggle Caps Lock, and click with modifiers.
//!
//! New Win32 APIs: `GetKeyState`, `GetAsyncKeyState`.
//!
//! Run: `cargo run -p e18_shift_state`

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
            lpszClassName: w!("e18_class"),
            style: (CS_HREDRAW | CS_VREDRAW) as u32,
            ..Default::default()
        };
        debug_assert!(RegisterClassW(&wc) != 0);

        let hwnd = CreateWindowExW(
            0,
            w!("e18_class"),
            w!("e18 · Shift State — hold modifiers, click"),
            (WS_OVERLAPPEDWINDOW | WS_VISIBLE) as u32,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            600,
            340,
            None,
            None,
            Some(instance),
            None,
        );
        debug_assert!(!hwnd.is_null());

        let mut message = MSG::default();
        while GetMessageW(&mut message, None, 0, 0).0 > 0 {
            _ = TranslateMessage(&message);
            DispatchMessageW(&message);
        }
    }
}

/// Down = the high (sign) bit of the returned SHORT, so "negative" is the
/// idiomatic test.
unsafe fn is_down(vk: i32) -> bool {
    unsafe { GetKeyState(vk) < 0 }
}

/// Toggled = the low bit. Only meaningful for the lock keys.
unsafe fn is_toggled(vk: i32) -> bool {
    unsafe { GetKeyState(vk) & 1 != 0 }
}

fn to_wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

extern "system" fn wndproc(hwnd: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe {
        match message as i32 {
            // Any key going down or up may have changed a modifier: repaint.
            WM_KEYDOWN | WM_KEYUP => {
                _ = InvalidateRect(Some(hwnd), None, true);
                0
            }
            // Alt arrives through the SYS variants. Repaint, then let the
            // default procedure have it so Alt+F4 & co. keep working.
            WM_SYSKEYDOWN | WM_SYSKEYUP => {
                _ = InvalidateRect(Some(hwnd), None, true);
                DefWindowProcW(hwnd, message, wparam, lparam)
            }
            WM_LBUTTONDOWN => {
                // GetKeyState answers for the moment THIS click was
                // generated — exactly what "Shift+click" should mean.
                // (Mouse messages also carry MK_SHIFT / MK_CONTROL in
                // wparam, as e12 showed; GetKeyState is the general tool
                // and the only way to ask about Alt.)
                let kind = match (is_down(VK_SHIFT), is_down(VK_CONTROL), is_down(VK_MENU)) {
                    (false, false, false) => "plain click",
                    (true, false, false) => "Shift+click  (extend selection?)",
                    (false, true, false) => "Ctrl+click   (toggle selection?)",
                    (false, false, true) => "Alt+click",
                    _ => "click with several modifiers",
                };
                let title = HSTRING::from(format!("e18 · last: {kind}"));
                _ = SetWindowTextW(hwnd, &title);
                0
            }
            WM_PAINT => {
                let mut ps = PAINTSTRUCT::default();
                let hdc = BeginPaint(hwnd, &mut ps);

                let mark = |on: bool| if on { "[x]" } else { "[ ]" };
                let lines = [
                    "GetKeyState — high bit: key is DOWN".to_string(),
                    format!("  {} Shift   (VK_SHIFT)", mark(is_down(VK_SHIFT))),
                    format!("  {} Ctrl    (VK_CONTROL)", mark(is_down(VK_CONTROL))),
                    format!("  {} Alt     (VK_MENU)", mark(is_down(VK_MENU))),
                    String::new(),
                    "GetKeyState — low bit: key is TOGGLED ON".to_string(),
                    format!("  {} Caps Lock (VK_CAPITAL)", mark(is_toggled(VK_CAPITAL))),
                    format!("  {} Num Lock  (VK_NUMLOCK)", mark(is_toggled(VK_NUMLOCK))),
                    String::new(),
                    // GetAsyncKeyState reads the hardware state right now,
                    // bypassing the queue. Same bit layout for "down".
                    format!(
                        "GetAsyncKeyState(VK_SHIFT) says: {}",
                        if GetAsyncKeyState(VK_SHIFT) < 0 {
                            "down"
                        } else {
                            "up"
                        }
                    ),
                ];
                for (i, line) in lines.iter().enumerate() {
                    let wide = to_wide(line);
                    _ = TextOutW(
                        hdc,
                        24,
                        20 + i as i32 * 24,
                        PCWSTR::from_raw(wide.as_ptr()),
                        -1,
                    );
                }

                _ = EndPaint(hwnd, &ps);
                0
            }
            WM_DESTROY => {
                PostQuitMessage(0);
                0
            }
            _ => DefWindowProcW(hwnd, message, wparam, lparam),
        }
    }
}
