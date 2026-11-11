//! e12 — Mouse Clicks: button messages and drawing outside WM_PAINT
//!
//! What you learn:
//! * `WM_LBUTTONDOWN` / `WM_RBUTTONDOWN` / `WM_MBUTTONDOWN` — and where the
//!   position hides: `lparam`, two signed 16-bit values packed into one isize
//! * `wparam` carries the modifier flags (`MK_SHIFT`, `MK_CONTROL`, ...)
//! * `GetDC` / `ReleaseDC` — painting *outside* WM_PAINT (fine for effects;
//!  not for content that must survive a repaint)
//! * `CS_DBLCLKS` — the class flag that makes double-clicks arrive as
//!   `WM_LBUTTONDBLCLK` instead of a click-stream
//!
//! Left click = blue dot, right = red, middle = green, double-click = ring.
//! Then minimize/restore the window: the dots VANISH — nothing remembers
//! them. That's the core lesson: durable drawing needs stored state drawn
//! in WM_PAINT, which is exactly where e13 takes you next.
//!
//! Run: `cargo run -p e12_mouse_clicks`

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
            lpszClassName: w!("e12_class"),
            // Without CS_DBLCLKS a double-click is just DOWN/UP/DOWN/UP;
            // with it, the second click arrives as WM_LBUTTONDBLCLK.
            style: (CS_HREDRAW | CS_VREDRAW | CS_DBLCLKS) as u32,
            ..Default::default()
        };
        debug_assert!(RegisterClassW(&wc) != 0);

        let hwnd = CreateWindowExW(
            0,
            w!("e12_class"),
            w!("e12 · Mouse Clicks — minimize/restore me"),
            (WS_OVERLAPPEDWINDOW | WS_VISIBLE) as u32,
            CW_USEDEFAULT, CW_USEDEFAULT, 640, 400,
            None, None, Some(instance), None,
        );
        debug_assert!(!hwnd.is_null());

        let mut message = MSG::default();
        while GetMessageW(&mut message, None, 0, 0).0 > 0 {
            _ = TranslateMessage(&message);
            DispatchMessageW(&message);
        }
    }
}

/// Mouse messages pack the position into lparam: low 16 bits = x, high 16
/// bits = y. Both are SIGNED shorts — they go negative when the mouse is
/// captured and dragged outside the window (e13 demos capture) — so the
/// casts go through i16, never u16.
fn mouse_pos(lparam: LPARAM) -> (i32, i32) {
    let x = (lparam & 0xffff) as i16 as i32;
    let y = ((lparam >> 16) & 0xffff) as i16 as i32;
    (x, y)
}

/// COLORREF is 0x00BBGGRR — blue in the high bits, red in the low.
fn rgb(red: u8, green: u8, blue: u8) -> COLORREF {
    (red as u32) | ((green as u32) << 8) | ((blue as u32) << 16)
}

/// Draw a filled circle at a client position, right now.
/// Painting outside WM_PAINT: GetDC borrows a fresh DC, ReleaseDC returns
/// it. Totally legitimate — just remember Windows owes you nothing: the
/// next repaint erases everything drawn this way.
unsafe fn dot(hwnd: HWND, x: i32, y: i32, color: COLORREF, radius: i32) {
    unsafe {
        let hdc = GetDC(Some(hwnd));
        let brush = CreateSolidBrush(color);
        let old_brush = SelectObject(hdc, brush as HGDIOBJ);
        _ = Ellipse(hdc, x - radius, y - radius, x + radius, y + radius);
        SelectObject(hdc, old_brush);
        _ = DeleteObject(brush as HGDIOBJ);
        ReleaseDC(Some(hwnd), hdc);
    }
}

extern "system" fn wndproc(hwnd: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe {
        match message as i32 {
            WM_LBUTTONDOWN | WM_RBUTTONDOWN | WM_MBUTTONDOWN | WM_LBUTTONDBLCLK => {
                let (x, y) = mouse_pos(lparam);

                // wparam's bits are the modifiers held at click time:
                // MK_SHIFT / MK_CONTROL / MK_LBUTTON ... (e18 covers
                // keyboard-state queries in depth).
                let modifiers = if wparam & MK_SHIFT as usize != 0 { "+SHIFT" } else { "" };
                println!("click at ({x:4},{y:4}){modifiers}");

                match message as i32 {
                    WM_LBUTTONDOWN => dot(hwnd, x, y, rgb(0x20, 0x60, 0xD0), 9),
                    WM_RBUTTONDOWN => dot(hwnd, x, y, rgb(0xD0, 0x30, 0x30), 9),
                    WM_MBUTTONDOWN => dot(hwnd, x, y, rgb(0x20, 0x90, 0x30), 9),
                    // A bigger, hollow ring for the double click.
                    _ => dot(hwnd, x, y, rgb(0x80, 0x20, 0xA0), 22),
                }
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
