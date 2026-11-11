//! e08 — Client Rect: getting the exact drawable size you asked for
//!
//! What you learn:
//! * Window size ≠ client size: `CreateWindowExW`'s width/height include
//!   the title bar and borders
//! * `AdjustWindowRect` — "I want THIS client area; how big must the whole
//!   window be?" (it grows a rect by the frame for a given style)
//! * `FillRect` + system color brushes for quick solid fills
//!
//! The window shows an 8×8 checkerboard whose client area is *exactly*
//! 512×512 pixels — every square a crisp 64px, no partial cells.
//!
//! New Win32 APIs:
//! * `AdjustWindowRect` — client rect → required window rect
//! * `FillRect`, `GetSysColorBrush` — fill a rectangle with a stock brush
//!
//! Run: `cargo run -p e08_client_rect`

use windows::{Win32::*, core::*};

const CELLS: i32 = 8; // squares per side
const CELL: i32 = 64; // pixels per square

fn main() {
    unsafe {
        let instance = GetModuleHandleW(PCWSTR::null());
        debug_assert!(!instance.is_null());

        let wc = WNDCLASSW {
            lpfnWndProc: Some(wndproc),
            hInstance: instance,
            hCursor: LoadCursorW(None, PCWSTR(IDC_ARROW.0.cast())),
            hbrBackground: ((COLOR_WINDOW + 1) as isize) as HBRUSH,
            lpszClassName: w!("e08_class"),
            style: (CS_HREDRAW | CS_VREDRAW) as u32,
            ..Default::default()
        };
        debug_assert!(RegisterClassW(&wc) != 0);

        // Fixed-size look: no thick (resizable) frame, no maximize button.
        // The style matters to AdjustWindowRect — different frames have
        // different thicknesses — so compute it once and reuse it.
        let style = (WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU | WS_MINIMIZEBOX | WS_VISIBLE) as u32;

        // Start from the client rectangle we WANT...
        let mut rect = RECT {
            left: 0,
            top: 0,
            right: CELLS * CELL,
            bottom: CELLS * CELL,
        };
        // ...and let Windows inflate it to the full window rectangle for
        // this style. (The last argument says whether the window has a
        // menu bar, which would add height — ours doesn't.)
        // After the call, left/top are negative: the frame sticks out
        // around the client area on all sides.
        _ = AdjustWindowRect(&mut rect, style, false);

        let hwnd = CreateWindowExW(
            0,
            w!("e08_class"),
            w!("e08 · Client Rect — exactly 512 x 512"),
            style,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            rect.right - rect.left, // full window width
            rect.bottom - rect.top, // full window height
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

extern "system" fn wndproc(hwnd: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe {
        match message as i32 {
            WM_PAINT => {
                let mut ps = PAINTSTRUCT::default();
                let hdc = BeginPaint(hwnd, &mut ps);

                // System color brushes are owned by Windows — no cleanup
                // needed (unlike brushes you create; see e23).
                let light = GetSysColorBrush(COLOR_WINDOW);
                let dark = GetSysColorBrush(COLOR_WINDOWTEXT);

                for row in 0..CELLS {
                    for col in 0..CELLS {
                        let cell = RECT {
                            left: col * CELL,
                            top: row * CELL,
                            right: (col + 1) * CELL,
                            bottom: (row + 1) * CELL,
                        };
                        // Classic checkerboard parity.
                        let brush = if (row + col) % 2 == 0 { light } else { dark };
                        FillRect(hdc, &cell, brush);
                    }
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
