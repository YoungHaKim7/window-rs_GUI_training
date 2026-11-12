//! e21 — Pixels & Lines: the atom level of GDI
//!
//! What you learn:
//! * `SetPixelV` — the single-pixel primitive. Every pattern you can
//!   compute, you can draw (two procedural textures here: an XOR grid and
//!   radial rings)
//! * `MoveToEx` / `LineTo` — GDI's line drawing: MoveToEx positions an
//!   invisible cursor, LineTo draws *to* an endpoint (exclusive!) and
//!   leaves the cursor there
//! * A line fan is just a loop over angles — trigonometry as UI
//! * Why `SetPixelV` (fast, no return value) instead of `SetPixel`
//!
//! Stateless like e04: everything is recomputed from the client rect in
//! every WM_PAINT, so resizing "just works".
//!
//! New Win32 APIs: `SetPixelV`, `MoveToEx`, `LineTo` (all wingdi).
//!
//! Run: `cargo run -p e21_pixels_lines`

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
            lpszClassName: w!("e21_class"),
            style: (CS_HREDRAW | CS_VREDRAW) as u32,
            ..Default::default()
        };
        debug_assert!(RegisterClassW(&wc) != 0);

        let hwnd = CreateWindowExW(
            0,
            w!("e21_class"),
            w!("e21 · Pixels & Lines"),
            (WS_OVERLAPPEDWINDOW | WS_VISIBLE) as u32,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            800,
            520,
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

fn to_wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

extern "system" fn wndproc(hwnd: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe {
        match message as i32 {
            WM_PAINT => {
                let mut ps = PAINTSTRUCT::default();
                let hdc = BeginPaint(hwnd, &mut ps);

                let mut rc = RECT::default();
                _ = GetClientRect(hwnd, &mut rc);
                let (w, h) = (rc.right, rc.bottom);

                // Split the client area: textures on the left half,
                // line fan on the right half.
                let half = w / 2;

                // ---- Texture 1: XOR grid (top-left quadrant) -------------
                // The classic demoscene pattern: color = x XOR y. Cheap to
                // compute, hypnotic to look at. Step 2 px so the pattern
                // forms visible cells instead of dense 1px noise.
                for y in (20..h / 2 - 20).step_by(2) {
                    for x in (20..half - 20).step_by(2) {
                        let v = ((x ^ y) & 0xff) as u8;
                        _ = SetPixelV(hdc, x, y, rgb(v, v, v));
                    }
                }

                // ---- Texture 2: radial rings (bottom-left quadrant) ------
                // Distance from the quadrant center picks the hue; the
                // sine quantizes it into rings.
                let (cx, cy) = (half / 2, (h / 2 + h) / 2); // center of lower-left
                for y in h / 2 + 20..h - 20 {
                    for x in 20..half - 20 {
                        let dx = x - cx;
                        let dy = y - cy;
                        let dist = ((dx * dx + dy * dy) as f64).sqrt();
                        // rings: sin of distance, quantized to 3 bits
                        let band = (((dist * 0.35).sin() * 127.0 + 128.0) as u32) >> 5 << 5;
                        _ = SetPixelV(hdc, x, y, rgb(band as u8, 40, (255 - band) as u8));
                    }
                }

                // ---- Line fan (right half) --------------------------------
                // MoveToEx sets the (invisible) current-position cursor;
                // LineTo strokes from there to the endpoint — the endpoint
                // becomes the new cursor, and the *last* pixel is NOT
                // drawn (LineTo is exclusive, a GDI quirk dating to 1985).
                let (fx, fy) = (half + (w - half) / 2, h / 2); // fan hub
                let fan_r = (w - half) / 2 - 40;
                let pen = CreatePen(PS_SOLID, 1, rgb(0x30, 0x60, 0xC0));
                let old_pen = SelectObject(hdc, pen as HGDIOBJ);
                for i in 0..36 {
                    let angle = i as f64 * std::f64::consts::TAU / 36.0;
                    let ex = fx + (fan_r as f64 * angle.cos()) as i32;
                    let ey = fy + (fan_r as f64 * angle.sin()) as i32;
                    _ = MoveToEx(hdc, fx, fy, None);
                    _ = LineTo(hdc, ex, ey);
                }
                // A spiral on top, one continuous MoveToEx + many LineTo:
                // each LineTo continues from where the last one stopped.
                let spiral = CreatePen(PS_SOLID, 2, rgb(0xE0, 0x80, 0x20));
                SelectObject(hdc, spiral as HGDIOBJ);
                _ = MoveToEx(hdc, fx, fy, None);
                for i in 0..180 {
                    let t = i as f64 / 179.0;
                    let angle = t * std::f64::consts::TAU * 3.0;
                    let r = t * fan_r as f64;
                    _ = LineTo(
                        hdc,
                        fx + (r * angle.cos()) as i32,
                        fy + (r * angle.sin()) as i32,
                    );
                }
                SelectObject(hdc, old_pen);
                _ = DeleteObject(pen as HGDIOBJ);
                _ = DeleteObject(spiral as HGDIOBJ);

                // ---- Labels ------------------------------------------------
                _ = SetBkMode(hdc, TRANSPARENT);
                _ = SetTextColor(hdc, rgb(0x40, 0x40, 0x40));
                for (text, x, y) in [
                    ("SetPixelV · x XOR y texture", 20, 4),
                    ("SetPixelV · radial rings", 20, h / 2 + 4),
                    ("MoveToEx / LineTo · fan + spiral", half + 20, 4),
                ] {
                    let wide = to_wide(text);
                    _ = TextOutW(hdc, x, y, PCWSTR::from_raw(wide.as_ptr()), -1);
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
