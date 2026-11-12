//! e91 — Process List: enumerating everything Windows is running
//!
//! What you learn:
//! * The Tool Help snapshot: `CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS)`
//!   freezes a point-in-time copy of the process table; you then walk it
//!   with `Process32FirstW` / `Process32NextW` until Next returns FALSE —
//!   the classic "open, iterate, close" enumeration shape (the same shape
//!   as registry enums and FindFile loops)
//! * `PROCESSENTRY32W`: the per-process record. The gotchas that bite
//!   everyone once: you MUST set `dwSize = size_of::<PROCESSENTRY32W>()`
//!   before the first call (it doubles as a version check), and the name
//!   is a fixed `[WCHAR; 260]` array — read to the NUL, not all 260
//! * A snapshot HANDLE is still a HANDLE: `CloseHandle` it when done
//!
//! Task Manager's "Details" tab is this example plus icons and privileges.
//! Press Refresh (or launch/quit apps and press it again — the snapshot is
//! a photo, not a live feed).
//!
//! New Win32 APIs: `CreateToolhelp32Snapshot`, `Process32FirstW`,
//! `Process32NextW`, `PROCESSENTRY32W` (tlhelp32); `CloseHandle` (handleapi).
//!
//! Run: `cargo run -p e91_process_list`

use windows::{Win32::*, core::*};

const IDC_LIST: i32 = 1001;
const IDC_REFRESH: i32 = 1002;

#[derive(Default)]
struct State {
    hwnd_list: HWND,
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
            lpszClassName: w!("e91_class"),
            style: (CS_HREDRAW | CS_VREDRAW) as u32,
            ..Default::default()
        };
        debug_assert!(RegisterClassW(&wc) != 0);

        let hwnd = CreateWindowExW(
            0,
            w!("e91_class"),
            w!("e91 · Process List"),
            (WS_OVERLAPPEDWINDOW | WS_VISIBLE) as u32,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            480,
            520,
            None,
            None,
            Some(instance),
            None,
        );
        debug_assert!(!hwnd.is_null());

        let state = Box::into_raw(Box::<State>::default());
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, state as isize);

        let list = CreateWindowExW(
            0,
            w!("LISTBOX"),
            w!(""),
            (WS_CHILD | WS_VISIBLE | WS_BORDER | LBS_NOTIFY | WS_VSCROLL) as u32,
            20,
            20,
            420,
            400,
            Some(hwnd),
            Some(IDC_LIST as usize as HMENU),
            Some(instance),
            None,
        );
        debug_assert!(!list.is_null());

        let refresh = CreateWindowExW(
            0,
            w!("BUTTON"),
            w!("Refresh"),
            (WS_CHILD | WS_VISIBLE | BS_PUSHBUTTON) as u32,
            20,
            436,
            110,
            34,
            Some(hwnd),
            Some(IDC_REFRESH as usize as HMENU),
            Some(instance),
            None,
        );
        debug_assert!(!refresh.is_null());

        if let Some(s) = state_of(hwnd) {
            s.hwnd_list = list;
            refresh_list(hwnd, s.hwnd_list);
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

/// Fixed WCHAR array -> Rust String: cut at the first NUL.
fn wide_array_to_string(buf: &[u16]) -> String {
    let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    String::from_utf16_lossy(&buf[..len])
}

/// Take a fresh process snapshot and (re)fill the list box.
unsafe fn refresh_list(hwnd: HWND, hwnd_list: HWND) {
    unsafe {
        _ = SendMessageW(hwnd_list, LB_RESETCONTENT as u32, 0, 0);

        // TH32CS_SNAPPROCESS: "include the process list". 0 = all processes
        // (a pid here would snapshot just that one). On failure this returns
        // INVALID_HANDLE_VALUE (-1), NOT a null handle — a trap for anyone
        // testing .is_null().
        let snap = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS as u32, 0);
        if snap == INVALID_HANDLE_VALUE {
            _ = SetWindowTextW(hwnd, w!("e91 · CreateToolhelp32Snapshot failed"));
            return;
        }

        let mut entry = PROCESSENTRY32W::default();
        // The dwSize contract: you tell it which struct version you compiled
        // against; Process32FirstW fails outright if it disagrees.
        entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;

        let mut count = 0usize;
        if Process32FirstW(snap, &mut entry).as_bool() {
            loop {
                let name = wide_array_to_string(&entry.szExeFile);
                let line = format!("{:6}  {}", entry.th32ProcessID, name);
                let wide = to_wide(&line);
                _ = SendMessageW(hwnd_list, LB_ADDSTRING as u32, 0, wide.as_ptr() as isize);
                count += 1;

                if !Process32NextW(snap, &mut entry).as_bool() {
                    break; // normal end of the walk
                }
            }
        }

        // The snapshot is a kernel object like any other — close it, or it
        // leaks until process exit.
        _ = CloseHandle(snap);

        _ = SetWindowTextW(
            hwnd,
            &HSTRING::from(format!("e91 · Process List — {count} processes")),
        );
    }
}

fn to_wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
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
                if notification == BN_CLICKED as u32 && control_id == IDC_REFRESH as u32 {
                    refresh_list(hwnd, state.hwnd_list);
                }
                0
            }
            WM_SIZE => {
                // Keep the list + button glued to the window on resize:
                // LOWORD/HIWORD of lparam = new client width/height (there
                // is no GET_X_LPARAM-style helper in this crate — inline it).
                let w = (lparam & 0xffff) as u16 as i32;
                let h = ((lparam >> 16) & 0xffff) as u16 as i32;
                _ = MoveWindow(state.hwnd_list, 20, 20, w - 40, h - 80, true);
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
