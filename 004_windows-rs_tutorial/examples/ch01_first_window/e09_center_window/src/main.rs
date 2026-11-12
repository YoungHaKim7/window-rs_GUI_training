//! e09 — Center Window: screen metrics and window placement
//!
//! What you learn:
//! * `GetSystemMetrics` — the query API for screen width/height, border
//!   sizes, scrollbar widths, "is it a touchscreen?" ... one function,
//!   ~100 SM_* indices
//! * Centering a window: compute (screen - window) / 2 before creating it
//! * `GetWindowRect` vs `GetClientRect`: full window in *screen*
//!   coordinates vs drawable area in *client* coordinates
//! * `WM_MOVE` + `InvalidateRect`: repaint when the window moves
//!
//! Drag the window around — the caption tracks its live position.
//!
//! New Win32 APIs:
//! * `GetSystemMetrics` (SM_CXSCREEN / SM_CYSCREEN)
//! * `GetWindowRect`, `WM_MOVE`, `InvalidateRect`
//!
//! Run: `cargo run -p e09_center_window`

use windows::{Win32::*, core::*};

const WIDTH: i32 = 480;
const HEIGHT: i32 = 300;

fn main() {
    unsafe {
        let instance = GetModuleHandleW(PCWSTR::null());
        debug_assert!(!instance.is_null());

        let wc = WNDCLASSW {
            lpfnWndProc: Some(wndproc),
            hInstance: instance,
            hCursor: LoadCursorW(None, PCWSTR(IDC_ARROW.0.cast())),
            hbrBackground: ((COLOR_WINDOW + 1) as isize) as HBRUSH,
            lpszClassName: w!("e09_class"),
            style: (CS_HREDRAW | CS_VREDRAW) as u32,
            ..Default::default()
        };
        debug_assert!(RegisterClassW(&wc) != 0);

        // Ask the system how big the (primary) screen is. For multi-monitor
        // arrangements people usually prefer MonitorFromWindow +
        // GetMonitorInfoW — try that as homework once you reach e91.
        let screen_w = GetSystemMetrics(SM_CXSCREEN);
        let screen_h = GetSystemMetrics(SM_CYSCREEN);

        // Halves rounding down is fine: an odd difference would leave a
        // 0.5px imbalance — invisible.
        let x = (screen_w - WIDTH) / 2;
        let y = (screen_h - HEIGHT) / 2;

        let hwnd = CreateWindowExW(
            0,
            w!("e09_class"),
            w!("e09 · Center Window"),
            (WS_OVERLAPPEDWINDOW | WS_VISIBLE) as u32,
            x,
            y,
            WIDTH,
            HEIGHT,
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

                // Where is the whole window on screen right now? (Compare:
                // GetClientRect always returns (0,0)-(w,h) — position-free.)
                let mut wr = RECT::default();
                _ = GetWindowRect(hwnd, &mut wr);

                let screen_w = GetSystemMetrics(SM_CXSCREEN);
                let screen_h = GetSystemMetrics(SM_CYSCREEN);
                let text = HSTRING::from(format!(
                    "Screen: {screen_w} x {screen_h}\n\
                     Window: ({}, {}) size {} x {}\n\n\
                     Drag me — WM_MOVE keeps this readout fresh.",
                    wr.left,
                    wr.top,
                    wr.right - wr.left,
                    wr.bottom - wr.top
                ));

                let mut rect = RECT::default();
                _ = GetClientRect(hwnd, &mut rect);
                rect.left += 24;
                rect.top += 24;
                rect.right -= 24;
                rect.bottom -= 24;

                // DT_WORDBREAK: wrap at the rect edges.
                DrawTextW(
                    hdc,
                    PCWSTR(text.as_ptr()),
                    -1,
                    &mut rect,
                    (DT_WORDBREAK | DT_NOCLIP) as u32,
                );

                _ = EndPaint(hwnd, &ps);
                0
            }

            // Fired every time the window moves. Ask for a full repaint so
            // the position readout stays current (NULL rect = whole window).
            WM_MOVE => {
                _ = InvalidateRect(Some(hwnd), None, true);
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
