//! e20 — Cursors: different pointers in different zones
//!
//! What you learn:
//! * The class `hCursor` is a blunt instrument — one cursor for the whole
//!   client area. Zone-specific cursors mean handling `WM_SETCURSOR`
//!   yourself (with the class cursor set to NULL so Windows doesn't fight
//!   you on every mouse move)
//! * `WM_SETCURSOR`: lparam's low word = the hit-test result, high word =
//!   the mouse message that triggered it. Handle it, `SetCursor(...)`,
//!   return 1 ("I did it"); return DefWindowProcW otherwise.
//! * `SetCursor` must be re-called EVERY time WM_SETCURSOR arrives — the
//!   cursor resets to the class default on each mouse move
//! * The stock cursor zoo: arrow, cross, hand, wait, I-beam...
//!
//! Move the mouse across the four quadrants.
//!
//! New Win32 APIs: `WM_SETCURSOR`, `SetCursor`, `IDC_CROSS`, `IDC_HAND`,
//! `IDC_WAIT`, `IDC_HELP`.
//!
//! Run: `cargo run -p e20_cursors`

use windows::{Win32::*, core::*};

fn main() {
    unsafe {
        let instance = GetModuleHandleW(PCWSTR::null());
        debug_assert!(!instance.is_null());

        let wc = WNDCLASSW {
            lpfnWndProc: Some(wndproc),
            hInstance: instance,
            // NULL class cursor: "I'll manage cursors myself." If we left
            // IDC_ARROW here, Windows would snap the arrow back on every
            // mouse move and our SetCursor calls would flicker and lose.
            hCursor: std::ptr::null_mut(),
            hbrBackground: ((COLOR_WINDOW + 1) as isize) as HBRUSH,
            lpszClassName: w!("e20_class"),
            style: (CS_HREDRAW | CS_VREDRAW) as u32,
            ..Default::default()
        };
        debug_assert!(RegisterClassW(&wc) != 0);

        let hwnd = CreateWindowExW(
            0,
            w!("e20_class"),
            w!("e20 · Cursors — sweep the mouse across the quadrants"),
            (WS_OVERLAPPEDWINDOW | WS_VISIBLE) as u32,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            640,
            480,
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

fn rgb(red: u8, green: u8, blue: u8) -> COLORREF {
    (red as u32) | ((green as u32) << 8) | ((blue as u32) << 16)
}

extern "system" fn wndproc(hwnd: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe {
        match message as i32 {
            WM_SETCURSOR => {
                // lparam = MAKELONG(hit-test, mouse-message). Only take
                // over when the question is about the CLIENT area — frame
                // zones (resize borders etc.) keep default behavior.
                let hit = (lparam & 0xffff) as u16 as i32;
                if hit == HTCLIENT {
                    // Where is the mouse in client coordinates right now?
                    let mut point = POINT::default();
                    _ = GetCursorPos(&mut point);
                    _ = ScreenToClient(hwnd, &mut point);

                    let mut rect = RECT::default();
                    _ = GetClientRect(hwnd, &mut rect);
                    let right_half = point.x >= rect.right / 2;
                    let bottom_half = point.y >= rect.bottom / 2;

                    let cursor = match (right_half, bottom_half) {
                        (false, false) => LoadCursorW(None, PCWSTR(IDC_ARROW.0.cast())),
                        (true, false) => LoadCursorW(None, PCWSTR(IDC_CROSS.0.cast())),
                        (false, true) => LoadCursorW(None, PCWSTR(IDC_HAND.0.cast())),
                        (true, true) => LoadCursorW(None, PCWSTR(IDC_WAIT.0.cast())),
                    };

                    // Remember: the system resets the cursor on mouse
                    // movement — this call is needed on EVERY WM_SETCURSOR,
                    // not just once.
                    _ = SetCursor(Some(cursor));

                    // Returning TRUE (1): "handled, don't touch it."
                    // Falling back to DefWindowProcW would re-apply the
                    // (null) class cursor, i.e. erase our choice.
                    1
                } else {
                    DefWindowProcW(hwnd, message, wparam, lparam)
                }
            }
            WM_PAINT => {
                let mut ps = PAINTSTRUCT::default();
                let hdc = BeginPaint(hwnd, &mut ps);

                let mut rect = RECT::default();
                _ = GetClientRect(hwnd, &mut rect);

                // Four tinted quadrants with labels.
                let (w, h) = (rect.right, rect.bottom);
                let quads: [(RECT, COLORREF, &str); 4] = [
                    (
                        RECT {
                            left: 0,
                            top: 0,
                            right: w / 2,
                            bottom: h / 2,
                        },
                        rgb(0xF5, 0xF5, 0xF5),
                        "IDC_ARROW",
                    ),
                    (
                        RECT {
                            left: w / 2,
                            top: 0,
                            right: w,
                            bottom: h / 2,
                        },
                        rgb(0xE8, 0xF0, 0xE8),
                        "IDC_CROSS",
                    ),
                    (
                        RECT {
                            left: 0,
                            top: h / 2,
                            right: w / 2,
                            bottom: h,
                        },
                        rgb(0xF0, 0xE8, 0xF0),
                        "IDC_HAND",
                    ),
                    (
                        RECT {
                            left: w / 2,
                            top: h / 2,
                            right: w,
                            bottom: h,
                        },
                        rgb(0xF0, 0xF0, 0xE0),
                        "IDC_WAIT",
                    ),
                ];
                for (r, color, label) in quads {
                    let brush = CreateSolidBrush(color);
                    FillRect(hdc, &r, brush);
                    _ = DeleteObject(brush as HGDIOBJ);
                    let wide = to_wide(label);
                    _ = SetBkMode(hdc, TRANSPARENT);
                    _ = TextOutW(
                        hdc,
                        r.left + 20,
                        r.top + 20,
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

fn to_wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}
