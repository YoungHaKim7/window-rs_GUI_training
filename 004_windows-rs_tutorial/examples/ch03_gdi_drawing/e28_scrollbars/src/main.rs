//! e28 — Scrollbars: window scrollbars and a 2000×1400 virtual canvas
//!
//! What you learn:
//! * **Window scrollbars** (`WS_VSCROLL | WS_HSCROLL` on the window
//!   itself) — not scrollbar *controls*; they emit `WM_VSCROLL`/
//!   `WM_HSCROLL` to the window and are configured with
//!   `SetScrollInfo`/`GetScrollInfo` + the `SCROLLINFO` struct
//!   (range, page size, thumb position — the page size is what makes
//!   the thumb shrink to reflect how much of the document you see)
//! * Every `SB_*` request code: LINE/PAGE (arrows, bar clicks, all four
//!   directions), THUMBTRACK (live drag) vs THUMBPOSITION (release),
//!   TOP/BOTTOM/LEFT/RIGHT, ENDSCROLL
//! * Why THUMB codes need `GetScrollInfo(SIF_TRACKPOS)`: the 16-bit
//!   position packed in lparam can't express scroll ranges > 32767 —
//!   SCROLLINFO carries the full 32-bit value
//! * `ScrollWindowEx` — shift the *existing* pixels (cheap blit) and
//!   invalidate only the newly exposed strip, instead of repainting
//!   everything on every tick
//! * `WM_MOUSEWHEEL` — the modern way users actually scroll
//!
//! New Win32 APIs: `SetScrollInfo`, `GetScrollInfo`, `ScrollWindowEx`,
//! `UpdateWindow` (all winuser).
//!
//! Run: `cargo run -p e28_scrollbars`

use windows::{Win32::*, core::*};

/// The virtual document is far bigger than any window.
const VIRT_W: i32 = 2000;
const VIRT_H: i32 = 1400;
const CELL: i32 = 100; // grid spacing on the virtual canvas
const LINE_STEP: i32 = 20; // one arrow click scrolls this far (px)

#[derive(Default)]
struct State {
    xpos: i32, // scroll position — the canvas point at the client's left
    ypos: i32, // ...and top. Virtual (x,y) maps to client (x - xpos, y - ypos).
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
            lpszClassName: w!("e28_class"),
            style: (CS_HREDRAW | CS_VREDRAW) as u32,
            ..Default::default()
        };
        debug_assert!(RegisterClassW(&wc) != 0);

        // WS_VSCROLL | WS_HSCROLL bolt the two bars onto the window frame.
        // From now on Windows draws them, hit-tests them, and posts
        // WM_VSCROLL / WM_HSCROLL when clicked — the *meaning* of the
        // positions is entirely our business.
        let hwnd = CreateWindowExW(
            0,
            w!("e28_class"),
            w!("e28 · Scrollbars — drag a 2000×1400 canvas"),
            (WS_OVERLAPPEDWINDOW | WS_VISIBLE | WS_VSCROLL | WS_HSCROLL) as u32,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            700,
            480,
            None,
            None,
            Some(instance),
            None,
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

fn rgb(red: u8, green: u8, blue: u8) -> COLORREF {
    (red as u32) | ((green as u32) << 8) | ((blue as u32) << 16)
}

fn to_wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Push the range/page/position of one bar into Windows. nPage is the
/// magic: the thumb size = nPage / (nMax - nMin + 1) of the track, and
/// the farthest scrollable position becomes nMax - nPage + 1 (you can't
/// scroll the document past its end).
unsafe fn set_bar(hwnd: HWND, bar: i32, max: i32, page: i32, pos: i32) {
    unsafe {
        let si = SCROLLINFO {
            cbSize: std::mem::size_of::<SCROLLINFO>() as u32,
            fMask: (SIF_RANGE | SIF_PAGE | SIF_POS) as u32, // which fields to use
            nMin: 0,
            nMax: max,
            nPage: page.max(1) as u32,
            nPos: pos,
            nTrackPos: 0,
        };
        _ = SetScrollInfo(hwnd, bar, &si, true); // true = redraw the bar
    }
}

/// The shared heart of WM_VSCROLL and WM_HSCROLL: decode the request
/// code (LOWORD of wparam), compute a new position, scroll the pixels,
/// update the bar. `bar` is SB_VERT or SB_HORZ.
unsafe fn on_scroll(hwnd: HWND, bar: i32, code: i32) {
    unsafe {
        // SIF_ALL = range + page + pos + trackpos in one query.
        let mut si = SCROLLINFO {
            cbSize: std::mem::size_of::<SCROLLINFO>() as u32,
            fMask: SIF_ALL as u32,
            ..Default::default()
        };
        _ = GetScrollInfo(hwnd, bar, &mut si);

        let old = si.nPos;
        let max_pos = (si.nMax - si.nPage as i32 + 1).max(si.nMin);
        let new = match code {
            // Vertical and horizontal codes share values (SB_TOP = SB_LEFT
            // = 6 ...), so one match covers both bars. Rust flags duplicate
            // constants in one or-pattern as unreachable, so each arm lists
            // one name; the horizontal alias rides along in a comment.
            SB_TOP /* = SB_LEFT, Home / drag to top */ => si.nMin,
            SB_BOTTOM /* = SB_RIGHT, End / drag to bottom */ => max_pos,
            SB_LINEUP /* = SB_LINELEFT, arrow button */ => old - LINE_STEP,
            SB_LINEDOWN /* = SB_LINERIGHT */ => old + LINE_STEP,
            SB_PAGEUP /* = SB_PAGELEFT, bar click above thumb */
                => old - si.nPage as i32,
            SB_PAGEDOWN /* = SB_PAGERIGHT, bar click below thumb */
                => old + si.nPage as i32,
            // Live drag. lparam also carries a 16-bit position, but
            // documents can be longer than 32767 px — nTrackPos is the
            // trustworthy 32-bit value. (SB_THUMBPOSITION is the same
            // value delivered once, on button release.)
            SB_THUMBTRACK | SB_THUMBPOSITION => si.nTrackPos,
            // SB_ENDSCROLL fires when the user lets go — nothing to do,
            // we already scrolled everything eagerly.
            _ => old,
        }
        .clamp(si.nMin, max_pos);

        if new != old {
            // Scroll the CONTENT: shift already-drawn pixels by
            // (old - new) — sign matters, scrolling down moves content up.
            // The exposed strip is invalidated; SW_INVALIDATE|SW_ERASE make
            // it a proper WM_PAINT region. Cheaper than full repaints.
            let (dx, dy) = if bar == SB_VERT {
                (0, old - new)
            } else {
                (old - new, 0)
            };
            _ = ScrollWindowEx(
                hwnd,
                dx,
                dy,
                None,
                None,
                None,
                None,
                (SW_INVALIDATE | SW_ERASE) as u32,
            );
            // Push the new thumb position to the bar itself.
            si.fMask = SIF_POS as u32;
            si.nPos = new;
            _ = SetScrollInfo(hwnd, bar, &si, true);
            // Paint the exposed strip right now instead of waiting for the
            // queue to drain — without this the strip shows stale pixels.
            _ = UpdateWindow(hwnd);
        }
    }
}

extern "system" fn wndproc(hwnd: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe {
        let Some(state) = state_of(hwnd) else {
            return DefWindowProcW(hwnd, message, wparam, lparam);
        };

        match message as i32 {
            WM_SIZE => {
                // Client size changed → the page size changes → re-apply
                // both bars, and clamp positions to the new maximum.
                let w = (lparam & 0xffff) as i16 as i32;
                let h = ((lparam >> 16) & 0xffff) as i16 as i32;
                state.xpos = state.xpos.clamp(0, (VIRT_W - w).max(0));
                state.ypos = state.ypos.clamp(0, (VIRT_H - h).max(0));
                set_bar(hwnd, SB_HORZ, VIRT_W - 1, w, state.xpos);
                set_bar(hwnd, SB_VERT, VIRT_H - 1, h, state.ypos);
                0
            }
            WM_VSCROLL => {
                on_scroll(hwnd, SB_VERT, (wparam & 0xffff) as i32);
                state.ypos = GetScrollPos(hwnd, SB_VERT);
                0
            }
            WM_HSCROLL => {
                on_scroll(hwnd, SB_HORZ, (wparam & 0xffff) as i32);
                state.xpos = GetScrollPos(hwnd, SB_HORZ);
                0
            }
            WM_MOUSEWHEEL => {
                // The wheel talks in multiples of WHEEL_DELTA (120); many
                // mice with free-spinning wheels send fractions, so
                // accumulate the signed value. Three grid cells per notch
                // feels documenty. (Vertical only; Shift+wheel → horizontal
                // is left to you — check GetKeyState(VK_SHIFT).)
                let delta = ((wparam >> 16) & 0xffff) as u16 as i16;
                let code = if delta < 0 { SB_LINEDOWN } else { SB_LINEUP };
                let notches = delta.unsigned_abs() as usize / WHEEL_DELTA as usize;
                for _ in 0..notches.max(1) * 3 {
                    on_scroll(hwnd, SB_VERT, code);
                }
                state.ypos = GetScrollPos(hwnd, SB_VERT);
                0
            }
            WM_PAINT => {
                let mut ps = PAINTSTRUCT::default();
                let hdc = BeginPaint(hwnd, &mut ps);
                let mut rc = RECT::default();
                _ = GetClientRect(hwnd, &mut rc);

                // Everything below is drawn in CLIENT coordinates, but
                // positioned by (virtual - scroll). This one subtraction
                // IS the scrolling: no big DC transform, no offset GDI.
                let (ox, oy) = (-state.xpos, -state.ypos);

                let pen = CreatePen(PS_SOLID, 1, rgb(0xC0, 0xC8, 0xD0));
                let old_pen = SelectObject(hdc, pen as HGDIOBJ);

                // Checkerboard cells, tinted so movement is obvious.
                for gy in 0..=(VIRT_H / CELL) {
                    for gx in 0..=(VIRT_W / CELL) {
                        // Cheap visibility test: skip cells fully outside
                        // the client area (plus one cell of margin).
                        let left = gx * CELL + ox;
                        let top = gy * CELL + oy;
                        if left > rc.right || top > rc.bottom || left + CELL < 0 || top + CELL < 0 {
                            continue;
                        }
                        if (gx + gy) % 2 == 0 {
                            let brush = CreateSolidBrush(rgb(0xF0, 0xF4, 0xF8));
                            _ = FillRect(
                                hdc,
                                &RECT {
                                    left,
                                    top,
                                    right: left + CELL,
                                    bottom: top + CELL,
                                },
                                brush,
                            );
                            _ = DeleteObject(brush as HGDIOBJ);
                        }
                    }
                }
                // Grid lines.
                for gx in 0..=(VIRT_W / CELL) {
                    let x = gx * CELL + ox;
                    _ = MoveToEx(hdc, x, oy, None);
                    _ = LineTo(hdc, x, oy + VIRT_H);
                }
                for gy in 0..=(VIRT_H / CELL) {
                    let y = gy * CELL + oy;
                    _ = MoveToEx(hdc, ox, y, None);
                    _ = LineTo(hdc, ox + VIRT_W, y);
                }
                SelectObject(hdc, old_pen);
                _ = DeleteObject(pen as HGDIOBJ);

                // Cell labels — a few landmarks prove the coordinates.
                _ = SetBkMode(hdc, TRANSPARENT);
                _ = SetTextColor(hdc, rgb(0x40, 0x50, 0x60));
                for gy in 0..=(VIRT_H / CELL) {
                    for gx in 0..=(VIRT_W / CELL) {
                        let (x, y) = (gx * CELL + ox + 6, gy * CELL + oy + 6);
                        if x > -120 && y > -20 && x < rc.right && y < rc.bottom {
                            let label = format!("{},{}", gx, gy);
                            let wide = to_wide(&label);
                            _ = TextOutW(hdc, x, y, PCWSTR::from_raw(wide.as_ptr()), -1);
                        }
                    }
                }
                // A "you are here" marker out in the far corner.
                let brush = CreateSolidBrush(rgb(0xE0, 0x60, 0x30));
                _ = SelectObject(hdc, brush as HGDIOBJ);
                _ = Ellipse(hdc, 1800 + ox, 1200 + oy, 1870 + ox, 1270 + oy);
                _ = SelectObject(hdc, GetStockObject(BLACK_BRUSH));
                _ = DeleteObject(brush as HGDIOBJ);
                let wide = to_wide("corner marker @ (1800,1200)");
                _ = TextOutW(
                    hdc,
                    1740 + ox,
                    1290 + oy,
                    PCWSTR::from_raw(wide.as_ptr()),
                    -1,
                );

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
