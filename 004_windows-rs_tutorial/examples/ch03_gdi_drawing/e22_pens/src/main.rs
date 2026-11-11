//! e22 — Pens: line styles, widths, and the DeleteObject discipline
//!
//! What you learn:
//! * `CreatePen` — the two-fact pen model: **cosmetic** pens (width ≤ 1)
//!   are always exactly one physical pixel wide and support all the dash
//!   styles; **geometric** pens (width ≥ 2) draw thick lines but *only*
//!   PS_SOLID — ask for a fat dashed pen and GDI silently gives you a fat
//!   solid one (watch the "fat dash" row below lie about its style!)
//! * The full PS_* style gallery: SOLID, DASH, DOT, DASHDOT, DASHDOTDOT,
//!   NULL, INSIDEFRAME
//! * Strict GDI etiquette: create → select (keep the old!) → draw →
//!   re-select the old → `DeleteObject`. A pen still selected into a DC
//!   cannot be deleted — the handle leaks until the DC dies.
//! * `GetStockObject(NULL_PEN)` — stock objects are owned by the system
//!   and must NEVER be deleted (contrast with our created pens)
//!
//! New Win32 APIs: `CreatePen`, `GetStockObject` (wingdi).
//!
//! Run: `cargo run -p e22_pens`

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
            lpszClassName: w!("e22_class"),
            style: (CS_HREDRAW | CS_VREDRAW) as u32,
            ..Default::default()
        };
        debug_assert!(RegisterClassW(&wc) != 0);

        let hwnd = CreateWindowExW(
            0,
            w!("e22_class"),
            w!("e22 · Pen Gallery"),
            (WS_OVERLAPPEDWINDOW | WS_VISIBLE) as u32,
            CW_USEDEFAULT, CW_USEDEFAULT, 760, 560,
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

/// One gallery row: what the label says, what CreatePen gets called with.
struct Row {
    label: &'static str,
    style: i32,  // PS_* constant (i32 in this crate version)
    width: i32,
    color: COLORREF,
}

extern "system" fn wndproc(hwnd: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe {
        match message as i32 {
            WM_PAINT => {
                let mut ps = PAINTSTRUCT::default();
                let hdc = BeginPaint(hwnd, &mut ps);

                let mut rc = RECT::default();
                _ = GetClientRect(hwnd, &mut rc);
                let width = rc.right - rc.left;

                let rows = [
                    Row { label: "PS_SOLID · width 1 (cosmetic)", style: PS_SOLID, width: 1, color: rgb(0x20, 0x20, 0x20) },
                    Row { label: "PS_SOLID · width 5 (geometric)", style: PS_SOLID, width: 5, color: rgb(0xC0, 0x20, 0x20) },
                    Row { label: "PS_DASH · width 1", style: PS_DASH, width: 1, color: rgb(0x20, 0x60, 0xC0) },
                    Row { label: "PS_DOT · width 1", style: PS_DOT, width: 1, color: rgb(0x20, 0x80, 0x40) },
                    Row { label: "PS_DASHDOT · width 1", style: PS_DASHDOT, width: 1, color: rgb(0x90, 0x40, 0xA0) },
                    Row { label: "PS_DASHDOTDOT · width 1", style: PS_DASHDOTDOT, width: 1, color: rgb(0xD0, 0x80, 0x20) },
                    Row { label: "PS_DASH · width 5 — falls back to solid!", style: PS_DASH, width: 5, color: rgb(0x80, 0x80, 0x80) },
                    Row { label: "PS_INSIDEFRAME · width 9 (stroke stays inside)", style: PS_INSIDEFRAME, width: 9, color: rgb(0x20, 0x70, 0x70) },
                ];

                _ = SetBkMode(hdc, TRANSPARENT);
                let old_text_color = GetTextColor(hdc);

                let mut y = 40;
                for row in &rows {
                    // The discipline, in full, once per pen:
                    // 1. create
                    let pen = CreatePen(row.style, row.width, row.color);
                    debug_assert!(!pen.is_null());
                    // 2. select into the DC, REMEMBERING what was in there
                    let old_pen = SelectObject(hdc, pen as HGDIOBJ);
                    // 3. draw
                    _ = MoveToEx(hdc, 30, y, None);
                    _ = LineTo(hdc, width - 260, y);
                    // 4. put the old object back...
                    SelectObject(hdc, old_pen);
                    // 5. ...and only then delete ours. Deleting a pen that
                    //    is still selected into a DC fails silently.
                    _ = DeleteObject(pen as HGDIOBJ);

                    _ = SetTextColor(hdc, rgb(0x50, 0x50, 0x50));
                    let wide = to_wide(row.label);
                    _ = TextOutW(hdc, width - 245, y - 7, PCWSTR::from_raw(wide.as_ptr()), -1);
                    y += 34;
                }

                // ---- Stock objects: borrowed, never deleted ----------------
                // NULL_PEN draws nothing — handy for "fill without outline"
                // (e24 uses exactly this). GetStockObject returns an HGDIOBJ
                // directly; stock objects live as long as the process and
                // must NOT be DeleteObject'd.
                let brush1 = CreateSolidBrush(rgb(0xE8, 0xE0, 0xD0));
                let old_brush = SelectObject(hdc, brush1 as HGDIOBJ);
                SelectObject(hdc, GetStockObject(NULL_PEN));
                _ = Rectangle(hdc, 30, y + 6, 230, y + 46); // fill only, no outline
                SelectObject(hdc, GetStockObject(BLACK_PEN)); // back to default pen

                // The final row: a PS_NULL pen behaves like NULL_PEN.
                let brush2 = CreateSolidBrush(rgb(0xD0, 0xD8, 0xE8));
                SelectObject(hdc, brush2 as HGDIOBJ);
                let pen = CreatePen(PS_NULL, 0, 0);
                let old_pen = SelectObject(hdc, pen as HGDIOBJ);
                _ = Rectangle(hdc, 250, y + 6, 450, y + 46);
                SelectObject(hdc, old_pen);
                SelectObject(hdc, old_brush); // restore both defaults
                _ = DeleteObject(pen as HGDIOBJ);
                _ = DeleteObject(brush1 as HGDIOBJ);
                _ = DeleteObject(brush2 as HGDIOBJ);

                _ = SetTextColor(hdc, rgb(0x50, 0x50, 0x50));
                for (text, x) in [
                    ("NULL_PEN stock · outline skipped", 30),
                    ("PS_NULL · outline skipped", 250),
                ] {
                    let wide = to_wide(text);
                    _ = TextOutW(hdc, x, y + 52, PCWSTR::from_raw(wide.as_ptr()), -1);
                }
                _ = SetTextColor(hdc, old_text_color);

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
