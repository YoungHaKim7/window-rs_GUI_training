//! e10 — Window Styles: a live tour of WS_ / WS_EX_ flags
//!
//! What you learn:
//! * Style bits live in two places: `GWL_STYLE` (everyday look) and
//!   `GWL_EXSTYLE` (the exotic ones) — read and written with
//!   `GetWindowLongPtrW` / `SetWindowLongPtrW`
//! * A style change needs `SetWindowPos(..., SWP_FRAMECHANGED)` before
//!   Windows actually re-dresses the frame
//! * Which WS_ bits make up `WS_OVERLAPPEDWINDOW` — toggle them one by one
//! * `WS_EX_TOPMOST` — the polite way to float above everything (via
//!   `HWND_TOPMOST` rather than poking the bit directly)
//!
//! Keys: [1] title bar  [2] resize border  [3] minimize box  [4] maximize
//! box  [5] system menu  [T] topmost  [Esc] quit
//! (With the caption gone you can't drag the window — that's the point!)
//!
//! Run: `cargo run -p e10_window_styles`

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
            lpszClassName: w!("e10_class"),
            style: (CS_HREDRAW | CS_VREDRAW) as u32,
            ..Default::default()
        };
        debug_assert!(RegisterClassW(&wc) != 0);

        let hwnd = CreateWindowExW(
            0,
            w!("e10_class"),
            w!("e10 · Window Styles"),
            (WS_OVERLAPPEDWINDOW | WS_VISIBLE) as u32,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            560,
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

/// Apply a new style and force Windows to rebuild the frame.
unsafe fn apply_style(hwnd: HWND, new_style: u32) {
    unsafe {
        SetWindowLongPtrW(hwnd, GWL_STYLE, new_style as isize);
        // SWP_FRAMECHANGED is the magic flag: "recompute the frame from
        // the style, even though size/position didn't change".
        _ = SetWindowPos(
            hwnd,
            None,
            0,
            0,
            0,
            0,
            (SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_FRAMECHANGED) as u32,
        );
        _ = InvalidateRect(Some(hwnd), None, true);
    }
}

extern "system" fn wndproc(hwnd: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe {
        match message as i32 {
            WM_KEYDOWN => {
                // For digits and letters the virtual-key code equals ASCII,
                // so casting to char keeps the match readable.
                match wparam as u8 as char {
                    '1'..='5' => {
                        let bit = [
                            WS_CAPTION,
                            WS_THICKFRAME,
                            WS_MINIMIZEBOX,
                            WS_MAXIMIZEBOX,
                            WS_SYSMENU,
                        ][(wparam as u8 - b'1') as usize];
                        let style = GetWindowLongPtrW(hwnd, GWL_STYLE) as u32;
                        apply_style(hwnd, style ^ bit as u32); // XOR = toggle
                        0
                    }
                    'T' | 't' => {
                        // Topmost is better flipped via SetWindowPos's
                        // "insert after" argument than by editing WS_EX_*.
                        let ex = GetWindowLongPtrW(hwnd, GWL_EXSTYLE) as u32;
                        let is_topmost = ex & WS_EX_TOPMOST as u32 != 0;
                        _ = SetWindowPos(
                            hwnd,
                            Some(if is_topmost {
                                HWND_NOTOPMOST
                            } else {
                                HWND_TOPMOST
                            }),
                            0,
                            0,
                            0,
                            0,
                            (SWP_NOMOVE | SWP_NOSIZE) as u32,
                        );
                        _ = InvalidateRect(Some(hwnd), None, true);
                        0
                    }
                    // VK_ESCAPE (0x1B) — patterns can't contain `as` casts,
                    // so spell the virtual key as its character literal.
                    '\u{1b}' => {
                        // Safety hatch: with the caption toggled off there's
                        // no close button — Esc still works.
                        _ = DestroyWindow(hwnd);
                        0
                    }
                    _ => 0,
                }
            }
            WM_PAINT => {
                let mut ps = PAINTSTRUCT::default();
                let hdc = BeginPaint(hwnd, &mut ps);

                let style = GetWindowLongPtrW(hwnd, GWL_STYLE) as u32;
                let ex_style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE) as u32;

                let mark = |on: bool| if on { "[x]" } else { "[ ]" };
                let lines = [
                    format!(
                        "{} [1]  WS_CAPTION      title bar + draggable frame",
                        mark(style & WS_CAPTION as u32 != 0)
                    ),
                    format!(
                        "{} [2]  WS_THICKFRAME   resize borders",
                        mark(style & WS_THICKFRAME as u32 != 0)
                    ),
                    format!(
                        "{} [3]  WS_MINIMIZEBOX  minimize button",
                        mark(style & WS_MINIMIZEBOX as u32 != 0)
                    ),
                    format!(
                        "{} [4]  WS_MAXIMIZEBOX  maximize button",
                        mark(style & WS_MAXIMIZEBOX as u32 != 0)
                    ),
                    format!(
                        "{} [5]  WS_SYSMENU      system menu (X / Alt+Space)",
                        mark(style & WS_SYSMENU as u32 != 0)
                    ),
                    format!(
                        "{} [T]  WS_EX_TOPMOST   float above other windows",
                        mark(ex_style & WS_EX_TOPMOST as u32 != 0)
                    ),
                    String::new(),
                    "Esc = quit".to_string(),
                ];

                let mut rect = RECT::default();
                _ = GetClientRect(hwnd, &mut rect);
                for (i, line) in lines.iter().enumerate() {
                    let mut r = rect;
                    r.top += 24 + i as i32 * 26;
                    let wide = to_wide(line);
                    DrawTextW(
                        hdc,
                        PCWSTR::from_raw(wide.as_ptr()),
                        -1,
                        &mut r,
                        (DT_LEFT | DT_SINGLELINE) as u32,
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
