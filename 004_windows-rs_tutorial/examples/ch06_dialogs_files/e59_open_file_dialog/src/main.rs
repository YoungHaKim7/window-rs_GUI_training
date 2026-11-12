//! e59 — Open File Dialog: GetOpenFileNameW and the double-null filter
//!
//! What you learn:
//! * `GetOpenFileNameW` — the classic (pre-Vista, still everywhere) file
//!   picker: fill one `OPENFILENAMEW` struct, call, read `lpstrFile`
//! * **The double-null-terminated filter format**: `lpstrFilter` points at
//!   a sequence of null-terminated wide strings in PAIRS — display label,
//!   then pattern — with the whole list ended by ONE MORE null (i.e. an
//!   empty final string). Text files then all files looks like
//!   `"Text\0*.txt\0All\0*.*\0\0"`. A single trailing zero (Rust's
//!   to_wide!) is NOT enough — this is the #1 crash here.
//! * `lpstrFile` is a buffer YOU allocate (`MAX_PATH` u16s) and the dialog
//!   fills in; `nMaxFile` tells it the size — forget it and get truncation
//! * The OFN_ flag vocabulary: `OFN_PATHMUSTEXIST` + `OFN_FILEMUSTEXIST`
//!   turn the "File name" box honest (no typing inventions)
//!
//! Pick a file: its full path lands in the title bar, and std::fs reports
//! its size — Win32 for the dialog, plain Rust for the file.
//!
//! New Win32 APIs: `GetOpenFileNameW`, `OPENFILENAMEW`, `MAX_PATH`.
//!
//! Run: `cargo run -p e59_open_file_dialog`

use windows::{Win32::*, core::*};

/// Main-window button that opens the picker.
const IDC_OPEN: i32 = 1001;

fn main() {
    unsafe {
        let instance = GetModuleHandleW(PCWSTR::null());
        debug_assert!(!instance.is_null());

        let wc = WNDCLASSW {
            lpfnWndProc: Some(wndproc),
            hInstance: instance,
            hCursor: LoadCursorW(None, PCWSTR(IDC_ARROW.0.cast())),
            hbrBackground: ((COLOR_WINDOW + 1) as isize) as HBRUSH,
            lpszClassName: w!("e59_class"),
            style: (CS_HREDRAW | CS_VREDRAW) as u32,
            ..Default::default()
        };
        debug_assert!(RegisterClassW(&wc) != 0);

        let hwnd = CreateWindowExW(
            0,
            w!("e59_class"),
            w!("e59 · Open File — pick anything on disk"),
            (WS_OVERLAPPEDWINDOW | WS_VISIBLE) as u32,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            520,
            240,
            None,
            None,
            Some(instance),
            None,
        );
        debug_assert!(!hwnd.is_null());

        let open = CreateWindowExW(
            0,
            w!("BUTTON"),
            w!("Open..."),
            (WS_CHILD | WS_VISIBLE | BS_PUSHBUTTON) as u32,
            20,
            30,
            110,
            32,
            Some(hwnd),
            Some(IDC_OPEN as usize as HMENU),
            Some(instance),
            None,
        );
        debug_assert!(!open.is_null());

        let mut message = MSG::default();
        while GetMessageW(&mut message, None, 0, 0).0 > 0 {
            _ = TranslateMessage(&message);
            DispatchMessageW(&message);
        }
    }
}

/// Build the filter blob: every string null-terminated, then one extra
/// empty string terminates the whole list (the DOUBLE null).
///
///    "Text files\0*.txt\0All files\0*.*\0\0"
///     └─ label ──┘└pattern┘ ...          └┴─ end of list
///
/// The dialog shows the labels in the dropdown; the patterns do the
/// filtering. `nFilterIndex` (1-based) picks the initially shown pair.
fn filter(parts: &[&str]) -> Vec<u16> {
    let mut blob = Vec::new();
    for part in parts {
        blob.extend(part.encode_utf16());
        blob.push(0); // terminator of THIS string
    }
    blob.push(0); // terminator of the LIST: an empty string at the end
    blob
}

/// Read a null-terminated wide buffer into a String.
fn wide_to_string(buf: &[u16]) -> String {
    let end = buf.iter().position(|&u| u == 0).unwrap_or(buf.len());
    String::from_utf16_lossy(&buf[..end])
}

/// Run the picker; returns None if the user canceled.
unsafe fn pick_file(owner: HWND) -> Option<String> {
    unsafe {
        // The filter pairs for the dropdown.
        let filters = filter(&[
            "Text files (*.txt;*.md)",
            "*.txt;*.md",
            "All files (*.*)",
            "*.*",
        ]);
        // The result buffer: MAX_PATH wide chars, zeroed. (Single-select
        // fills one path; OFN_ALLOWMULTISELECT would fill
        // "dir\0file1\0file2\0\0" and needs a much larger buffer.)
        let mut file_buf = vec![0u16; MAX_PATH as usize];

        let mut ofn = OPENFILENAMEW {
            // lStructSize: the classic ABI versioning handshake — comdlg32
            // reads this to know which OPENFILENAMEW vintage you are.
            lStructSize: std::mem::size_of::<OPENFILENAMEW>() as u32,
            hwndOwner: owner, // modal against our window
            lpstrFilter: PCWSTR::from_raw(filters.as_ptr()),
            lpstrFile: PWSTR(file_buf.as_mut_ptr()),
            nMaxFile: file_buf.len() as u32,
            nFilterIndex: 1, // open with the first filter pair active
            Flags: (OFN_PATHMUSTEXIST | OFN_FILEMUSTEXIST) as u32,
            ..Default::default()
        };

        // Returns 0 on cancel OR failure (CommDlgExtendedError tells them
        // apart; for a tutorial, "no file" covers both).
        if GetOpenFileNameW(&mut ofn).0 == 0 {
            return None;
        }
        Some(wide_to_string(&file_buf))
    }
}

extern "system" fn wndproc(hwnd: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe {
        match message as i32 {
            WM_COMMAND => {
                let notification = ((wparam >> 16) & 0xffff) as u32;
                let control_id = (wparam & 0xffff) as u32;
                if notification == BN_CLICKED as u32 && control_id == IDC_OPEN as u32 {
                    match pick_file(hwnd) {
                        Some(path) => {
                            // The Win32 side is done; plain Rust takes over.
                            // metadata() follows the path (relative to the
                            // process CWD if relative — the picker returns
                            // absolute paths).
                            let text = match std::fs::metadata(&path) {
                                Ok(meta) => {
                                    format!("e59 · {path} — {} bytes", meta.len())
                                }
                                Err(err) => format!("e59 · {path} — {err}"),
                            };
                            _ = SetWindowTextW(hwnd, &HSTRING::from(text));
                        }
                        None => {
                            _ = SetWindowTextW(hwnd, w!("e59 · canceled"));
                        }
                    }
                }
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
