//! e13 — Mouse Move & the Window-State Pattern: the most important Rust
//! idiom in this whole tutorial
//!
//! What you learn:
//! * **Attaching state to a window** — `Box<State>` leaked into
//!   `GWLP_USERDATA`, retrieved at the top of wndproc. This is how every
//!   serious windows-rs program stores per-window data without globals.
//!   (Introduced here out of necessity — drawing anything that *persists*
//!   needs memory. e65 gives the pattern its full treatment: multiple
//!   windows, safe teardown, when the pointer is and isn't there.)
//! * `WM_MOUSEMOVE` — fires constantly; only act on it when it matters
//! * `SetCapture` / `ReleaseCapture` — keeping the mouse on a leash while
//!   dragging, so the gesture survives leaving the window
//! * Rubber-band drawing: full model redrawn in WM_PAINT each frame
//!
//! Drag with the left button to stretch a line; release to keep it.
//!
//! New Win32 APIs: `SetCapture`, `ReleaseCapture`, `MoveToEx`, `LineTo`.
//!
//! Run: `cargo run -p e13_mouse_move`

use windows::{Win32::*, core::*};

/// Everything this window knows. Plain Rust data — the windowing system
/// never sees it; we just hide a pointer to it in the window itself.
#[derive(Default)]
struct State {
    dragging: bool,
    start: (i32, i32),                    // anchor point of the in-flight line
    current: (i32, i32),                  // mouse position while dragging
    lines: Vec<((i32, i32), (i32, i32))>, // committed lines
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
            lpszClassName: w!("e13_class"),
            style: (CS_HREDRAW | CS_VREDRAW) as u32,
            ..Default::default()
        };
        debug_assert!(RegisterClassW(&wc) != 0);

        let hwnd = CreateWindowExW(
            0,
            w!("e13_class"),
            w!("e13 · Mouse Move — drag to draw lines"),
            (WS_OVERLAPPEDWINDOW | WS_VISIBLE) as u32,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            640,
            400,
            None,
            None,
            Some(instance),
            None,
        );
        debug_assert!(!hwnd.is_null());

        // ---- Attaching the state ---------------------------------------
        // 1. Move the State to the heap and forget it (Box::into_raw).
        // 2. Store that raw pointer in the window's GWLP_USERDATA slot.
        // From now on, wndproc can recover `&mut State` from the HWND
        // alone — the window becomes a handle to our data, literally.
        let state = Box::into_raw(Box::<State>::default());
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, state as isize);

        let mut message = MSG::default();
        while GetMessageW(&mut message, None, 0, 0).0 > 0 {
            _ = TranslateMessage(&message);
            DispatchMessageW(&message);
        }
    }
}

/// Fish the state back out of the window. Returns None for the handful of
/// messages that arrive before our SetWindowLongPtrW ran (WM_CREATE and
/// friends) — those simply take the default path.
unsafe fn state_of(hwnd: HWND) -> Option<&'static mut State> {
    unsafe {
        let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut State;
        if ptr.is_null() { None } else { Some(&mut *ptr) }
    }
}

fn mouse_pos(lparam: LPARAM) -> (i32, i32) {
    let x = (lparam & 0xffff) as i16 as i32;
    let y = ((lparam >> 16) & 0xffff) as i16 as i32;
    (x, y)
}

fn rgb(red: u8, green: u8, blue: u8) -> COLORREF {
    (red as u32) | ((green as u32) << 8) | ((blue as u32) << 16)
}

extern "system" fn wndproc(hwnd: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe {
        // Early messages (before the pointer is attached) take the default
        // path. Note this check must come BEFORE touching state.
        let Some(state) = state_of(hwnd) else {
            return DefWindowProcW(hwnd, message, wparam, lparam);
        };

        match message as i32 {
            WM_LBUTTONDOWN => {
                // Capture the mouse: all further mouse messages come to us
                // even outside the window — the drag keeps working if the
                // user's hand wanders off the edge. Released on button-up.
                _ = SetCapture(hwnd);
                state.dragging = true;
                state.start = mouse_pos(lparam);
                state.current = state.start;
                0
            }
            WM_MOUSEMOVE => {
                // This message fires on EVERY pixel of movement. Updating
                // state is cheap; repainting is not — but during a drag
                // there is no way around it (that flicker you might notice
                // gets fixed properly by double buffering in e26).
                state.current = mouse_pos(lparam);
                if state.dragging {
                    _ = InvalidateRect(Some(hwnd), None, true);
                }
                0
            }
            WM_LBUTTONUP => {
                if state.dragging {
                    _ = ReleaseCapture();
                    state.dragging = false;
                    // The finished line joins the model — this is what makes
                    // it SURVIVE repaints, unlike e12's dots.
                    state.lines.push((state.start, state.current));
                    _ = InvalidateRect(Some(hwnd), None, true);
                }
                0
            }
            WM_PAINT => {
                let mut ps = PAINTSTRUCT::default();
                let hdc = BeginPaint(hwnd, &mut ps);

                let pen = CreatePen(PS_SOLID, 2, rgb(0x20, 0x20, 0x80));
                let old_pen = SelectObject(hdc, pen as HGDIOBJ);

                // Draw the committed model...
                for &(from, to) in &state.lines {
                    _ = MoveToEx(hdc, from.0, from.1, None);
                    _ = LineTo(hdc, to.0, to.1);
                }

                // ...plus the live rubber band in a different color.
                if state.dragging {
                    let live = CreatePen(PS_DASH, 1, rgb(0xA0, 0x30, 0x30));
                    let old = SelectObject(hdc, live as HGDIOBJ);
                    _ = MoveToEx(hdc, state.start.0, state.start.1, None);
                    _ = LineTo(hdc, state.current.0, state.current.1);
                    SelectObject(hdc, old);
                    _ = DeleteObject(live as HGDIOBJ);
                }

                SelectObject(hdc, old_pen);
                _ = DeleteObject(pen as HGDIOBJ);
                _ = EndPaint(hwnd, &ps);
                0
            }
            WM_DESTROY => {
                // Reclaim exactly once: take the pointer out of the window
                // first, then drop the Box. (e65 shows the subtleties —
                // WM_NCDESTROY ordering, multiple windows, re-entrancy.)
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
