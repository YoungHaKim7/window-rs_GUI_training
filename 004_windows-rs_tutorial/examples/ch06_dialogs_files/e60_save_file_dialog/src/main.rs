//! e60 — Save File Dialog: GetSaveFileNameW, and really writing the file
//!
//! What you learn:
//! * `GetSaveFileNameW` — the same `OPENFILENAMEW` struct as e59's picker,
//!   three differences: the button says Save, the returned name may be a
//!   file that does NOT exist yet, and you are responsible for what
//!   happens to it
//! * `OFN_OVERWRITEPROMPT` — the dialog asks "file exists, replace?" for
//!   you (skip it and YOU must do that asking)
//! * `lpstrDefExt` — when the user types "notes" with no dot, the dialog
//!   appends ".txt" before returning (it does NOT do so when a dot was
//!   typed)
//! * The write itself: `GetWindowTextW` off the EDIT child, then plain
//!   `std::fs::write` — Win32 asks where, Rust does the work
//!
//! New Win32 APIs: `GetSaveFileNameW`, `OPENFILENAMEW.lpstrDefExt`,
//! `OFN_OVERWRITEPROMPT`.
//!
//! Run: `cargo run -p e60_save_file_dialog`

use windows::{Win32::*, core::*};

const IDC_EDIT: i32 = 1001; // single-line EDIT with the text to save
const IDC_SAVE: i32 = 1002; // "Save as..." button

#[derive(Default)]
struct State {
    hwnd_edit: HWND,
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
            lpszClassName: w!("e60_class"),
            style: (CS_HREDRAW | CS_VREDRAW) as u32,
            ..Default::default()
        };
        debug_assert!(RegisterClassW(&wc) != 0);

        let hwnd = CreateWindowExW(
            0,
            w!("e60_class"),
            w!("e60 · Save File — type text, pick a name"),
            (WS_OVERLAPPEDWINDOW | WS_VISIBLE) as u32,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            560,
            220,
            None,
            None,
            Some(instance),
            None,
        );
        debug_assert!(!hwnd.is_null());

        let state = Box::into_raw(Box::<State>::default());
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, state as isize);

        let edit = CreateWindowExW(
            WS_EX_CLIENTEDGE as usize as u32,
            w!("EDIT"),
            w!(""),
            (WS_CHILD | WS_VISIBLE) as u32,
            20,
            24,
            400,
            26,
            Some(hwnd),
            Some(IDC_EDIT as usize as HMENU),
            Some(instance),
            None,
        );
        debug_assert!(!edit.is_null());
        _ = SetWindowTextW(edit, w!("hello from the EDIT control"));
        _ = SetFocus(Some(edit));

        let save = CreateWindowExW(
            0,
            w!("BUTTON"),
            w!("Save as..."),
            (WS_CHILD | WS_VISIBLE | BS_PUSHBUTTON) as u32,
            432,
            24,
            100,
            26,
            Some(hwnd),
            Some(IDC_SAVE as usize as HMENU),
            Some(instance),
            None,
        );
        debug_assert!(!save.is_null());

        if let Some(s) = state_of(hwnd) {
            s.hwnd_edit = edit;
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

/// The e59 filter-blob recipe: label/pattern pairs, list closed by an
/// empty string (the double null).
fn filter(parts: &[&str]) -> Vec<u16> {
    let mut blob = Vec::new();
    for part in parts {
        blob.extend(part.encode_utf16());
        blob.push(0);
    }
    blob.push(0);
    blob
}

fn wide_to_string(buf: &[u16]) -> String {
    let end = buf.iter().position(|&u| u == 0).unwrap_or(buf.len());
    String::from_utf16_lossy(&buf[..end])
}

unsafe fn window_text(hwnd: HWND) -> String {
    unsafe {
        let len = GetWindowTextLengthW(hwnd);
        let mut buf = vec![0u16; len as usize + 1];
        let copied = GetWindowTextW(hwnd, PWSTR(buf.as_mut_ptr()), buf.len() as i32);
        String::from_utf16_lossy(&buf[..copied.max(0) as usize])
    }
}

/// Run the Save dialog; None = canceled.
unsafe fn pick_save_path(owner: HWND) -> Option<String> {
    unsafe {
        let filters = filter(&["Text files (*.txt)", "*.txt", "All files (*.*)", "*.*"]);
        let mut file_buf = vec![0u16; MAX_PATH as usize];

        let mut ofn = OPENFILENAMEW {
            lStructSize: std::mem::size_of::<OPENFILENAMEW>() as u32,
            hwndOwner: owner,
            lpstrFilter: PCWSTR::from_raw(filters.as_ptr()),
            lpstrFile: PWSTR(file_buf.as_mut_ptr()),
            nMaxFile: file_buf.len() as u32,
            nFilterIndex: 1,
            // OVERWRITEPROMPT: if the picked name exists, comdlg32 asks
            // "Replace?" itself. Without it you'd have to ask (and if you
            // neither ask nor check, you silently clobber files).
            Flags: (OFN_OVERWRITEPROMPT | OFN_PATHMUSTEXIST) as u32,
            // Appended when the typed name has no extension — "notes"
            // becomes "notes.txt". Must be extension-only (no dot).
            lpstrDefExt: w!("txt"),
            ..Default::default()
        };

        if GetSaveFileNameW(&mut ofn).0 == 0 {
            return None;
        }
        Some(wide_to_string(&file_buf))
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
                if notification == BN_CLICKED as u32 && control_id == IDC_SAVE as u32 {
                    if let Some(path) = pick_save_path(hwnd) {
                        // Read the EDIT's text and write it out — the
                        // whole "save" verb in two Rust lines once the
                        // dialog has done the asking.
                        let text = window_text(state.hwnd_edit);
                        let report = match std::fs::write(&path, text.as_bytes()) {
                            Ok(()) => format!("e60 · saved {} bytes — {path}", text.len()),
                            Err(err) => format!("e60 · write failed — {err}"),
                        };
                        _ = SetWindowTextW(hwnd, &HSTRING::from(report));
                    } else {
                        _ = SetWindowTextW(hwnd, w!("e60 · canceled"));
                    }
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
