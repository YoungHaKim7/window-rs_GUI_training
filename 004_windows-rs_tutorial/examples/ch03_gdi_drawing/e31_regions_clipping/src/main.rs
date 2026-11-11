//! e31 — Regions & Clipping: shaping where GDI may draw
//!
//! What you learn:
//! * Regions (`HRGN`) — geometric descriptions of "this part of a
//!   surface": `CreateRectRgn`, `CreateEllipticRgn`, `CreateRoundRectRgn`
//! * `CombineRgn(dst, a, b, mode)` — set operations on shapes; the four
//!   interesting modes RGN_AND (intersection), RGN_OR (union),
//!   RGN_DIFF (a minus b) and RGN_XOR (union minus intersection),
//!   demonstrated here in four quadrants, each fed the *same* rectangle
//!   and ellipse so only the mode differs
//! * `SelectClipRgn` — install a region as the DC's clip: everything
//!   drawn afterwards lands only inside it (pass `None` to clear)
//! * Regions are GDI objects: `DeleteObject` them when done
//!
//! The faint outlines show the two source shapes; the dense diagonal hatch
//! is drawn through the combined clip — so what you see hatched IS the
//! set-operation result. The bottom band is clipped to a
//! `CreateRoundRectRgn` shape.
//!
//! New Win32 APIs: `CreateRectRgn`, `CreateEllipticRgn`,
//! `CreateRoundRectRgn`, `CombineRgn`, `SelectClipRgn` (wingdi).
//!
//! Run: `cargo run -p e31_regions_clipping`

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
            lpszClassName: w!("e31_class"),
            style: (CS_HREDRAW | CS_VREDRAW) as u32,
            ..Default::default()
        };
        debug_assert!(RegisterClassW(&wc) != 0);

        let hwnd = CreateWindowExW(
            0,
            w!("e31_class"),
            w!("e31 · Regions — CombineRgn set operations as clip shapes"),
            (WS_OVERLAPPEDWINDOW | WS_VISIBLE) as u32,
            CW_USEDEFAULT, CW_USEDEFAULT, 720, 560,
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

/// Draw a dense diagonal hatch across `cell`, then label it. The hatch is
/// clipped by `clip`, so the visible hatched area shows the region's shape.
unsafe fn hatch_through(
    hdc: HDC,
    cell: &RECT,
    clip: HRGN,
    mode_name: &str,
) {
    unsafe {
        // Install the region as the DC's clip. From here until the None
        // call below, every drawing primitive is trimmed to the region.
        _ = SelectClipRgn(hdc, Some(clip));

        let pen = CreatePen(PS_SOLID, 1, rgb(0x20, 0x70, 0xC0));
        let old_pen = SelectObject(hdc, pen as HGDIOBJ);
        let span = (cell.right - cell.left + cell.bottom - cell.top) as usize;
        let step = 8; // px between hatch lines — "dense"
        let mut x = -(cell.bottom - cell.top);
        while x < (cell.right - cell.left) + span as i32 {
            _ = MoveToEx(hdc, cell.left + x, cell.bottom, None);
            _ = LineTo(hdc, cell.left + x + (cell.bottom - cell.top), cell.top);
            x += step;
        }
        SelectObject(hdc, old_pen);
        _ = DeleteObject(pen as HGDIOBJ);

        // Clear the clip (None = "no region restriction") before drawing
        // the label — otherwise the label itself would get trimmed.
        _ = SelectClipRgn(hdc, None);

        _ = SetTextColor(hdc, rgb(0x30, 0x30, 0x30));
        let wide = to_wide(mode_name);
        _ = TextOutW(hdc, cell.left + 8, cell.top + 8, PCWSTR::from_raw(wide.as_ptr()), -1);
    }
}

extern "system" fn wndproc(hwnd: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe {
        match message as i32 {
            WM_PAINT => {
                let mut ps = PAINTSTRUCT::default();
                let hdc = BeginPaint(hwnd, &mut ps);
                let mut rc = RECT::default();
                _ = GetClientRect(hwnd, &mut rc);

                _ = SetBkMode(hdc, TRANSPARENT);

                // Reserve the bottom band for the round-rect demo.
                let band_h = 90;
                let grid = RECT { bottom: rc.bottom - band_h, ..rc };

                // Faint outlines of the two source shapes, drawn once per
                // quadrant so the eye can compare source vs result.
                let outline = CreatePen(PS_DOT, 1, rgb(0xB0, 0xB0, 0xB0));
                let old_outline = SelectObject(hdc, outline as HGDIOBJ);

                // The four modes, one per quadrant. In each cell: a rect
                // covering the left 3/4 and an ellipse covering the right
                // 3/4 — they overlap in the middle, which is where the
                // four set operations visibly disagree.
                let modes: [(&str, i32); 4] = [
                    ("RGN_AND — intersection", RGN_AND),
                    ("RGN_OR — union", RGN_OR),
                    ("RGN_DIFF — rect minus ellipse", RGN_DIFF),
                    ("RGN_XOR — union minus intersection", RGN_XOR),
                ];
                let half_w = (grid.right - grid.left) / 2;
                let half_h = (grid.bottom - grid.top) / 2;
                for (i, (name, mode)) in modes.iter().enumerate() {
                    let cell = RECT {
                        left: grid.left + (i as i32 % 2) * half_w,
                        top: grid.top + (i as i32 / 2) * half_h,
                        right: grid.left + (i as i32 % 2 + 1) * half_w,
                        bottom: grid.top + (i as i32 / 2 + 1) * half_h,
                    };
                    // Inset so the quadrants have breathing room.
                    let cell = RECT { left: cell.left + 12, top: cell.top + 12, right: cell.right - 12, bottom: cell.bottom - 32 };

                    // Source shape A: rectangle over the left 3/4 of the cell.
                    let rect = CreateRectRgn(
                        cell.left,
                        cell.top,
                        cell.left + 3 * (cell.right - cell.left) / 4,
                        cell.bottom,
                    );
                    // Source shape B: ellipse over the right 3/4 (overlap!).
                    let ellipse = CreateEllipticRgn(
                        cell.left + (cell.right - cell.left) / 4,
                        cell.top,
                        cell.right,
                        cell.bottom,
                    );

                    // Outline both sources faintly (clipping is off here).
                    SelectObject(hdc, outline as HGDIOBJ);
                    _ = MoveToEx(hdc, cell.left, cell.top, None);
                    _ = LineTo(hdc, cell.left + 3 * (cell.right - cell.left) / 4, cell.top);
                    _ = LineTo(hdc, cell.left + 3 * (cell.right - cell.left) / 4, cell.bottom);
                    _ = LineTo(hdc, cell.left, cell.bottom);
                    _ = LineTo(hdc, cell.left, cell.top);
                    _ = Ellipse(
                        hdc,
                        cell.left + (cell.right - cell.left) / 4,
                        cell.top,
                        cell.right,
                        cell.bottom,
                    );

                    // CombineRgn writes into dst (a pre-made region) and
                    // returns the result's complexity class (SIMPLEREGION,
                    // COMPLEXREGION, ...) — 0 (RGN_ERROR) means failure.
                    let combined = CreateRectRgn(0, 0, 0, 0); // empty dst
                    let result = CombineRgn(Some(combined), Some(rect), Some(ellipse), *mode);
                    debug_assert!(result != RGN_ERROR);

                    hatch_through(hdc, &cell, combined, name);

                    // Regions are GDI objects — all three must be freed.
                    _ = DeleteObject(rect as HGDIOBJ);
                    _ = DeleteObject(ellipse as HGDIOBJ);
                    _ = DeleteObject(combined as HGDIOBJ);
                }
                SelectObject(hdc, old_outline);
                _ = DeleteObject(outline as HGDIOBJ);

                // ---- Bottom band: a rounded-rectangle clip ---------------
                let band = RECT { top: grid.bottom + 10, bottom: rc.bottom - 10, ..rc };
                let band = RECT { left: band.left + 12, right: band.right - 12, ..band };
                // Corner "radius" w=h=36: the ellipse the corners follow.
                let round = CreateRoundRectRgn(
                    band.left, band.top, band.right, band.bottom, 36, 36,
                );
                hatch_through(hdc, &band, round, "CreateRoundRectRgn — rounded clip");
                _ = DeleteObject(round as HGDIOBJ);

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
