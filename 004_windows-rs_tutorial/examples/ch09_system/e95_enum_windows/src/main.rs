//! e95 — EnumWindows: walking every top-level window on the desktop
//!
//! What you learn:
//! * `EnumWindows` — the Win32 enumeration idiom for callbacks: you hand it
//!   an `extern "system" fn(HWND, LPARAM) -> BOOL`, it calls you once per
//!   top-level window, and **returning FALSE from the callback stops the
//!   walk** — any nonzero value continues it (the exact mirror of e91's
//!   Process32Next loop, but push-style instead of pull-style)
//! * The `lparam` trick for context: Win32 callbacks take no user-data
//!   pointer by accident — they take exactly one, and here we smuggle a
//!   `*mut Vec<(HWND, String)>` through it so the callback can collect into
//!   ordinary Rust storage (the same `LPARAM`-as-pointer move wndprocs use
//!   with `CREATESTRUCTW` in e65)
//! * Filtering a window flood: `IsWindowVisible` and a nonzero
//!   `GetWindowTextLengthW` together keep only real, visible, titled
//!   windows — the raw enumeration includes tool windows, message-only
//!   parents and invisible scaffolding you never want in an Alt-Tab list
//! * `SetForegroundWindow` and its famous caveat: Windows (since 98/2000)
//!   lets only the *foreground* process give away the foreground, so an
//!   unfocused app calling it often gets only taskbar flashing instead.
//!   It's a deliberate anti-focus-steal rule, not a bug in your code
//!
//! Double-click a row to bring that window to the front (best effort —
//! see the caveat above). Refresh re-runs the enumeration.
//!
//! New Win32 APIs: `EnumWindows` + `WNDENUMPROC`, `IsWindowVisible`,
//! `GetWindowTextLengthW`, `GetWindowTextW`, `SetForegroundWindow`
//! (winuser); list-box recap (`LB_*`, `LBN_DBLCLK`) from e15/e21.
//!
//! Run: `cargo run -p e95_enum_windows`

use windows::{Win32::*, core::*};

const IDC_LIST: i32 = 1001;
const IDC_REFRESH: i32 = 1002;

#[derive(Default)]
struct State {
    hwnd_list: HWND,
    /// The HWND behind each row of the list box, kept in row order so the
    /// LB_GETCURSEL index from a double-click maps straight to a window.
    windows: Vec<HWND>,
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
            lpszClassName: w!("e95_class"),
            style: (CS_HREDRAW | CS_VREDRAW) as u32,
            ..Default::default()
        };
        debug_assert!(RegisterClassW(&wc) != 0);

        let hwnd = CreateWindowExW(
            0,
            w!("e95_class"),
            w!("e95 · Enum Windows — double-click a row to raise it"),
            (WS_OVERLAPPEDWINDOW | WS_VISIBLE) as u32,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            480,
            420,
            None,
            None,
            Some(instance),
            None,
        );
        debug_assert!(!hwnd.is_null());

        let state = Box::into_raw(Box::<State>::default());
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, state as isize);

        // LBS_NOTIFY is what makes a LISTBOX send WM_COMMAND for clicks;
        // without it, double-clicks stay inside the control.
        let list = CreateWindowExW(
            0,
            w!("LISTBOX"),
            w!(""),
            (WS_CHILD
                | WS_VISIBLE
                | WS_BORDER
                | LBS_NOTIFY
                | LBS_NOINTEGRALHEIGHT
                | WS_VSCROLL) as u32,
            12,
            12,
            440,
            330,
            Some(hwnd),
            Some(IDC_LIST as HMENU),
            Some(instance),
            None,
        );
        debug_assert!(!list.is_null());

        let refresh = CreateWindowExW(
            0,
            w!("BUTTON"),
            w!("Refresh"),
            (WS_CHILD | WS_VISIBLE | WS_TABSTOP) as u32,
            12,
            352,
            110,
            32,
            Some(hwnd),
            Some(IDC_REFRESH as HMENU),
            Some(instance),
            None,
        );
        debug_assert!(!refresh.is_null());

        if let Some(s) = state_of(hwnd) {
            s.hwnd_list = list;
        }

        // First population happens once the list control exists.
        refresh_list(hwnd);

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

/// Read a window's title as a Rust String. The classic two-step: ask for
/// the length first (NUL excluded), then fetch exactly that many UTF-16
/// units. Returns None for untitled windows.
unsafe fn window_title(hwnd: HWND) -> Option<String> {
    unsafe {
        let len = GetWindowTextLengthW(hwnd);
        if len <= 0 {
            return None;
        }
        let mut buf = vec![0u16; len as usize + 1]; // room for the NUL
        let copied = GetWindowTextW(hwnd, PWSTR::from_raw(buf.as_mut_ptr()), buf.len() as i32);
        if copied <= 0 {
            return None;
        }
        buf.truncate(copied as usize); // drop everything past what was written
        Some(String::from_utf16_lossy(&buf))
    }
}

/// The EnumWindows callback: called once per top-level window, in no
/// guaranteed order. `lparam` is whatever we passed to EnumWindows — our
/// collection pointer. Return FALSE (0) to stop early; anything else walks on.
extern "system" fn enum_proc(hwnd: HWND, lparam: LPARAM) -> BOOL {
    unsafe {
        let windows = &mut *(lparam as *mut Vec<(HWND, String)>);

        // Keep only visible, titled windows. The raw list contains dozens
        // of invisible helper windows (every process has some); they would
        // just be empty rows.
        if !IsWindowVisible(hwnd).as_bool() {
            return BOOL::from(true); // skip, but keep walking
        }
        if let Some(title) = window_title(hwnd) {
            windows.push((hwnd, title));
        }

        BOOL::from(true) // nonzero = "continue enumerating"
    }
}

/// Run one enumeration pass and rebuild the list box from the result.
unsafe fn refresh_list(hwnd: HWND) {
    unsafe {
        let Some(state) = state_of(hwnd) else {
            return;
        };

        let mut found: Vec<(HWND, String)> = Vec::new();
        // The LPARAM carries our *mut Vec — LPARAM is a plain isize alias,
        // so it's a plain cast, no constructor. The Vec outlives the call.
        _ = EnumWindows(Some(enum_proc), &mut found as *mut _ as isize);

        // Rebuild the control: clear both sides, then re-add row by row.
        // LB_ADDSTRING wants the string pointer in LPARAM.
        SendMessageW(state.hwnd_list, LB_RESETCONTENT as u32, 0, 0);
        state.windows.clear();
        for (hwnd_found, title) in &found {
            let wide: Vec<u16> = title.encode_utf16().chain(std::iter::once(0)).collect();
            SendMessageW(
                state.hwnd_list,
                LB_ADDSTRING as u32,
                0,
                wide.as_ptr() as isize,
            );
            state.windows.push(*hwnd_found);
        }

        _ = SetWindowTextW(
            hwnd,
            &HSTRING::from(format!(
                "e95 \u{b7} Enum Windows \u{2014} {} visible top-level windows (double-click to raise)",
                found.len()
            )),
        );
    }
}

extern "system" fn wndproc(hwnd: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe {
        let Some(state) = state_of(hwnd) else {
            return DefWindowProcW(hwnd, message, wparam, lparam);
        };

        match message as i32 {
            WM_COMMAND => {
                let id = (wparam & 0xffff) as u16 as i32;
                let code = ((wparam >> 16) & 0xffff) as u16 as i32;

                match (id, code) {
                    (IDC_REFRESH, BN_CLICKED) => {
                        refresh_list(hwnd);
                        0
                    }
                    (IDC_LIST, LBN_DBLCLK) => {
                        // Which row? Our parallel Vec<HWND> holds the answer.
                        let sel = SendMessageW(state.hwnd_list, LB_GETCURSEL as u32, 0, 0) as i32;
                        if sel != LB_ERR && (sel as usize) < state.windows.len() {
                            let target = state.windows[sel as usize];

                            // Restore first if minimized (SetForegroundWindow
                            // alone won't un-minimize it)...
                            if IsIconic(target).as_bool() {
                                _ = ShowWindow(target, SW_RESTORE);
                            }
                            // ...then ask to come forward. CAVEAT: the OS
                            // may refuse — only the foreground process is
                            // allowed to hand the foreground away, so from
                            // an unfocused app this often just flashes the
                            // target's taskbar button. By design.
                            _ = SetForegroundWindow(target);
                        }
                        0
                    }
                    _ => DefWindowProcW(hwnd, message, wparam, lparam),
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
