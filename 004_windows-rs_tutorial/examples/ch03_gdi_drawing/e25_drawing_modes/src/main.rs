//! e25 — Drawing Modes: R2_NOT and the self-erasing XOR crosshair
//!
//! What you learn:
//! * `SetROP2` — raster operation codes decide how new pixels combine
//!   with what's already on screen. `R2_COPYPEN` (default) overwrites;
//!   `R2_NOT` inverts whatever is there — and inverting twice restores
//!   the original pixel *exactly*
//! * The classic trick that follows from it: draw the same figure twice
//!   with R2_NOT and it erases itself — no redraw of the background, no
//!   `InvalidateRect`, no WM_PAINT round-trip
//! * `GetDC` / `ReleaseDC` — drawing *outside* WM_PAINT when you need the
//!   screen NOW (feedback drawing can't wait for the paint cycle)
//! * Why this contrasts with e13: there, every mouse move invalidated and
//!   repainted the whole model. Here the move draws/erases directly —
//!   far fewer bits pushed, at a price (see below)
//!
//! Why XOR is legacy (and why you still learn it): you can't predict the
//! on-screen colors (inverting a blue pixel gives orange), it fights
//! antialiasing and alpha, and it only works against a static backdrop.
//! But the *concept* — instant visual feedback without repainting the
//! world — is eternal; modern code does it with layered windows,
//! DirectComposition, or (as in e26/e27) double buffering.
//!
//! New Win32 APIs: `SetROP2`, `GetDC`, `ReleaseDC`.
//!
//! Run: `cargo run -p e25_drawing_modes`

use windows::{Win32::*, core::*};

/// The crosshair only needs to remember where it currently is, so the
/// next move can erase it there. (None = not currently on screen.)
#[derive(Default)]
struct State {
    last: Option<(i32, i32)>,
}

fn main() {
    unsafe {
        let instance = GetModuleHandleW(PCWSTR::null());
        debug_assert!(!instance.is_null());

        let wc = WNDCLASSW {
            lpfnWndProc: Some(wndproc),
            hInstance: instance,
            hCursor: LoadCursorW(None, PCWSTR(IDC_ARROW.0.cast())),
            hbrBackground: ((COLOR_WINDOW + 1) as isize) as HBRUSH,
            lpszClassName: w!("e25_class"),
            style: (CS_HREDRAW | CS_VREDRAW) as u32,
            ..Default::default()
        };
        debug_assert!(RegisterClassW(&wc) != 0);

        let hwnd = CreateWindowExW(
            0,
            w!("e25_class"),
            w!("e25 · XOR Crosshair (move the mouse)"),
            (WS_OVERLAPPEDWINDOW | WS_VISIBLE) as u32,
            CW_USEDEFAULT, CW_USEDEFAULT, 640, 400,
            None, None, Some(instance), None,
        );
        debug_assert!(!hwnd.is_null());

        let state = Box::into_raw(Box::<State>::default());
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, state as isize);

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

fn mouse_pos(lparam: LPARAM) -> (i32, i32) {
    let x = (lparam & 0xffff) as i16 as i32;
    let y = ((lparam >> 16) & 0xffff) as i16 as i32;
    (x, y)
}

/// Draw (or, if the same lines are drawn again, UN-draw) the crosshair
/// through a point. Must be called with a DC already set to R2_NOT.
unsafe fn draw_crosshair(hdc: HDC, at: (i32, i32), size: (i32, i32)) {
    unsafe {
        _ = MoveToEx(hdc, 0, at.1, None);
        _ = LineTo(hdc, size.0, at.1); // full-width horizontal
        _ = MoveToEx(hdc, at.0, 0, None);
        _ = LineTo(hdc, at.0, size.1); // full-height vertical
    }
}

extern "system" fn wndproc(hwnd: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe {
        let Some(state) = state_of(hwnd) else {
            return DefWindowProcW(hwnd, message, wparam, lparam);
        };

        match message as i32 {
            WM_MOUSEMOVE => {
                let pos = mouse_pos(lparam);
                if state.last != Some(pos) {
                    // GetDC: we need to touch the screen right now, not at
                    // the next WM_PAINT. WM_PAINT's BeginPaint is merely
                    // the *borrowed* DC for invalid regions; GetDC gives
                    // the whole client area on demand. Always ReleaseDC.
                    let hdc = GetDC(Some(hwnd));
                    let mut rc = RECT::default();
                    _ = GetClientRect(hwnd, &mut rc);
                    let size = (rc.right, rc.bottom);

                    // R2_NOT: each pixel drawn becomes NOT(itself). Drawing
                    // the identical geometry a second time flips every
                    // pixel back — a perfect eraser, whatever the backdrop.
                    let old_rop = SetROP2(hdc, R2_NOT);
                    if let Some(old) = state.last {
                        draw_crosshair(hdc, old, size); // un-draw previous
                    }
                    draw_crosshair(hdc, pos, size); // draw at the new spot
                    SetROP2(hdc, old_rop); // restore (it returned the old mode)

                    _ = ReleaseDC(Some(hwnd), hdc);
                    state.last = Some(pos);

                    // NOTE: no InvalidateRect anywhere in this handler!
                    // Compare with e13, which repainted its whole model on
                    // every WM_MOUSEMOVE. Both are valid designs: e13 has a
                    // persistent document to keep consistent; XOR feedback
                    // is for transient UI on top of a static background.
                }
                0
            }
            WM_PAINT => {
                // A fresh WM_PAINT means the backdrop was redrawn — the
                // crosshair pixels are gone, so forget the old position or
                // the next erase pass would corrupt the new backdrop.
                state.last = None;

                let mut ps = PAINTSTRUCT::default();
                let hdc = BeginPaint(hwnd, &mut ps);
                let mut rc = RECT::default();
                _ = GetClientRect(hwnd, &mut rc);

                // A busy backdrop proves XOR preserves what's underneath:
                // stare at where the crosshair passed and see it intact.
                let pen = CreatePen(PS_SOLID, 1, rgb(0xC8, 0xD8, 0xC8));
                let old_pen = SelectObject(hdc, pen as HGDIOBJ);
                for x in (0..rc.right).step_by(24) {
                    _ = MoveToEx(hdc, x, 0, None);
                    _ = LineTo(hdc, x, rc.bottom);
                }
                for y in (0..rc.bottom).step_by(24) {
                    _ = MoveToEx(hdc, 0, y, None);
                    _ = LineTo(hdc, rc.right, y);
                }
                SelectObject(hdc, old_pen);
                _ = DeleteObject(pen as HGDIOBJ);

                _ = SetBkMode(hdc, TRANSPARENT);
                _ = SetTextColor(hdc, rgb(0x40, 0x40, 0x40));
                _ = TextOutW(hdc, 12, 10, w!("R2_NOT crosshair — move the mouse; the grid heals itself"), -1);

                _ = EndPaint(hwnd, &ps);
                0
            }
            WM_DESTROY => {
                let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut State;
                if !ptr.is_null() {
                    SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
                    drop(Box::from_raw(ptr));
                }
                PostQuitMessage(0);
                0
            }
            _ => DefWindowProcW(hwnd, message, wparam, lparam),
        }
    }
}

fn rgb(red: u8, green: u8, blue: u8) -> COLORREF {
    (red as u32) | ((green as u32) << 8) | ((blue as u32) << 16)
}
