//! e24 — Shape Gallery: every classic GDI primitive, with labels
//!
//! What you learn:
//! * The filled/outlined shape family: `Rectangle`, `Ellipse`,
//!   `RoundRect`, `Polygon` — each fills with the DC's current brush and
//!   outlines with the current pen (the pen+brush interplay is the whole
//!   trick: PS_NULL pen = fill only, NULL_BRUSH = outline only)
//! * The arc family: `Arc` (open curve), `Chord` (arc + closing line,
//!   filled), `Pie` (arc + two radii — pie slice, filled). All three
//!   share the same confusing "bounding box + two ray endpoints"
//!   parameterization — mapped out below with helper lines so you can
//!   finally see which parameter does what
//! * `Polygon` takes a POINT array — any closed poly you like
//!
//! New Win32 APIs: `Rectangle`, `Ellipse`, `RoundRect`, `Pie`, `Chord`,
//! `Arc`, `Polygon` (all wingdi).
//!
//! Run: `cargo run -p e24_shapes`

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
            lpszClassName: w!("e24_class"),
            style: (CS_HREDRAW | CS_VREDRAW) as u32,
            ..Default::default()
        };
        debug_assert!(RegisterClassW(&wc) != 0);

        let hwnd = CreateWindowExW(
            0,
            w!("e24_class"),
            w!("e24 · Shape Gallery"),
            (WS_OVERLAPPEDWINDOW | WS_VISIBLE) as u32,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            860,
            480,
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

                _ = SetBkMode(hdc, TRANSPARENT);
                _ = SetTextColor(hdc, rgb(0x40, 0x40, 0x40));

                // One pen + one brush, selected once for the whole gallery.
                let pen = CreatePen(PS_SOLID, 2, rgb(0x20, 0x50, 0xA0));
                let old_pen = SelectObject(hdc, pen as HGDIOBJ);
                let brush = CreateSolidBrush(rgb(0xF0, 0xE0, 0xB0));
                let old_brush = SelectObject(hdc, brush as HGDIOBJ);

                // Cell layout: 4 columns × 2 rows of demos.
                let (cw, ch) = (200, 190);
                for (col, row) in [(0i32, 0i32), (1, 0), (2, 0), (3, 0), (0, 1), (1, 1), (2, 1)] {
                    let x0 = 16 + col * cw;
                    let y0 = 16 + row * ch;
                    let label = |text: &str, y_off: i32| {
                        let wide = to_wide(text);
                        _ = TextOutW(hdc, x0, y0 + y_off, PCWSTR::from_raw(wide.as_ptr()), -1);
                    };

                    match (col, row) {
                        // ---- Filled rectangle: brush inside, pen on the edge
                        (0, 0) => {
                            _ = Rectangle(hdc, x0, y0 + 30, x0 + 150, y0 + 130);
                            label("Rectangle — brush fill + pen outline", 8);
                        }
                        // ---- Ellipse: inscribed in the same bounding box
                        (1, 0) => {
                            _ = Ellipse(hdc, x0, y0 + 30, x0 + 150, y0 + 130);
                            label("Ellipse — inscribed in bounding box", 8);
                        }
                        // ---- RoundRect: the corner ellipse is w×h
                        (2, 0) => {
                            _ = RoundRect(hdc, x0, y0 + 30, x0 + 150, y0 + 130, 40, 40);
                            label("RoundRect — corner radius 40×40", 8);
                        }
                        // ---- Polygon: array of POINTs, auto-closed
                        (3, 0) => {
                            let cx = x0 + 75;
                            let cy = y0 + 80;
                            let r = 50;
                            // a 5-pointed star: outer/inner radii alternate
                            let mut pts = [POINT::default(); 10];
                            for (i, pt) in pts.iter_mut().enumerate() {
                                let angle = std::f64::consts::FRAC_PI_2
                                    + i as f64 * std::f64::consts::PI / 5.0;
                                let radius = if i % 2 == 0 { r } else { r * 2 / 5 };
                                pt.x = cx + (radius as f64 * angle.cos()) as i32;
                                pt.y = cy - (radius as f64 * angle.sin()) as i32;
                            }
                            _ = Polygon(hdc, pts.as_ptr(), pts.len() as i32);
                            label("Polygon — 10 points, auto-closed", 8);
                        }
                        // ---- The arc family: one shared bounding box --------
                        // (x1,y1,x2,y2) bound an ellipse. (x3,y3) and (x4,y4)
                        // are points on rays FROM THE CENTER: the arc runs
                        // counterclockwise from where ray 3 crosses the
                        // ellipse to where ray 4 does. The thin helper lines
                        // below draw those rays so the parameterization is
                        // visible instead of mysterious.
                        (0, 1) | (1, 1) | (2, 1) => {
                            let (left, top, right, bottom) = (x0, y0 + 34, x0 + 150, y0 + 144);
                            let (cx, cy) = ((left + right) / 2, (top + bottom) / 2);
                            // ray endpoints, well outside the ellipse
                            let (p3, p4) = ((x0 + 150, y0 + 34), (x0 + 20, y0 + 174));

                            // faint rays (NULL_BRUSH so they don't fill anything)
                            SelectObject(hdc, GetStockObject(NULL_BRUSH));
                            let helper = CreatePen(PS_DOT, 1, rgb(0xA0, 0xA0, 0xA0));
                            let old = SelectObject(hdc, helper as HGDIOBJ);
                            _ = MoveToEx(hdc, cx, cy, None);
                            _ = LineTo(hdc, p3.0, p3.1);
                            _ = MoveToEx(hdc, cx, cy, None);
                            _ = LineTo(hdc, p4.0, p4.1);
                            _ = Ellipse(hdc, left, top, right, bottom); // the invisible guide
                            SelectObject(hdc, old);
                            _ = DeleteObject(helper as HGDIOBJ);
                            SelectObject(hdc, brush as HGDIOBJ);

                            match col {
                                // Arc: just the curve — always drawn with the
                                // pen; the brush is ignored (nothing to fill).
                                0 => {
                                    _ = Arc(hdc, left, top, right, bottom, p3.0, p3.1, p4.0, p4.1);
                                    label("Arc — open curve (pen only)", 8);
                                }
                                // Chord: arc closed by a straight line, filled.
                                1 => {
                                    _ = Chord(
                                        hdc, left, top, right, bottom, p3.0, p3.1, p4.0, p4.1,
                                    );
                                    label("Chord — arc + closing chord, filled", 8);
                                }
                                // Pie: arc closed through the CENTER, filled —
                                // the classic Pac-Man slice.
                                _ => {
                                    _ = Pie(hdc, left, top, right, bottom, p3.0, p3.1, p4.0, p4.1);
                                    label("Pie — arc + two radii, filled", 8);
                                }
                            }
                        }
                        // ---- Pen/brush interplay cheat sheet ------------------
                        _ => {
                            // Same rectangle three ways:
                            // outline only (NULL_BRUSH as brush),
                            // fill only (NULL_PEN as pen),
                            // both (our pen + our brush).
                            SelectObject(hdc, GetStockObject(NULL_BRUSH));
                            _ = Rectangle(hdc, x0, y0 + 34, x0 + 44, y0 + 118);
                            SelectObject(hdc, GetStockObject(NULL_PEN));
                            SelectObject(hdc, brush as HGDIOBJ);
                            _ = Rectangle(hdc, x0 + 54, y0 + 34, x0 + 98, y0 + 118);
                            SelectObject(hdc, pen as HGDIOBJ);
                            _ = Rectangle(hdc, x0 + 108, y0 + 34, x0 + 152, y0 + 118);
                            label("outline only · fill only · both", 8);
                            let wide = to_wide("(NULL_BRUSH · NULL_PEN · both ours)");
                            _ = TextOutW(hdc, x0, y0 + 150, PCWSTR::from_raw(wide.as_ptr()), -1);
                        }
                    }
                }

                // Restore and free — same etiquette as always.
                SelectObject(hdc, old_pen);
                SelectObject(hdc, old_brush);
                _ = DeleteObject(pen as HGDIOBJ);
                _ = DeleteObject(brush as HGDIOBJ);

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
