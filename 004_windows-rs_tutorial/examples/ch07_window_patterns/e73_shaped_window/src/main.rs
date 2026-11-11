//! e73 — Shaped Windows: SetWindowRgn cuts the window itself
//!
//! What you learn:
//! * `SetWindowRgn` — clips a window to a region and gives the cut to the
//!   WINDOW MANAGER, not just to painting: hit-testing, shadow, resize
//!   edges, everything outside the region stops existing. The mouse
//!   literally clicks through the missing parts.
//! * `CreateEllipticRgn` and `CreatePolygonRgn` — two of the region
//!   constructors (RECT- and roundrect-based siblings exist too); a
//!   5-point star via 10 alternating vertices is the classic polygon demo.
//! * The ownership rule that bites everyone: once `SetWindowRgn` SUCCEEDS,
//!   the system owns the region — never `DeleteObject` it. On failure you
//!   still own it. Passing `None` as the region resets to the full
//!   rectangle and the old region is freed by Windows.
//! * The fundamental limitation, stated plainly: a region is a **binary
//!   mask** — a pixel is in or out, there is no antialiasing and no
//!   partial transparency. Curved cuts show staircase edges on modern
//!   high-DPI screens. Today you would reach for the layered window
//!   instead (e72's LWA_COLORKEY for the same trick with the same jagged
//!   caveat, or `UpdateLayeredWindow` for true per-pixel alpha with
//!   smooth edges).
//! * Regions are in WINDOW coordinates (origin = top-left of the whole
//!   window, non-client frame included) — so WM_SIZE re-applies them,
//!   which is also the natural hook for a shape that tracks the size.
//!
//! Press 1 (ellipse), 2 (star), 3 (back to rectangle). Resize in star
//! mode: the star re-fits the window.
//!
//! New Win32 APIs: `SetWindowRgn`, `CreateEllipticRgn`,
//! `CreatePolygonRgn`, `ALTERNATE`/`WINDING` fill modes.
//!
//! Run: `cargo run -p e73_shaped_window`

use windows::{Win32::*, core::*};

#[derive(Clone, Copy, PartialEq)]
enum Shape {
    Rect,
    Ellipse,
    Star,
}

#[derive(Default)]
struct State {
    shape: Option<Shape>, // None until first WM_SIZE — region applied lazily
}

fn main() {
    unsafe {
        let instance = GetModuleHandleW(PCWSTR::null());
        debug_assert!(!instance.is_null());

        let wc = WNDCLASSW {
            lpfnWndProc: Some(wndproc),
            hInstance: instance,
            hCursor: LoadCursorW(None, PCWSTR(IDC_ARROW.0.cast())),
            // The class brush paints whatever the region leaves visible —
            // a color per shape below makes the cut obvious.
            hbrBackground: ((COLOR_WINDOW + 1) as isize) as HBRUSH,
            lpszClassName: w!("e73_class"),
            style: (CS_HREDRAW | CS_VREDRAW) as u32,
            ..Default::default()
        };
        debug_assert!(RegisterClassW(&wc) != 0);

        let state = Box::into_raw(Box::<State>::default());

        let hwnd = CreateWindowExW(
            0,
            w!("e73_class"),
            w!("e73 · Shaped Window — press 1 / 2 / 3"),
            (WS_OVERLAPPEDWINDOW | WS_VISIBLE) as u32,
            CW_USEDEFAULT, CW_USEDEFAULT, 420, 420,
            None, None, Some(instance),
            Some(state as LPVOID),
        );
        debug_assert!(!hwnd.is_null());
        if hwnd.is_null() {
            drop(Box::from_raw(state));
        }

        let mut message = MSG::default();
        while GetMessageW(&mut message, None, 0, 0).0 > 0 {
            _ = TranslateMessage(&message);
            DispatchMessageW(&message);
        }
    }
}

unsafe fn state_of(hwnd: HWND) -> Option<&'static mut State> {
    unsafe {
        let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut State;
        (!ptr.is_null()).then(|| &mut *ptr)
    }
}

fn rgb(red: u8, green: u8, blue: u8) -> COLORREF {
    (red as u32) | ((green as u32) << 8) | ((blue as u32) << 16)
}

fn to_wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Ten points of a 5-pointed star: outer vertex, inner vertex, outer...
/// rotated around the center; start pointing up (-90°).
fn star_points(cx: i32, cy: i32, outer: i32, inner: i32) -> Vec<POINT> {
    let mut pts = Vec::with_capacity(10);
    for i in 0..10 {
        let radius = if i % 2 == 0 { outer } else { inner };
        let angle = -std::f64::consts::FRAC_PI_2 + std::f64::consts::TAU * (i as f64) / 10.0;
        pts.push(POINT {
            x: cx + (angle.cos() * radius as f64).round() as i32,
            y: cy + (angle.sin() * radius as f64).round() as i32,
        });
    }
    pts
}

/// (Re)apply the window region for the current shape. Called from WM_SIZE
/// and after each keypress — one place, one ownership story.
unsafe fn apply_shape(hwnd: HWND) {
    unsafe {
        let Some(state) = state_of(hwnd) else { return };

        // Regions live in WINDOW coordinates: measure the whole window,
        // then use size-relative values with origin (0,0) at its corner.
        let mut wr = RECT::default();
        _ = GetWindowRect(hwnd, &mut wr);
        let (w, h) = (wr.right - wr.left, wr.bottom - wr.top);

        match state.shape {
            // Passing None resets to the plain rectangle. Windows frees the
            // previous region for us.
            None | Some(Shape::Rect) => {
                _ = SetWindowRgn(hwnd, None, true);
            }
            Some(Shape::Ellipse) => {
                let rgn = CreateEllipticRgn(0, 0, w, h);
                // On SUCCESS ownership passes to the system: no
                // DeleteObject, ever. Only a FAILED call would leave the
                // region ours to clean up (returns 0).
                if SetWindowRgn(hwnd, Some(rgn), true) == 0 {
                    _ = DeleteObject(rgn as HGDIOBJ);
                }
            }
            Some(Shape::Star) => {
                let pts = star_points(w / 2, h / 2, w.min(h) / 2 - 8, w.min(h) / 5);
                // WINDING vs ALTERNATE: for a simple star outline both
                // fill the pentagon center; WINDING is the safe default
                // whenever a polygon might touch or cross itself.
                let rgn = CreatePolygonRgn(pts.as_ptr(), pts.len() as i32, WINDING);
                if SetWindowRgn(hwnd, Some(rgn), true) == 0 {
                    _ = DeleteObject(rgn as HGDIOBJ);
                }
            }
        }

        _ = InvalidateRect(Some(hwnd), None, true);
    }
}

extern "system" fn wndproc(hwnd: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe {
        match message as i32 {
            WM_NCCREATE => {
                let cs = lparam as *const CREATESTRUCTW;
                SetWindowLongPtrW(hwnd, GWLP_USERDATA, (*cs).lpCreateParams as isize);
                DefWindowProcW(hwnd, message, wparam, lparam)
            }

            WM_CREATE => {
                if let Some(state) = state_of(hwnd) {
                    state.shape = Some(Shape::Rect);
                }
                apply_shape(hwnd);
                0
            }

            WM_SIZE => {
                // The region was computed for the OLD size — re-cut for
                // the new one (most visible in star mode: resize away).
                apply_shape(hwnd);
                0
            }

            WM_KEYDOWN => {
                let key = wparam as i32;
                let new_shape = match key {
                    k if k == '1' as i32 => Some(Shape::Ellipse),
                    k if k == '2' as i32 => Some(Shape::Star),
                    k if k == '3' as i32 => Some(Shape::Rect),
                    _ => None,
                };
                match new_shape {
                    Some(shape) => {
                        if let Some(state) = state_of(hwnd) {
                            state.shape = Some(shape);
                        }
                        apply_shape(hwnd);
                        0
                    }
                    None => DefWindowProcW(hwnd, message, wparam, lparam),
                }
            }

            WM_PAINT => {
                let mut ps = PAINTSTRUCT::default();
                let hdc = BeginPaint(hwnd, &mut ps);
                let mut rc = RECT::default();
                _ = GetClientRect(hwnd, &mut rc);

                // A background that MATCHES the shape: each mode paints its
                // own color, so the silhouette reads as intentional rather
                // than "a rectangle with bits missing". Everything painted
                // outside the region is harmlessly clipped by the system.
                let Some(state) = state_of(hwnd) else {
                    _ = EndPaint(hwnd, &ps);
                    return 0;
                };
                let color = match state.shape {
                    Some(Shape::Ellipse) => rgb(0x2D, 0x6C, 0xAF), // blue disc
                    Some(Shape::Star) => rgb(0xE0, 0x9B, 0x2E),    // gold star
                    _ => rgb(0xF4, 0xF4, 0xF4),                    // plain window
                };
                let brush = CreateSolidBrush(color);
                _ = FillRect(hdc, &rc, brush);
                _ = DeleteObject(brush as HGDIOBJ);

                _ = SetBkMode(hdc, TRANSPARENT);
                _ = SetTextColor(hdc, rgb(0x20, 0x20, 0x20));
                let wide = to_wide("1 ellipse · 2 star · 3 rectangle");
                DrawTextW(
                    hdc,
                    PCWSTR::from_raw(wide.as_ptr()),
                    -1,
                    &mut rc,
                    (DT_CENTER | DT_BOTTOM | DT_SINGLELINE) as u32,
                );

                _ = EndPaint(hwnd, &ps);
                0
            }

            WM_NCDESTROY => {
                let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut State;
                if !ptr.is_null() {
                    SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
                    drop(Box::from_raw(ptr));
                }
                DefWindowProcW(hwnd, message, wparam, lparam)
            }

            WM_DESTROY => {
                PostQuitMessage(0);
                0
            }

            _ => DefWindowProcW(hwnd, message, wparam, lparam),
        }
    }
}
