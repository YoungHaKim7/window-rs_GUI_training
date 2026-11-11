//! e41 — Button: the "hello world" of child controls
//!
//! What you learn:
//! * A BUTTON is just a child window created with `CreateWindowExW`,
//!   class name `w!("BUTTON")`, style `WS_CHILD | WS_VISIBLE` plus a
//!   `BS_*` style that picks the flavor (push button, checkbox, radio...)
//! * The child's **control id** travels through the `hmenu` parameter —
//!   for a child window that parameter is NOT a menu but the id you get
//!   back in `WM_COMMAND`
//! * `WM_COMMAND`: `HIWORD(wparam)` = notification code (`BN_CLICKED`),
//!   `LOWORD(wparam)` = control id, `lparam` = the control's own HWND
//!
//! New Win32 APIs: `CreateWindowExW` (child flavor), `WM_COMMAND`,
//! `BN_CLICKED`, `SetWindowTextW`.
//!
//! Run: `cargo run -p e41_button`

use windows::{Win32::*, core::*};

/// Control id for our button. You choose these freely; they identify the
/// control inside WM_COMMAND (menus and accelerators share the id space).
const IDC_CLICK: i32 = 1001;

/// The window-state pattern from e13 — even a click counter lives here,
/// never in a `static mut`.
#[derive(Default)]
struct State {
    clicks: i32,
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
            lpszClassName: w!("e41_class"),
            style: (CS_HREDRAW | CS_VREDRAW) as u32,
            ..Default::default()
        };
        debug_assert!(RegisterClassW(&wc) != 0);

        let hwnd = CreateWindowExW(
            0,
            w!("e41_class"),
            w!("e41 · Button — clicked 0 times"),
            (WS_OVERLAPPEDWINDOW | WS_VISIBLE) as u32,
            CW_USEDEFAULT, CW_USEDEFAULT, 420, 200,
            None, None, Some(instance), None,
        );
        debug_assert!(!hwnd.is_null());

        // Attach the state BEFORE creating children (see e13 for the pattern).
        let state = Box::into_raw(Box::<State>::default());
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, state as isize);

        // ---- The child control ------------------------------------------------
        // Everything here reads exactly like the top-level window creation in
        // e05, except for three parameters:
        //   * style gains WS_CHILD (owned by the parent, coordinates relative
        //     to its client area) and WS_VISIBLE — children start hidden.
        //   * hwndparent: Some(hwnd) — this is what makes it a *child*.
        //   * hmenu: for a child window this is the **control id**, not a
        //     menu handle. Hence the cast `1001usize as HMENU` — the type is
        //     a lie of convenience dating back to 16-bit Windows.
        // BS_PUSHBUTTON is the plain 3D push button (value 0 — the default).
        let _hwnd_button = CreateWindowExW(
            0,
            w!("BUTTON"),               // one of the built-in control classes
            w!("Click me"),             // the button label = its window text
            (WS_CHILD | WS_VISIBLE | BS_PUSHBUTTON) as u32,
            24, 24, 120, 32,            // x, y, w, h — in PARENT's client pixels
            Some(hwnd),                 // parent: our main window
            Some(IDC_CLICK as usize as HMENU), // control id smuggled through hmenu
            Some(instance),
            None,
        );
        debug_assert!(!_hwnd_button.is_null());

        let mut message = MSG::default();
        while GetMessageW(&mut message, None, 0, 0).0 > 0 {
            _ = TranslateMessage(&message);
            DispatchMessageW(&message);
        }
    }
}

/// Fish the state back out of the window (e13 pattern).
unsafe fn state_of(hwnd: HWND) -> Option<&'static mut State> {
    unsafe {
        let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut State;
        (!ptr.is_null()).then(|| &mut *ptr)
    }
}

extern "system" fn wndproc(hwnd: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe {
        let Some(state) = state_of(hwnd) else {
            return DefWindowProcW(hwnd, message, wparam, lparam);
        };

        match message as i32 {
            WM_COMMAND => {
                // WM_COMMAND packs its payload into wparam:
                //   HIWORD (top 16 bits) = notification code from the control
                //   LOWORD (bottom 16)   = the control id we passed in hmenu
                // lparam = the control's own HWND (handy, but you rarely need
                // it when ids are unique). There is no HIWORD()/LOWORD() in
                // this crate — shift and mask yourself.
                let notification = ((wparam >> 16) & 0xffff) as u32;
                let control_id = (wparam & 0xffff) as u32;

                if notification == BN_CLICKED as u32 && control_id == IDC_CLICK as u32 {
                    // BN_CLICKED = pressed AND released on the button.
                    state.clicks += 1;

                    // A control never touches its parent's title; we do that
                    // ourselves. HSTRING satisfies the string parameter.
                    _ = SetWindowTextW(
                        hwnd,
                        &HSTRING::from(format!("e41 · Button — clicked {} times", state.clicks)),
                    );
                }
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
