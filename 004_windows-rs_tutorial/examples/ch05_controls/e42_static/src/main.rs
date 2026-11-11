//! e42 — Static Controls: labels, frames, and controls that never talk back
//!
//! What you learn:
//! * STATIC is the built-in control for text labels and simple line art
//!   (`SS_LEFT` / `SS_CENTER` / `SS_RIGHT` alignment, `SS_ETCHEDFRAME`,
//!   `SS_SUNKEN` borders)
//! * STATICs are **disabled for input by design**: they receive no keyboard
//!   focus and no mouse clicks — clicks fall through to the parent, which
//!   is why they make perfect captions
//! * A control's "text" (set via the window name at creation) is all a
//!   STATIC ever shows
//!
//! New Win32 APIs: `w!("STATIC")` child class, `SS_*` styles.
//!
//! Run: `cargo run -p e42_static`

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
            lpszClassName: w!("e42_class"),
            style: (CS_HREDRAW | CS_VREDRAW) as u32,
            ..Default::default()
        };
        debug_assert!(RegisterClassW(&wc) != 0);

        let hwnd = CreateWindowExW(
            0,
            w!("e42_class"),
            w!("e42 · Static — labels, frames, sunken boxes"),
            (WS_OVERLAPPEDWINDOW | WS_VISIBLE) as u32,
            CW_USEDEFAULT, CW_USEDEFAULT, 560, 340,
            None, None, Some(instance), None,
        );
        debug_assert!(!hwnd.is_null());

        // ---- The child factory ----------------------------------------------
        // One helper so the SS_* style is the only thing that varies. Note
        // `hmenu = None`: a pure label never sends WM_COMMAND, so it needs
        // no control id.
        let mk_static = |style: i32, text: PCWSTR, ex_style: u32, x: i32, y: i32, w: i32, h: i32| {
            CreateWindowExW(
                ex_style,
                w!("STATIC"),
                text,
                (WS_CHILD | WS_VISIBLE | style) as u32,
                x, y, w, h,
                Some(hwnd),
                None,
                Some(instance),
                None,
            )
        };

        // ---- Column 1: the three text alignments -----------------------------
        // SS_LEFT is 0 (the default) — spelled out anyway for symmetry.
        _ = mk_static(SS_LEFT,   w!("SS_LEFT — flush left"), 0, 20, 20, 170, 20);
        _ = mk_static(SS_CENTER, w!("SS_CENTER"),            0, 20, 48, 170, 20);
        _ = mk_static(SS_RIGHT,  w!("SS_RIGHT — flush right"), 0, 20, 76, 170, 20);

        // ---- Column 2: frames and sunken boxes --------------------------------
        // SS_ETCHEDFRAME: the classic two-tone "etched" border — exactly what
        // a group box draws. Give it no text; it is pure line art.
        _ = mk_static(SS_ETCHEDFRAME, w!(""), 0, 210, 20, 150, 40);
        // SS_SUNKEN: a simple sunken edge — the look of a read-only value box.
        _ = mk_static(SS_SUNKEN, w!(""), 0, 210, 72, 150, 40);
        // The everyday "sunken box" is often SS_SUNKEN combined with the
        // WS_EX_CLIENTEDGE extended style — heavier, edit-control-like border.
        _ = mk_static(SS_SUNKEN, w!(""), WS_EX_CLIENTEDGE as usize as u32, 380, 20, 150, 40);

        // Captions naming each frame — STATICs captioning STATICs.
        _ = mk_static(SS_LEFT, w!("SS_ETCHEDFRAME"), 0, 210, 66, 150, 18);
        _ = mk_static(SS_LEFT, w!("SS_SUNKEN"),      0, 210, 118, 150, 18);
        _ = mk_static(SS_LEFT, w!("SS_SUNKEN + EX_CLIENTEDGE"), 0, 380, 66, 170, 18);

        // ---- The input lesson --------------------------------------------------
        // Click anywhere in the client area — including on a label — and the
        // PARENT gets WM_LBUTTONDOWN. STATIC controls never receive input
        // messages themselves: no focus, no clicks. That is not a bug, it is
        // the feature that lets you stack captions over interactive areas.
        _ = mk_static(
            SS_CENTER,
            w!("Labels eat no input — click anywhere; the parent answers"),
            0,
            20, 170, 510, 20,
        );

        let mut message = MSG::default();
        while GetMessageW(&mut message, None, 0, 0).0 > 0 {
            _ = TranslateMessage(&message);
            DispatchMessageW(&message);
        }
    }
}

extern "system" fn wndproc(hwnd: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe {
        match message as i32 {
            WM_LBUTTONDOWN => {
                // Fires wherever you click — over a STATIC too, because the
                // control is input-transparent and lets the message fall
                // through to us. (Its whole job: draw and stay out of the way.)
                _ = MessageBoxW(
                    Some(hwnd),
                    w!("You clicked the PARENT window.\nA STATIC never sees this message — it is disabled for input by design."),
                    w!("Clicks fall through"),
                    (MB_OK | MB_ICONINFORMATION) as u32,
                );
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
