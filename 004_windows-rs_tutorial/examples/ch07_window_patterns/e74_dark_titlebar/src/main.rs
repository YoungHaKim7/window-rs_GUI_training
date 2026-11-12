//! e74 — Dark Title Bar: DwmSetWindowAttribute paints the frame for you
//!
//! What you learn:
//! * `DwmSetWindowAttribute` — the Desktop Window Manager's per-window
//!   knob panel. Unlike GDI (which only ever sees the CLIENT area), DWM
//!   owns the non-client frame — title bar, borders, caption buttons —
//!   and this call is how an app restyles it without owner-drawing the
//!   whole frame (WM_NCPAINT territory, deliberately out of scope here).
//! * `DWMWA_USE_IMMERSIVE_DARK_MODE` (value 20): the boolean that swaps
//!   the entire frame light↔dark in one shot. Historical wrinkle: on
//!   Windows 10 builds before 20H1 the same attribute number was **19**
//!   (an unreleased precursor flag) — robust code tries 20 first and
//!   falls back to 19, which is exactly what this example does.
//! * `DWMWA_CAPTION_COLOR` and `DWMWA_TEXT_COLOR` — direct COLORREF
//!   overrides of the title bar fill and the caption text. Win11 22000+
//!   only; passing `DWMWA_COLOR_DEFAULT` (0xFFFFFFFF) resets to system.
//! * Graceful degradation: every call returns an HRESULT and we IGNORE
//!   failures on purpose — on an OS without a DWM attribute the window
//!   must simply stay default-styled, never crash or "error-popup".
//!
//! Press D to flip dark/light. On Win11 the caption and text colors ride
//! along; on Win10 the immersive-dark flag alone still recolors the frame.
//!
//! New Win32 APIs: `DwmSetWindowAttribute`, `DWMWA_USE_IMMERSIVE_DARK_MODE`,
//! `DWMWA_CAPTION_COLOR`, `DWMWA_TEXT_COLOR`, `DWMWA_COLOR_DEFAULT`.
//!
//! Run: `cargo run -p e74_dark_titlebar`

use windows::{Win32::*, core::*};

/// DWMWA_USE_IMMERSIVE_DARK_MODE as a raw number: 19 on Win10 1809–2004,
/// 20 from 20H1 on. The vendored constant is 20; we also poke 19 so the
/// example behaves on old builds. (Both are DWORD attributes = a u32 that
/// must stay alive for the duration of the call.)
const DWMWA_USE_IMMERSIVE_DARK_MODE_19: u32 = 19;

#[derive(Default)]
struct State {
    dark: bool,
}

fn main() {
    unsafe {
        let instance = GetModuleHandleW(PCWSTR::null());
        debug_assert!(!instance.is_null());

        let wc = WNDCLASSW {
            lpfnWndProc: Some(wndproc),
            hInstance: instance,
            hCursor: LoadCursorW(None, PCWSTR(IDC_ARROW.0.cast())),
            // The brush is fixed; WM_PAINT overlays client colors to
            // match the frame, since a dark frame over a white client
            // would look unfinished.
            hbrBackground: ((COLOR_WINDOW + 1) as isize) as HBRUSH,
            lpszClassName: w!("e74_class"),
            style: (CS_HREDRAW | CS_VREDRAW) as u32,
            ..Default::default()
        };
        debug_assert!(RegisterClassW(&wc) != 0);

        let state = Box::into_raw(Box::new(State { dark: true }));

        let hwnd = CreateWindowExW(
            0,
            w!("e74_class"),
            w!("e74 · Dark Title Bar — press D to flip"),
            (WS_OVERLAPPEDWINDOW | WS_VISIBLE) as u32,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            560,
            320,
            None,
            None,
            Some(instance),
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

/// Thin wrapper: one attribute, one value, result ignored. HRESULTs are
/// #[must_use] here — `.ok()` into a discarded Result makes the intent
/// ("failure is fine") visible to both the reader and the compiler.
unsafe fn set_dword_attribute(hwnd: HWND, attribute: u32, value: u32) {
    unsafe {
        _ = DwmSetWindowAttribute(
            hwnd,
            attribute,
            &value as *const u32 as LPCVOID, // pvAttribute: pointer to the value
            std::mem::size_of::<u32>() as u32, // cbAttribute: its size
        )
        .ok();
    }
}

/// Apply (or re-apply) the whole frame styling for the current mode.
unsafe fn apply_theme(hwnd: HWND) {
    unsafe {
        let Some(state) = state_of(hwnd) else { return };

        // 1) The immersive dark flag — try 20, then the legacy 19. One of
        //    them works on anything Win10 1809+, which is the floor this
        //    attribute ever existed on.
        let on = state.dark as u32;
        set_dword_attribute(hwnd, DWMWA_USE_IMMERSIVE_DARK_MODE as u32, on);
        set_dword_attribute(hwnd, DWMWA_USE_IMMERSIVE_DARK_MODE_19, on);

        // 2) Explicit caption + text colors (Win11+; silently no-ops
        //    below). DWMWA_COLOR_DEFAULT hands the choice back to the OS.
        if state.dark {
            set_dword_attribute(hwnd, DWMWA_CAPTION_COLOR as u32, rgb(0x20, 0x20, 0x20));
            set_dword_attribute(hwnd, DWMWA_TEXT_COLOR as u32, rgb(0xF0, 0xF0, 0xF0));
        } else {
            set_dword_attribute(hwnd, DWMWA_CAPTION_COLOR as u32, DWMWA_COLOR_DEFAULT);
            set_dword_attribute(hwnd, DWMWA_TEXT_COLOR as u32, DWMWA_COLOR_DEFAULT);
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
                apply_theme(hwnd);
                0
            }

            WM_KEYDOWN => {
                if wparam as i32 == 'D' as i32 {
                    if let Some(state) = state_of(hwnd) {
                        state.dark = !state.dark;
                    }
                    apply_theme(hwnd);
                    0
                } else {
                    DefWindowProcW(hwnd, message, wparam, lparam)
                }
            }

            WM_PAINT => {
                let mut ps = PAINTSTRUCT::default();
                let hdc = BeginPaint(hwnd, &mut ps);
                let mut rc = RECT::default();
                _ = GetClientRect(hwnd, &mut rc);

                let Some(state) = state_of(hwnd) else {
                    _ = EndPaint(hwnd, &ps);
                    return 0;
                };

                // Client area follows the frame so the whole window reads
                // as one theme (DWM never touches client pixels — that is
                // still our job).
                let (bg, fg, note) = if state.dark {
                    (rgb(0x28, 0x28, 0x28), rgb(0xE8, 0xE8, 0xE8), "dark")
                } else {
                    (rgb(0xFA, 0xFA, 0xFA), rgb(0x20, 0x20, 0x20), "light")
                };
                let brush = CreateSolidBrush(bg);
                _ = FillRect(hdc, &rc, brush);
                _ = DeleteObject(brush as HGDIOBJ);

                _ = SetBkMode(hdc, TRANSPARENT);
                _ = SetTextColor(hdc, fg);
                let wide = to_wide(&format!(
                    "theme: {note}\n\nDWMWA_USE_IMMERSIVE_DARK_MODE tried as 20 AND 19.\n\
                     caption/text colors: Win11+ only — older builds just ignore them\n\
                     (that is why every Dwm call here drops its HRESULT on purpose).\n\n\
                     press D to flip."
                ));
                DrawTextW(
                    hdc,
                    PCWSTR::from_raw(wide.as_ptr()),
                    -1,
                    &mut rc,
                    (DT_CENTER | DT_VCENTER | DT_WORDBREAK) as u32,
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
