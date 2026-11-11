//! e07 — Fonts & Text: typefaces, colors, and angles
//!
//! What you learn:
//! * `CreateFontW` — requesting a font (family, size, weight, italic...)
//!   and what "height 0 / width 0" mean (let GDI pick aspect ratio)
//! * `SelectObject` — the GDI idiom: tools (pen/brush/font) are selected
//!   into a device context; *restore the old one* when done
//! * `SetTextColor` / `SetBkMode(TRANSPARENT)` — colored text that doesn't
//!   stamp an opaque box behind the glyphs
//! * Text rotation via `lfEscapement` (tenths of a degree)
//! * GDI objects leak! Every CreateFontW needs a DeleteObject.
//!
//! New Win32 APIs:
//! * `CreateFontW`, `SelectObject`, `DeleteObject` (wingdi)
//! * `SetTextColor`, `SetBkMode`, `TextOutW` (wingdi)
//!
//! Run: `cargo run -p e07_fonts_and_text`

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
            lpszClassName: w!("e07_class"),
            style: (CS_HREDRAW | CS_VREDRAW) as u32,
            ..Default::default()
        };
        debug_assert!(RegisterClassW(&wc) != 0);

        let hwnd = CreateWindowExW(
            0,
            w!("e07_class"),
            w!("e07 · Fonts & Text"),
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

/// COLORREF layout is 0x00_BB_GG_RR (blue high, red low) — the reverse of
/// the more intuitive RGB. windows-rs has no RGB() macro, so:
fn rgb(red: u8, green: u8, blue: u8) -> COLORREF {
    (red as u32) | ((green as u32) << 8) | ((blue as u32) << 16)
}

extern "system" fn wndproc(hwnd: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe {
        match message as i32 {
            WM_PAINT => {
                let mut ps = PAINTSTRUCT::default();
                let hdc = BeginPaint(hwnd, &mut ps);

                // ---- A big title in Segoe UI Semibold ----------------------
                let title_font = CreateFontW(
                    48, // height in logical px (0 = "size from width")
                    0,  // width: 0 = keep the font's natural aspect ratio
                    0,  // escapement: rotation of the text line (tenths of deg)
                    0,  // orientation: rotation of individual glyphs
                    FW_SEMIBOLD, // weight: 400 normal, 700 bold, 100..900
                    0,  // italic
                    0,  // underline
                    0,  // strikeout
                    DEFAULT_CHARSET as u32,
                    OUT_DEFAULT_PRECIS as u32,
                    CLIP_DEFAULT_PRECIS as u32,
                    CLEARTYPE_QUALITY as u32,
                    (DEFAULT_PITCH | FF_DONTCARE) as u32,
                    w!("Segoe UI"), // the requested family (best effort)
                );

                // SelectObject swaps a tool into the DC and returns the old
                // one — keep it! Restoring it later is the GDI etiquette.
                let old_font = SelectObject(hdc, title_font as HGDIOBJ);

                // TRANSPARENT background mode = don't paint the box behind
                // glyphs (OPAQUE would stamp the current background color).
                _ = SetBkMode(hdc, TRANSPARENT);

                _ = SetTextColor(hdc, rgb(0x1E, 0x3A, 0x8D));
                _ = TextOutW(hdc, 40, 30, w!("Segoe UI · 48px semibold"), -1);

                // ---- Italic red subtitle, same technique, new font --------
                let sub_font = CreateFontW(
                    26, 0, 0, 0, FW_NORMAL, 1, // italic = TRUE
                    0, 0, DEFAULT_CHARSET as u32, OUT_DEFAULT_PRECIS as u32,
                    CLIP_DEFAULT_PRECIS as u32, CLEARTYPE_QUALITY as u32,
                    (DEFAULT_PITCH | FF_DONTCARE) as u32, w!("Georgia"),
                );
                SelectObject(hdc, sub_font as HGDIOBJ);
                _ = SetTextColor(hdc, rgb(0xB0, 0x30, 0x30));
                _ = TextOutW(hdc, 40, 90, w!("Georgia · italic 26px"), -1);

                // ---- Rotated text: escapement 450 = 45 degrees -------------
                let rotated = CreateFontW(
                    24, 0,
                    450, // rotate the whole line by 45° (tenths of a degree)
                    450, // rotate glyphs to match
                    FW_NORMAL, 0, 0, 0, DEFAULT_CHARSET as u32,
                    OUT_DEFAULT_PRECIS as u32, CLIP_DEFAULT_PRECIS as u32,
                    CLEARTYPE_QUALITY as u32, (DEFAULT_PITCH | FF_DONTCARE) as u32,
                    w!("Segoe UI"),
                );
                SelectObject(hdc, rotated as HGDIOBJ);
                _ = SetTextColor(hdc, rgb(0x20, 0x80, 0x20));
                _ = TextOutW(hdc, 60, 260, w!("Rotated 45 degrees — lfEscapement"), -1);

                // ---- Teardown: restore, then delete what we created -------
                SelectObject(hdc, old_font); // restore first...
                _ = DeleteObject(title_font as HGDIOBJ); // ...then free our objects
                _ = DeleteObject(sub_font as HGDIOBJ);
                _ = DeleteObject(rotated as HGDIOBJ);

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
