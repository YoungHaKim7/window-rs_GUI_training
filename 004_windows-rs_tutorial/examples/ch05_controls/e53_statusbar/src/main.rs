//! e53 — Status Bar: parts, live text, and the WM_SIZE forwarding contract
//!
//! What you learn:
//! * The status bar (`msctls_statusbar32`, exported as `STATUSCLASSNAMEW`)
//! * `SB_SETPARTS` — divide the bar into independent cells by giving each
//!   one's right edge (in pixels); `-1` for the last part means "stretch
//!   to the end"
//! * `SB_SETTEXTW` — write one part's text by index; the classic use is
//!   live mouse coordinates in part 0 and a counter/size/state in part 1
//! * **The resize contract**: the status bar parks itself at the bottom of
//!   the parent, but it only re-sticks there if the PARENT forwards its
//!   `WM_SIZE` to the control (`SendMessageW(statusbar, WM_SIZE, ...)`).
//!   Forgetting this line is why status bars "float mid-window" after a
//!   resize. Toolbars (e54) share the same contract.
//!
//! New Win32 APIs: `STATUSCLASSNAMEW`, `SB_SETPARTS`, `SB_SETTEXTW`,
//! `WM_MOUSEMOVE` → `WM_SIZE` forwarding.
//!
//! Run: `cargo run -p e53_statusbar`

use windows::{Win32::*, core::*};

#[derive(Default)]
struct State {
    hwnd_status: HWND,
    clicks: i32,
}

fn main() {
    unsafe {
        // The status bar belongs to the ICC_BAR_CLASSES family (same flag
        // as the trackbar in e51).
        let icc = INITCOMMONCONTROLSEX {
            dwSize: std::mem::size_of::<INITCOMMONCONTROLSEX>() as u32,
            dwICC: ICC_BAR_CLASSES as u32,
        };
        _ = InitCommonControlsEx(&icc);

        let instance = GetModuleHandleW(PCWSTR::null());
        debug_assert!(!instance.is_null());

        let wc = WNDCLASSW {
            lpfnWndProc: Some(wndproc),
            hInstance: instance,
            hCursor: LoadCursorW(None, PCWSTR(IDC_ARROW.0.cast())),
            hbrBackground: ((COLOR_WINDOW + 1) as isize) as HBRUSH,
            lpszClassName: w!("e53_class"),
            style: (CS_HREDRAW | CS_VREDRAW) as u32,
            ..Default::default()
        };
        debug_assert!(RegisterClassW(&wc) != 0);

        let hwnd = CreateWindowExW(
            0,
            w!("e53_class"),
            w!("e53 · Status Bar — move the mouse, click, resize"),
            (WS_OVERLAPPEDWINDOW | WS_VISIBLE) as u32,
            CW_USEDEFAULT, CW_USEDEFAULT, 520, 280,
            None, None, Some(instance), None,
        );
        debug_assert!(!hwnd.is_null());

        let state = Box::into_raw(Box::<State>::default());
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, state as isize);

        // ---- The status bar ---------------------------------------------------
        // STATUSCLASSNAMEW is the crate's exported name for the literal
        // w!("msctls_statusbar32") — a PCWSTR constant, so it slots straight
        // into CreateWindowExW. The y/height you pass are advisory: the
        // control computes its own height from the font, and welds itself to
        // the bottom of the parent's client area.
        let status = CreateWindowExW(
            0,
            STATUSCLASSNAMEW,
            w!(""), // no initial text; parts get their text below
            (WS_CHILD | WS_VISIBLE) as u32,
            0, 0, 0, 0, // position/size self-managed by the control
            Some(hwnd), None, Some(instance), None,
        );
        debug_assert!(!status.is_null());

        // Two parts: part 0 ends at pixel 260; part 1 stretches to the end
        // (-1 = "whatever remains"). lparam points at the edge array —
        // one i32 per part, and the array must outlive the call.
        let edges = [260i32, -1i32];
        SendMessageW(
            status,
            SB_SETPARTS as u32,
            edges.len(),
            edges.as_ptr() as LPARAM,
        );

        // Seed both cells so the bar is never blank at first paint.
        set_part_text(status, 0, "move the mouse — coordinates land here");
        set_part_text(status, 1, "clicks: 0");

        if let Some(s) = state_of(hwnd) {
            s.hwnd_status = status;
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

/// Write one part of a status bar. wparam = the part index (optionally OR'd
/// with SBT_* draw flags), lparam = the string pointer. The control copies
/// the text, so the HSTRING only needs to live through this call.
fn set_part_text(status: HWND, part: usize, text: &str) {
    let wide = HSTRING::from(text);
    unsafe {
        SendMessageW(status, SB_SETTEXTW as u32, part, wide.as_ptr() as LPARAM);
    }
}

fn mouse_pos(lparam: LPARAM) -> (i32, i32) {
    // Through i16: the packed coordinates are signed (STYLE.md's helper).
    let x = (lparam & 0xffff) as i16 as i32;
    let y = ((lparam >> 16) & 0xffff) as i16 as i32;
    (x, y)
}

extern "system" fn wndproc(hwnd: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe {
        let Some(state) = state_of(hwnd) else {
            return DefWindowProcW(hwnd, message, wparam, lparam);
        };

        match message as i32 {
            // ---- The resize contract ------------------------------------------
            // A status bar does not listen for its parent's size. YOU do,
            // and hand the message down verbatim (wparam and lparam, not
            // zeros — the control reads the new size out of them). This
            // one line is the difference between a bar that hugs the
            // bottom edge on every resize and one that hovers mid-air.
            WM_SIZE => {
                SendMessageW(state.hwnd_status, WM_SIZE as u32, wparam, lparam);
                0
            }
            WM_MOUSEMOVE => {
                // Status text is THE sink for high-frequency updates —
                // writing a cell is cheap (the control repaints just that
                // cell), so doing it on every mouse move is idiomatic,
                // not wasteful.
                let (x, y) = mouse_pos(lparam);
                set_part_text(state.hwnd_status, 0, &format!("cursor: {x}, {y}"));
                0
            }
            WM_LBUTTONDOWN => {
                state.clicks += 1;
                set_part_text(state.hwnd_status, 1, &format!("clicks: {}", state.clicks));
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
