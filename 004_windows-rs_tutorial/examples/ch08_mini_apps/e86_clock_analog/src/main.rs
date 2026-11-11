//! e86 — Analog Clock: GetLocalTime, angles, and a smooth double buffer
//!
//! What you learn:
//! * **Hand math**: the time-to-angle mapping is the whole program —
//!   hour hand sweeps 30°/hour, minute 6°/minute, second 6°/second, all
//!   measured clockwise from 12. GDI's y-axis points DOWN, so the
//!   classic formula is `x = cx + len·sin(a)`, `y = cy − len·cos(a)`
//!   (mind the minus, or your clock runs upside-down).
//! * `GetLocalTime` → `SYSTEMTIME` (sysinfoapi): a filled-in struct,
//!   no handle, no cleanup — the friendliest time API Win32 has.
//! * A **500 ms timer**, not 1000: the second hand should not visibly
//!   lag behind the system clock at repaint moments; coalescing to
//!   half-second ticks keeps it honest without doubling the paint load
//!   (repaints are cheap because e26's double buffer makes them one
//!   BitBlt).
//! * Tick marks drawn with the same angle formula — 60 minor, 12 major
//!   (the loop is over angles; nothing is hand-placed).
//! * Resize-aware geometry: the face derives from the client rect every
//!   paint, so stretching the window stretches the clock.
//!
//! Run: `cargo run -p e86_clock_analog`

use windows::{Win32::*, core::*};

const TIMER_ID: usize = 1;
const TICK_MS: u32 = 500;

fn main() {
    unsafe {
        let instance = GetModuleHandleW(PCWSTR::null());
        debug_assert!(!instance.is_null());

        let wc = WNDCLASSW {
            lpfnWndProc: Some(wndproc),
            hInstance: instance,
            hCursor: LoadCursorW(None, PCWSTR(IDC_ARROW.0.cast())),
            hbrBackground: ((COLOR_WINDOW + 1) as isize) as HBRUSH,
            lpszClassName: w!("e86_class"),
            style: (CS_HREDRAW | CS_VREDRAW) as u32,
            ..Default::default()
        };
        debug_assert!(RegisterClassW(&wc) != 0);

        let hwnd = CreateWindowExW(
            0,
            w!("e86_class"),
            w!("e86 · Analog Clock"),
            (WS_OVERLAPPEDWINDOW | WS_VISIBLE) as u32,
            CW_USEDEFAULT, CW_USEDEFAULT, 420, 460,
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

fn rgb(red: u8, green: u8, blue: u8) -> COLORREF {
    (red as u32) | ((green as u32) << 8) | ((blue as u32) << 16)
}

fn to_wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Point on a circle: angle in RADIANS measured clockwise from 12
/// o'clock. Because GDI's y grows downward, cos subtracts (see header).
fn on_circle(cx: i32, cy: i32, radius: f64, angle_deg: f64) -> (i32, i32) {
    let a = angle_deg.to_radians();
    (
        cx + (a.sin() * radius).round() as i32,
        cy - (a.cos() * radius).round() as i32, // MINUS: y is inverted
    )
}

/// Paint the whole clock face into any DC (offscreen when called from
/// WM_PAINT's double buffer).
unsafe fn draw_clock(hdc: HDC, w: i32, h: i32, now: &SYSTEMTIME) {
    unsafe {
        let cx = w / 2;
        let cy = h / 2;
        let radius = (w.min(h) as f64 / 2.0 - 24.0).max(40.0);

        // Dial: white disc with a dark rim.
        let face = CreateSolidBrush(rgb(0xFA, 0xFA, 0xF6));
        _ = SelectObject(hdc, face as HGDIOBJ);
        let rim = CreatePen(PS_SOLID, 3, rgb(0x30, 0x30, 0x30));
        let old_pen = SelectObject(hdc, rim as HGDIOBJ);
        _ = Ellipse(
            hdc,
            (cx as f64 - radius) as i32,
            (cy as f64 - radius) as i32,
            (cx as f64 + radius) as i32,
            (cy as f64 + radius) as i32,
        );
        SelectObject(hdc, old_pen);
        _ = DeleteObject(rim as HGDIOBJ);
        _ = SelectObject(hdc, GetStockObject(NULL_BRUSH));
        _ = DeleteObject(face as HGDIOBJ);

        // Ticks: 60 around, every 5th one major. Same angle math as the
        // hands — a loop, not 60 literals.
        let minor = CreatePen(PS_SOLID, 1, rgb(0x90, 0x90, 0x90));
        let major = CreatePen(PS_SOLID, 3, rgb(0x30, 0x30, 0x30));
        for i in 0..60 {
            let angle = i as f64 * 6.0; // 360 / 60
            let is_major = i % 5 == 0;
            let len = if is_major { 10.0 } else { 5.0 };
            let (x1, y1) = on_circle(cx, cy, radius - len, angle);
            let (x2, y2) = on_circle(cx, cy, radius - 2.0, angle);
            let pen = if is_major { major } else { minor };
            let old = SelectObject(hdc, pen as HGDIOBJ);
            _ = MoveToEx(hdc, x1, y1, None);
            _ = LineTo(hdc, x2, y2);
            SelectObject(hdc, old);
        }
        _ = DeleteObject(minor as HGDIOBJ);
        _ = DeleteObject(major as HGDIOBJ);

        // Hands: hour (short/thick), minute (long/medium), second
        // (thin/red, with a tail past the center like real movements).
        let (hf, mf, sf) = (
            now.wHour as f64 % 12.0 + now.wMinute as f64 / 60.0,
            now.wMinute as f64 + now.wSecond as f64 / 60.0,
            now.wSecond as f64,
        );
        let hand = |degrees_per_unit: f64, value: f64, length: f64, width: i32, color: COLORREF, tail: f64| {
            let angle = value * degrees_per_unit;
            let (hx, hy) = on_circle(cx, cy, length, angle);
            let (tx, ty) = on_circle(cx, cy, tail, angle + 180.0);
            let pen = CreatePen(PS_SOLID, width, color);
            let old = SelectObject(hdc, pen as HGDIOBJ);
            _ = MoveToEx(hdc, tx, ty, None);
            _ = LineTo(hdc, hx, hy);
            SelectObject(hdc, old);
            _ = DeleteObject(pen as HGDIOBJ);
        };
        hand(30.0, hf, radius * 0.5, 6, rgb(0x20, 0x20, 0x20), 10.0); // hour
        hand(6.0, mf, radius * 0.75, 4, rgb(0x20, 0x20, 0x20), 12.0); // minute
        hand(6.0, sf, radius * 0.85, 1, rgb(0xC0, 0x30, 0x30), 18.0); // second

        // Hub cap.
        let hub = CreateSolidBrush(rgb(0x30, 0x30, 0x30));
        let old_brush = SelectObject(hdc, hub as HGDIOBJ);
        _ = Ellipse(hdc, cx - 5, cy - 5, cx + 5, cy + 5);
        SelectObject(hdc, old_brush);
        _ = DeleteObject(hub as HGDIOBJ);

        // Date below the hub — SYSTEMTIME carries it, so no second API.
        _ = SetBkMode(hdc, TRANSPARENT);
        _ = SetTextColor(hdc, rgb(0x40, 0x40, 0x40));
        let date = format!("{:04}-{:02}-{:02}", now.wYear, now.wMonth, now.wDay);
        let wide = to_wide(&date);
        let (dx, dy) = on_circle(cx, cy, radius * 0.35, 180.0); // straight down
        _ = TextOutW(hdc, dx - 36, dy - 8, PCWSTR::from_raw(wide.as_ptr()), -1);
    }
}

extern "system" fn wndproc(hwnd: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe {
        match message as i32 {
            WM_CREATE => {
                _ = SetTimer(Some(hwnd), TIMER_ID, TICK_MS, None);
                0
            }

            WM_TIMER => {
                if wparam == TIMER_ID {
                    // Nothing is drawn here — tick just invalidates, the
                    // paint path does the work. 500 ms: see header.
                    _ = InvalidateRect(Some(hwnd), None, false);
                }
                0
            }

            WM_ERASEBKGND => 1, // e26: we own the erase, no white flash

            WM_PAINT => {
                let mut ps = PAINTSTRUCT::default();
                let hdc = BeginPaint(hwnd, &mut ps);
                let mut rc = RECT::default();
                _ = GetClientRect(hwnd, &mut rc);
                let (w, h) = (rc.right, rc.bottom);

                if w > 0 && h > 0 {
                    // The time is read ONCE per frame — all three hands
                    // (and the date) belong to the same instant.
                    let mut now = SYSTEMTIME::default();
                    GetLocalTime(&mut now);

                    // e26 double buffer: draw offscreen, BitBlt once.
                    let memdc = CreateCompatibleDC(Some(hdc));
                    let bitmap = CreateCompatibleBitmap(hdc, w, h); // screen-compatible!
                    let old_bitmap = SelectObject(memdc, bitmap as HGDIOBJ);

                    let paper = CreateSolidBrush(rgb(0xE8, 0xEC, 0xF2));
                    _ = FillRect(memdc, &rc, paper);
                    _ = DeleteObject(paper as HGDIOBJ);

                    draw_clock(memdc, w, h, &now);

                    _ = BitBlt(hdc, 0, 0, w, h, Some(memdc), 0, 0, SRCCOPY);

                    SelectObject(memdc, old_bitmap);
                    _ = DeleteObject(bitmap as HGDIOBJ);
                    _ = DeleteDC(memdc);
                }

                _ = EndPaint(hwnd, &ps);
                0
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
