//! e04 — Hello, Paint!: drawing text in the client area
//!
//! What you learn:
//! * `WM_PAINT`: the message Windows sends whenever the client area needs
//!   redrawing (first show, resize, un-cover, minimize/restore, ...)
//! * The `BeginPaint` / `EndPaint` pair and the device context (HDC)
//! * `DrawTextW` with the `DT_CENTER | DT_VCENTER | DT_SINGLELINE` flags
//! * Why you must never cache an HDC — get it fresh in every WM_PAINT
//!
//! New Win32 APIs:
//! * `BeginPaint` / `EndPaint` — validate the dirty region, borrow an HDC
//! * `GetClientRect` — the drawable rectangle, in *client coordinates*
//! * `DrawTextW` — formatted text drawing (alignment, wrapping, ellipsis...)
//!
//! Run: `cargo run -p e04_hello_paint`

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
            lpszClassName: w!("e04_class"),
            style: (CS_HREDRAW | CS_VREDRAW) as u32,
            ..Default::default()
        };
        debug_assert!(RegisterClassW(&wc) != 0);

        let hwnd = CreateWindowExW(
            0,
            w!("e04_class"),
            w!("e04 · Hello, Paint!"),
            (WS_OVERLAPPEDWINDOW | WS_VISIBLE) as u32,
            CW_USEDEFAULT, CW_USEDEFAULT, 640, 240,
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

extern "system" fn wndproc(hwnd: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe {
        match message as i32 {
            WM_PAINT => {
                // PAINTSTRUCT carries the clip region and other details;
                // BeginPaint fills it in and hands us a device context.
                // The HDC is a *borrowed* handle valid only until EndPaint.
                let mut ps = PAINTSTRUCT::default();
                let hdc = BeginPaint(hwnd, &mut ps);

                // The client area = the drawable part, excluding title bar
                // and borders. Coordinates start at (0,0) in its top-left.
                let mut rect = RECT::default();
                _ = GetClientRect(hwnd, &mut rect);

                // DrawTextW is the "formatted" text API: give it the text,
                // a rectangle, and flags; it does the layout.
                //   -1 length  = "the string is null-terminated"
                //   DT_SINGLELINE = no wrapping
                //   rect is &mut because DT_* can modify it (e.g. DT_CALCRECT)
                DrawTextW(
                    hdc,
                    w!("Hello, World!"),
                    -1,
                    &mut rect,
                    (DT_CENTER | DT_VCENTER | DT_SINGLELINE) as u32,
                );

                // EndPaint releases the DC and — crucially — *validates*
                // the painted region. Without it Windows would keep sending
                // WM_PAINT forever (try commenting it out!).
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
