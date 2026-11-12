//! e49 — Scrollbar Controls: a standalone SCROLLBAR child window
//!
//! What you learn:
//! * The two kinds of scrollbar: **window scrollbars** (from e28 — a
//!   `WS_VSCROLL` style on the window itself, part of its frame) vs the
//!   **scrollbar control** (a `SCROLLBAR`-class child window, created with
//!   `CreateWindowExW` like every control in this chapter, placed anywhere
//!   you like, addressed by control id)
//! * How to tell them apart at runtime: for a scrollbar *control*, the
//!   `WM_VSCROLL`/`WM_HSCROLL` message arrives at the PARENT with
//!   `lparam` = the scrollbar's HWND; for window scrollbars `lparam` is 0
//! * The full request loop — `SB_LINEUP`/`SB_LINEDOWN` (arrows),
//!   `SB_PAGEUP`/`SB_PAGEDOWN` (bar clicks), `SB_THUMBTRACK` (live drag)
//!   and `SB_THUMBPOSITION` (released) — with `GetScrollInfo`/
//!   `SetScrollInfo` using `SB_CTL` because the target is a control, not a
//!   window bar (`SB_VERT`/`SB_HORZ` in e28's case)
//!
//! New Win32 APIs: `SCROLLBAR` class, `SBS_VERT`, `SB_CTL`,
//! `GetScrollInfo` (SIF_TRACKPOS), `SetScrollInfo`.
//!
//! Run: `cargo run -p e49_scrollbar_controls`

use windows::{Win32::*, core::*};

/// Control id of the scrollbar (WM_COMMAND's LOWORD — scrollbars actually
/// notify via WM_VSCROLL, where lparam identifies them instead).
const IDC_SCROLL: i32 = 1001;

const RANGE_MIN: i32 = 0;
const RANGE_MAX: i32 = 100;

#[derive(Default)]
struct State {
    value: i32, // the scrollbar position, mirrored into Rust for painting
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
            lpszClassName: w!("e49_class"),
            style: (CS_HREDRAW | CS_VREDRAW) as u32,
            ..Default::default()
        };
        debug_assert!(RegisterClassW(&wc) != 0);

        let hwnd = CreateWindowExW(
            0,
            w!("e49_class"),
            w!("e49 · Scrollbar Control — drag or click the bar"),
            (WS_OVERLAPPEDWINDOW | WS_VISIBLE) as u32,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            420,
            340,
            None,
            None,
            Some(instance),
            None,
        );
        debug_assert!(!hwnd.is_null());

        let state = Box::into_raw(Box::<State>::default());
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, state as isize);

        // ---- The scrollbar control child -------------------------------------
        // Contrast with e28: there we styled the WINDOW with WS_VSCROLL and
        // the bar hugged the frame. Here the SCROLLBAR class creates an
        // ordinary child window — SBS_VERT makes it vertical (SBS_HORZ is
        // the default). It could sit mid-form between two other controls;
        // that placement freedom is the whole point of scrollbar controls.
        let bar = CreateWindowExW(
            0,
            w!("SCROLLBAR"),
            w!(""),
            (WS_CHILD | WS_VISIBLE | SBS_VERT) as u32,
            20,
            20,
            20,
            260,
            Some(hwnd),
            Some(IDC_SCROLL as usize as HMENU),
            Some(instance),
            None,
        );
        debug_assert!(!bar.is_null());

        // Same SCROLLINFO struct as e28, but note the bar type below:
        // SB_CTL addresses THIS control window; e28 used SB_VERT to talk
        // about a window's own frame scrollbar. nPage sizes the thumb: with
        // page 10 of a 0..100 range, the thumb fills 10% of the track and
        // the farthest reachable position is 100 - 10 + 1 = 91.
        let si = SCROLLINFO {
            cbSize: std::mem::size_of::<SCROLLINFO>() as u32,
            fMask: (SIF_RANGE | SIF_PAGE | SIF_POS) as u32,
            nMin: RANGE_MIN,
            nMax: RANGE_MAX,
            nPage: 10,
            nPos: 0,
            nTrackPos: 0,
        };
        _ = SetScrollInfo(bar, SB_CTL, &si, true); // true = redraw the bar

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

extern "system" fn wndproc(hwnd: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe {
        let Some(state) = state_of(hwnd) else {
            return DefWindowProcW(hwnd, message, wparam, lparam);
        };

        match message as i32 {
            // A scrollbar control reports through WM_VSCROLL (vertical) /
            // WM_HSCROLL (horizontal) — NOT WM_COMMAND like buttons and
            // lists. Two ways to know who sent it:
            //   * lparam != 0 → it is this lparam HWND: a scrollbar CONTROL.
            //   * lparam == 0 → one of OUR OWN window scrollbars (e28).
            WM_VSCROLL => {
                if lparam == 0 {
                    // Not ours (this window has no frame scrollbars), but
                    // forwarding to DefWindowProcW is still the correct move.
                    return DefWindowProcW(hwnd, message, wparam, lparam);
                }
                let bar = lparam as HWND;
                let request = (wparam & 0xffff) as u32; // LOWORD = SB_ code

                // Query the bar's current facts. SIF_ALL covers range, page,
                // position AND nTrackPos — the 32-bit live thumb position.
                // The HIWORD of wparam also carries a position for the two
                // THUMB codes, but only 16 bits of it; SIF_TRACKPOS is the
                // trustworthy value (e28 explains why ranges > 32767 break it).
                let mut si = SCROLLINFO {
                    cbSize: std::mem::size_of::<SCROLLINFO>() as u32,
                    fMask: SIF_ALL as u32,
                    ..Default::default()
                };
                _ = GetScrollInfo(bar, SB_CTL, &mut si);

                // With nPage = 10, the thumb stops one page short of nMax —
                // you can't scroll past the last visible page.
                let max_pos = (si.nMax - si.nPage as i32 + 1).max(si.nMin);
                let old_pos = si.nPos;

                si.nPos = match request as i32 {
                    SB_LINEUP => si.nPos - 1,                 // up/left arrow: one unit
                    SB_LINEDOWN => si.nPos + 1,               // down/right arrow
                    SB_PAGEUP => si.nPos - si.nPage as i32,   // click above thumb
                    SB_PAGEDOWN => si.nPos + si.nPage as i32, // click below
                    // Live drag: every pixel of movement fires another
                    // SB_THUMBTRACK. React to it and the view follows the
                    // thumb continuously.
                    SB_THUMBTRACK => si.nTrackPos,
                    // Thumb released: final position, one last message.
                    SB_THUMBPOSITION => si.nTrackPos,
                    _ => si.nPos, // SB_ENDSCROLL, SB_TOP/BOTTOM...: ignore here
                }
                .clamp(si.nMin, max_pos);

                // Write the new position back; the bar redraws its thumb.
                si.fMask = SIF_POS as u32; // only the position is a write
                _ = SetScrollInfo(bar, SB_CTL, &si, true);

                if si.nPos != old_pos {
                    state.value = si.nPos;
                    _ = InvalidateRect(Some(hwnd), None, true);
                }
                0
            }
            WM_PAINT => {
                let mut ps = PAINTSTRUCT::default();
                let hdc = BeginPaint(hwnd, &mut ps);
                let mut rect = RECT::default();
                _ = GetClientRect(hwnd, &mut rect);

                // A horizontal "fill gauge" driven by the scrollbar: value
                // 0..100 out of the client width right of the bar.
                let x0 = 60;
                let x1 = rect.right - 20;
                let y0 = 40;
                let h = 36;
                let fill_w = ((x1 - x0) * state.value) / RANGE_MAX;

                let frame = CreatePen(PS_SOLID, 1, rgb(0x80, 0x80, 0x80));
                let old_pen = SelectObject(hdc, frame as HGDIOBJ);
                let fill = CreateSolidBrush(rgb(0x30, 0x90, 0xD0));
                _ = Rectangle(hdc, x0, y0, x1 + 1, y0 + h); // empty frame first
                _ = FillRect(
                    hdc,
                    &RECT {
                        left: x0 + 1,
                        top: y0 + 1,
                        right: x0 + fill_w,
                        bottom: y0 + h,
                    },
                    fill,
                );
                _ = DeleteObject(fill as HGDIOBJ);
                SelectObject(hdc, old_pen);
                _ = DeleteObject(frame as HGDIOBJ);

                let text = to_wide(&format!("value = {} / 100", state.value));
                _ = SetBkMode(hdc, TRANSPARENT);
                _ = TextOutW(
                    hdc,
                    x0,
                    y0 + h + 12,
                    PCWSTR::from_raw(text.as_ptr()),
                    text.len() as i32 - 1,
                );
                _ = SetBkMode(hdc, OPAQUE);

                let hint = to_wide("arrows: ±1  ·  bar: ±page  ·  drag the thumb (THUMBTRACK)");
                _ = TextOutW(
                    hdc,
                    x0,
                    y0 + h + 44,
                    PCWSTR::from_raw(hint.as_ptr()),
                    hint.len() as i32 - 1,
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

fn to_wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}
