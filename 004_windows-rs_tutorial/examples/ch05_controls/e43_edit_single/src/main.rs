//! e43 — Single-Line Edit: reading text out of a control
//!
//! What you learn:
//! * The EDIT control: single-line text entry, prefilled at creation via
//!   `SetWindowTextW`, read back with `GetWindowTextW` into a Rust `String`
//! * `WS_EX_CLIENTEDGE` — the sunken border that visually says "type here"
//! * **Flipping styles at runtime**: read the style with
//!   `GetWindowLongPtrW(hwnd, GWL_STYLE)`, XOR in `ES_PASSWORD`, write it
//!   back with `SetWindowLongPtrW` — the same technique works for almost
//!   any WS_/BS_/ES_ style on any control
//!
//! New Win32 APIs: `GetWindowTextLengthW`, `GetWindowTextW`, `SetWindowTextW`,
//! `GetWindowLongPtrW` / `SetWindowLongPtrW` with `GWL_STYLE`.
//!
//! Run: `cargo run -p e43_edit_single`

use windows::{Win32::*, core::*};

const IDC_ECHO: i32 = 1001; // "Echo" button
const IDC_TOGGLE: i32 = 1002; // password on/off button

#[derive(Default)]
struct State {
    hwnd_edit: HWND, // default = null; filled in right after creation
    masked: bool, // is ES_PASSWORD currently set?
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
            lpszClassName: w!("e43_class"),
            style: (CS_HREDRAW | CS_VREDRAW) as u32,
            ..Default::default()
        };
        debug_assert!(RegisterClassW(&wc) != 0);

        let hwnd = CreateWindowExW(
            0,
            w!("e43_class"),
            w!("e43 · Edit — type, then Echo"),
            (WS_OVERLAPPEDWINDOW | WS_VISIBLE) as u32,
            CW_USEDEFAULT, CW_USEDEFAULT, 480, 200,
            None, None, Some(instance), None,
        );
        debug_assert!(!hwnd.is_null());

        let state = Box::into_raw(Box::<State>::default());
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, state as isize);

        // ---- The EDIT child ----------------------------------------------------
        // dwexstyle: WS_EX_CLIENTEDGE gives the classic sunken "field" border
        // (EDIT without it looks flat as a label — try removing it).
        // dwstyle: no ES_ style = plain single-line edit. Variants you can
        // OR in: ES_UPPERCASE/ES_LOWERCASE (force case), ES_NUMBER (digits
        // only), ES_PASSWORD (bullets — toggled at runtime below),
        // ES_READONLY, ES_RIGHT, ES_OEMCONVERT. Multiline is e44.
        let hwnd_edit = CreateWindowExW(
            WS_EX_CLIENTEDGE as usize as u32,
            w!("EDIT"),
            w!(""),
            (WS_CHILD | WS_VISIBLE) as u32,
            20, 24, 320, 26,
            Some(hwnd), None, Some(instance), None,
        );
        debug_assert!(!hwnd_edit.is_null());

        // Prefill with SetWindowTextW — the same API that retitles windows
        // sets a control's content (a control's text IS its window text).
        _ = SetWindowTextW(hwnd_edit, w!("hello from SetWindowTextW"));

        // Focus the edit so you can type immediately.
        _ = SetFocus(Some(hwnd_edit));

        let hwnd_echo = CreateWindowExW(
            0,
            w!("BUTTON"),
            w!("Echo"),
            (WS_CHILD | WS_VISIBLE | BS_PUSHBUTTON) as u32,
            352, 24, 90, 26,
            Some(hwnd), Some(IDC_ECHO as usize as HMENU), Some(instance), None,
        );
        debug_assert!(!hwnd_echo.is_null());

        let hwnd_toggle = CreateWindowExW(
            0,
            w!("BUTTON"),
            w!("Password: off"),
            (WS_CHILD | WS_VISIBLE | BS_PUSHBUTTON) as u32,
            20, 64, 180, 26,
            Some(hwnd), Some(IDC_TOGGLE as usize as HMENU), Some(instance), None,
        );
        debug_assert!(!hwnd_toggle.is_null());

        // Stash the edit HWND in the state so wndproc can reach it.
        if let Some(s) = unsafe { state_of(hwnd) } {
            s.hwnd_edit = hwnd_edit;
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

/// Read a window's (control's) full text into a String. Length first, then
/// the bytes — the two-call dance every Win32 string read performs.
unsafe fn window_text(hwnd: HWND) -> String {
    unsafe {
        let len = GetWindowTextLengthW(hwnd);
        let mut buf = vec![0u16; len as usize + 1];
        let copied = GetWindowTextW(hwnd, PWSTR(buf.as_mut_ptr()), buf.len() as i32);
        String::from_utf16_lossy(&buf[..copied.max(0) as usize])
    }
}

extern "system" fn wndproc(hwnd: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe {
        let Some(state) = state_of(hwnd) else {
            return DefWindowProcW(hwnd, message, wparam, lparam);
        };

        match message as i32 {
            WM_COMMAND => {
                let notification = ((wparam >> 16) & 0xffff) as u32;
                let control_id = (wparam & 0xffff) as u32;
                if notification != BN_CLICKED as u32 {
                    return 0;
                }

                match control_id as i32 {
                    IDC_ECHO => {
                        // Pull the edit's text out into Rust and bounce it
                        // back at the user two ways.
                        let text = window_text(state.hwnd_edit);
                        if text.is_empty() {
                            _ = MessageBoxW(
                                Some(hwnd),
                                w!("The edit box is empty — type something first."),
                                w!("Echo"),
                                MB_ICONWARNING as u32,
                            );
                            return 0;
                        }
                        _ = SetWindowTextW(hwnd, &HSTRING::from(format!("e43 · you typed: {text}")));
                        _ = MessageBoxW(
                            Some(hwnd),
                            &HSTRING::from(format!("The edit contains:\n\n{text}")),
                            w!("Echo"),
                            (MB_OK | MB_ICONINFORMATION) as u32,
                        );
                        0
                    }
                    IDC_TOGGLE => {
                        // ---- Runtime style flip -----------------------------------
                        // Styles are just bits in the window's GWL_STYLE slot.
                        // XOR toggles ES_PASSWORD on/off in one instruction;
                        // the control repaints itself to match (bullets vs
                        // text). The same read-modify-write works for nearly
                        // every WS_/ES_/BS_ style on any control.
                        let style = GetWindowLongPtrW(state.hwnd_edit, GWL_STYLE);
                        let new_style = style ^ ES_PASSWORD as isize;
                        SetWindowLongPtrW(state.hwnd_edit, GWL_STYLE, new_style);
                        state.masked = new_style & ES_PASSWORD as isize != 0;
                        // Relabel the toggle button so the state is visible.
                        // (We could also use the WM_COMMAND lparam, which IS
                        // the button's HWND for BN_CLICKED.)
                        let hwnd_toggle = GetDlgItem(Some(hwnd), IDC_TOGGLE);
                        _ = SetWindowTextW(
                            hwnd_toggle,
                            if state.masked { w!("Password: on") } else { w!("Password: off") },
                        );
                        0
                    }
                    _ => 0,
                }
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
