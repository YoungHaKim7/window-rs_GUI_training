//! e06 — Unicode Strings: Rust `&str` ↔ Win32 wide strings
//!
//! What you learn:
//! * Why every serious Win32 function ends in `W`: Windows text is UTF-16
//! * Three ways to hand Win32 a string:
//!   1. `w!("literal")` — compile-time, zero cost, literals only
//!   2. `Vec<u16>` + `PCWSTR::from_raw` — the manual runtime way
//!   3. `HSTRING` — a refcounted runtime string with automatic conversion
//! * Reading text *back* out of Windows (`GetWindowTextW`) into a Rust
//!   `String`
//!
//! Interaction: left-click updates the title with a runtime-built string;
//! right-click reads it back and prints it to the console.
//!
//! Run: `cargo run -p e06_unicode_strings`

use std::iter::once;

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
            lpszClassName: w!("e06_class"),
            style: (CS_HREDRAW | CS_VREDRAW) as u32,
            ..Default::default()
        };
        debug_assert!(RegisterClassW(&wc) != 0);

        // ---- Technique 2: runtime string via Vec<u16> ----------------
        // The title includes the tick count, so it *cannot* be a w! literal.
        // Encode to UTF-16, append the terminating NUL Win32 expects, and
        // point a PCWSTR at the buffer. The Vec must outlive the call —
        // keep it in a local binding, never inline!
        let title_wide = to_wide(&format!(
            "Unicode — built at run time (tick {})",
            GetTickCount64()
        ));

        let hwnd = CreateWindowExW(
            0,
            w!("e06_class"), // technique 1: compile-time literal
            PCWSTR::from_raw(title_wide.as_ptr()), // technique 2: runtime
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
            WM_LBUTTONDOWN => {
                // ---- Technique 3: HSTRING -----------------------------------
                // HSTRING is Windows' refcounted string type (the WinRT one).
                // The windows crate converts from &str/Into<String> for free
                // and implements the Param<PCWSTR> glue, so it can go
                // straight into any W function — no manual Vec juggling.
                let counter = GetTickCount64() / 1000;
                let title = HSTRING::from(format!("Clicked — uptime {counter} s"));
                _ = SetWindowTextW(hwnd, &title);
                0
            }
            WM_RBUTTONDOWN => {
                // ---- Reading text back --------------------------------------
                // API pattern: call once to learn the length, again to fill
                // a buffer you allocate. Then reinterpret the UTF-16.
                let len = GetWindowTextLengthW(hwnd);
                let mut buffer = vec![0u16; (len + 1) as usize]; // +1 for NUL
                let copied = GetWindowTextW(hwnd, PWSTR(buffer.as_mut_ptr()), len + 1);
                let text = String::from_utf16_lossy(&buffer[..copied as usize]);
                println!("Read back from the title bar: {text:?}");
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

/// &str → null-terminated UTF-16. The lingua franca converter. Keep the
/// returned Vec alive for as long as any PCWSTR pointing into it.
fn to_wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(once(0)).collect()
}
