//! e23 — Brushes: solid fills, hatch patterns, system brushes
//!
//! What you learn:
//! * `CreateSolidBrush` — a COLORREF in, an HBRUSH out; paints the insides
//!   of every filled shape (Rectangle, Ellipse, FillRect, ...)
//! * `CreateHatchBrush` — the six HS_* monochrome hatch patterns; the
//!   hatch's *background* color comes from the DC's `SetBkColor`
//!   (TRANSPARENT via SetBkMode also applies!)
//! * **Owned vs borrowed brushes**: everything you Create* must be
//!   `DeleteObject`'d — but `GetSysColorBrush` hands you system-owned
//!   brushes (the exact colors of the user's theme) that you must NOT
//!   delete
//! * `FillRect` — fill a rectangle directly with a brush (no pen involved)
//!
//! New Win32 APIs: `CreateSolidBrush`, `CreateHatchBrush`, `FillRect`,
//! `SetBkColor`, `GetSysColorBrush`.
//!
//! Run: `cargo run -p e23_brushes`

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
            lpszClassName: w!("e23_class"),
            style: (CS_HREDRAW | CS_VREDRAW) as u32,
            ..Default::default()
        };
        debug_assert!(RegisterClassW(&wc) != 0);

        let hwnd = CreateWindowExW(
            0,
            w!("e23_class"),
            w!("e23 · Brush Gallery"),
            (WS_OVERLAPPEDWINDOW | WS_VISIBLE) as u32,
            CW_USEDEFAULT, CW_USEDEFAULT, 780, 460,
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

extern "system" fn wndproc(hwnd: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe {
        match message as i32 {
            WM_PAINT => {
                let mut ps = PAINTSTRUCT::default();
                let hdc = BeginPaint(hwnd, &mut ps);

                _ = SetBkMode(hdc, TRANSPARENT); // labels sit on anything
                _ = SetTextColor(hdc, rgb(0x30, 0x30, 0x30));

                // A thin outline makes each swatch's extent visible.
                let pen = CreatePen(PS_SOLID, 1, rgb(0x80, 0x80, 0x80));
                let old_pen = SelectObject(hdc, pen as HGDIOBJ);

                // ---- Row 1: solid brushes ---------------------------------
                let solids: [(&str, COLORREF); 5] = [
                    ("Red", rgb(0xD0, 0x30, 0x30)),
                    ("Green", rgb(0x30, 0x90, 0x40)),
                    ("Blue", rgb(0x30, 0x60, 0xC0)),
                    ("DimGray", rgb(0x70, 0x70, 0x70)),
                    ("Cream", rgb(0xF0, 0xE8, 0xD0)),
                ];
                let mut x = 24;
                let (y, cw, ch) = (24, 130, 70);
                for (name, color) in solids {
                    // Create → select (remember old) → use → restore → delete.
                    // The same five-step etiquette as pens in e22.
                    let brush = CreateSolidBrush(color);
                    let old = SelectObject(hdc, brush as HGDIOBJ);
                    _ = Rectangle(hdc, x, y, x + cw, y + ch); // fills with brush, outlines with pen
                    SelectObject(hdc, old);
                    _ = DeleteObject(brush as HGDIOBJ);

                    let wide = to_wide(name);
                    _ = TextOutW(hdc, x + 8, y + ch + 4, PCWSTR::from_raw(wide.as_ptr()), -1);
                    x += cw + 16;
                }

                // ---- Row 2: the six hatch styles ---------------------------
                // HS_* constants (i32) name the pattern; the hatch strokes
                // use the brush color, the gaps use the DC background.
                let hatches: [(&str, i32); 6] = [
                    ("HS_HORIZONTAL", HS_HORIZONTAL),
                    ("HS_VERTICAL", HS_VERTICAL),
                    ("HS_FDIAGONAL", HS_FDIAGONAL), // forward: top-left → bottom-right
                    ("HS_BDIAGONAL", HS_BDIAGONAL), // backward: bottom-left → top-right
                    ("HS_CROSS", HS_CROSS),
                    ("HS_DIAGCROSS", HS_DIAGCROSS),
                ];
                let mut x = 24;
                let y = 160;
                let (cw, ch) = (108, 70);
                for (name, style) in hatches {
                    // SetBkColor paints the gaps between hatch strokes —
                    // here a warm paper tone. (SetBkMode(OPAQUE), which is
                    // the default, is what makes this take effect.)
                    _ = SetBkColor(hdc, rgb(0xF4, 0xEF, 0xE2));
                    let brush = CreateHatchBrush(style, rgb(0x20, 0x40, 0x80));
                    let old = SelectObject(hdc, brush as HGDIOBJ);
                    _ = Rectangle(hdc, x, y, x + cw, y + ch);
                    SelectObject(hdc, old);
                    _ = DeleteObject(brush as HGDIOBJ);

                    let wide = to_wide(name);
                    _ = TextOutW(hdc, x + 6, y + ch + 4, PCWSTR::from_raw(wide.as_ptr()), -1);
                    x += cw + 14;
                }

                // ---- Row 3: system brushes (borrowed!) ----------------------
                // GetSysColorBrush returns theme-owned brushes keyed by
                // COLOR_* indexes — exactly what Windows itself paints
                // buttons, faces, and windows with. NO DeleteObject for
                // these; they are shared for the whole session.
                let systems: [(&str, i32); 5] = [
                    ("COLOR_BTNFACE", COLOR_BTNFACE),
                    ("COLOR_WINDOW", COLOR_WINDOW),
                    ("COLOR_WINDOWFRAME", COLOR_WINDOWFRAME),
                    ("COLOR_ACTIVECAPTION", COLOR_ACTIVECAPTION),
                    ("COLOR_HIGHLIGHT", COLOR_HIGHLIGHT),
                ];
                let mut x = 24;
                let y = 300;
                let (cw, ch) = (130, 44);
                for (name, index) in systems {
                    let brush = GetSysColorBrush(index);
                    _ = FillRect(hdc, &RECT { left: x, top: y, right: x + cw, bottom: y + ch }, brush);
                    // (FillRect needs no SelectObject — takes the brush directly.)

                    let wide = to_wide(name);
                    _ = TextOutW(hdc, x + 8, y + ch + 4, PCWSTR::from_raw(wide.as_ptr()), -1);
                    x += cw + 16;
                }

                SelectObject(hdc, old_pen);
                _ = DeleteObject(pen as HGDIOBJ);
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
