//! e64 — File I/O with Text: a mini "notepad zero"
//!
//! What you learn:
//! * The three pieces of every editor glued together: an **Open** dialog
//!   (`GetOpenFileNameW`, e59) + `std::fs::read_to_string` → a multiline
//!   EDIT; a **Save as** dialog (`GetSaveFileNameW`, e60) +
//!   `GetWindowTextW` → `std::fs::write`
//! * **Laying out children on WM_SIZE**: buttons stay at the top, the
//!   status STATIC hugs the bottom, the EDIT stretches to everything
//!   between — via `MoveWindow` whenever the parent resizes
//! * A status line that reports path + byte count after every operation
//!
//! What's deliberately missing (the full notepad is e77): **dirty
//! tracking** — the EDIT sends `EN_CHANGE` (in `WM_COMMAND`, LOWORD = the
//! edit's id, HIWORD = 768) after every edit; a real editor sets a dirty
//! flag there, warns on close-with-changes, and puts "*" in the title.
//! We leave that as the commented hook below.
//!
//! New Win32 APIs: `ES_MULTILINE`, `MoveWindow`, STATIC as a status line,
//! `EN_CHANGE` (documented, not handled).
//!
//! Run: `cargo run -p e64_file_io_text`

use windows::{Win32::*, core::*;

const IDC_OPEN: i32 = 1001; // "Open..." button
const IDC_SAVE: i32 = 1002; // "Save as..." button
const IDC_EDIT: i32 = 1003; // the document EDIT
const IDC_STATUS: i32 = 1004; // the bottom status STATIC

const TOP: i32 = 42; // pixels reserved for the button row
const BOTTOM: i32 = 26; // pixels reserved for the status line

#[derive(Default)]
struct State {
    hwnd_edit: HWND,
    hwnd_status: HWND,
    path: String, // "" = untitled/never saved
    bytes: usize, // size of what was last loaded/saved
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
            lpszClassName: w!("e64_class"),
            style: (CS_HREDRAW | CS_VREDRAW) as u32,
            ..Default::default()
        };
        debug_assert!(RegisterClassW(&wc) != 0);

        let hwnd = CreateWindowExW(
            0,
            w!("e64_class"),
            w!("e64 · File I/O — notepad, chapter one"),
            (WS_OVERLAPPEDWINDOW | WS_VISIBLE) as u32,
            CW_USEDEFAULT, CW_USEDEFAULT, 620, 440,
            None, None, Some(instance), None,
        );
        debug_assert!(!hwnd.is_null());

        let state = Box::into_raw(Box::<State>::default());
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, state as isize);

        // ---- Button row ------------------------------------------------------
        let open = CreateWindowExW(
            0,
            w!("BUTTON"),
            w!("Open..."),
            (WS_CHILD | WS_VISIBLE | BS_PUSHBUTTON) as u32,
            8, 6, 90, 28,
            Some(hwnd), Some(IDC_OPEN as usize as HMENU), Some(instance), None,
        );
        debug_assert!(!open.is_null());
        let save = CreateWindowExW(
            0,
            w!("BUTTON"),
            w!("Save as..."),
            (WS_CHILD | WS_VISIBLE | BS_PUSHBUTTON) as u32,
            104, 6, 100, 28,
            Some(hwnd), Some(IDC_SAVE as usize as HMENU), Some(instance), None,
        );
        debug_assert!(!save.is_null());

        // ---- The document: multiline EDIT -------------------------------------
        // ES_MULTILINE is what makes an EDIT wrap into lines;
        // ES_AUTOVSCROLL keeps typing scrolled to the caret; WS_VSCROLL
        // adds the elevator. WS_HSCROLL + ES_AUTOHSCROLL would be the
        // no-wrap configuration (notepad's word-wrap toggle is exactly
        // these two style bits, flipped at runtime like e43's password).
        let edit = CreateWindowExW(
            WS_EX_CLIENTEDGE as usize as u32,
            w!("EDIT"),
            w!(""),
            (WS_CHILD | WS_VISIBLE | ES_MULTILINE | ES_AUTOVSCROLL | WS_VSCROLL) as u32,
            0, TOP, 600, 360, // placeholder size; WM_SIZE fixes it for real
            Some(hwnd), Some(IDC_EDIT as usize as HMENU), Some(instance), None,
        );
        debug_assert!(!edit.is_null());

        // ---- The status line: just a STATIC ----------------------------------
        let status = CreateWindowExW(
            0,
            w!("STATIC"),
            w!("untitled — open or type, then save"),
            (WS_CHILD | WS_VISIBLE | SS_LEFT) as u32,
            0, 0, 200, BOTTOM,
            Some(hwnd), Some(IDC_STATUS as usize as HMENU), Some(instance), None,
        );
        debug_assert!(!status.is_null());

        if let Some(s) = state_of(hwnd) {
            s.hwnd_edit = edit;
            s.hwnd_status = status;
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

unsafe fn window_text(hwnd: HWND) -> String {
    unsafe {
        let len = GetWindowTextLengthW(hwnd);
        let mut buf = vec![0u16; len as usize + 1];
        let copied = GetWindowTextW(hwnd, PWSTR(buf.as_mut_ptr()), buf.len() as i32);
        String::from_utf16_lossy(&buf[..copied.max(0) as usize])
    }
}

fn wide_to_string(buf: &[u16]) -> String {
    let end = buf.iter().position(|&u| u == 0).unwrap_or(buf.len());
    String::from_utf16_lossy(&buf[..end])
}

/// e59's filter blob recipe (label/pattern pairs + the empty final string).
fn filter(parts: &[&str]) -> Vec<u16> {
    let mut blob = Vec::new();
    for part in parts {
        blob.extend(part.encode_utf16());
        blob.push(0);
    }
    blob.push(0);
    blob
}

/// Shared dialog plumbing for open and save. `save` picks the function.
unsafe fn file_dialog(owner: HWND, save: bool) -> Option<String> {
    unsafe {
        let filters = filter(&["Text files (*.txt;*.md)", "*.txt;*.md", "All files (*.*)", "*.*"]);
        let mut file_buf = vec![0u16; MAX_PATH as usize];
        let mut ofn = OPENFILENAMEW {
            lStructSize: std::mem::size_of::<OPENFILENAMEW>() as u32,
            hwndOwner: owner,
            lpstrFilter: PCWSTR::from_raw(filters.as_ptr()),
            lpstrFile: PWSTR(file_buf.as_mut_ptr()),
            nMaxFile: file_buf.len() as u32,
            nFilterIndex: 1,
            Flags: if save {
                (OFN_OVERWRITEPROMPT | OFN_PATHMUSTEXIST) as u32
            } else {
                (OFN_PATHMUSTEXIST | OFN_FILEMUSTEXIST) as u32
            },
            lpstrDefExt: w!("txt"),
            ..Default::default()
        };
        let ok = if save {
            GetSaveFileNameW(&mut ofn)
        } else {
            GetOpenFileNameW(&mut ofn)
        };
        if ok.0 == 0 {
            None
        } else {
            Some(wide_to_string(&file_buf))
        }
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

                // ---- Dirty-tracking hook (comments only; e77 does it) -----
                // The EDIT sends EN_CHANGE (768) after EVERY user edit.
                // A real editor would do:
                //     if control_id == IDC_EDIT as u32 && notification == EN_CHANGE as u32
                //         { state.dirty = true; retitle with '*'; }
                // We skip it here so closing never blocks on "save?".

                if notification == BN_CLICKED as u32 && control_id == IDC_OPEN as u32 {
                    if let Some(path) = file_dialog(hwnd, false) {
                        // Read the whole file as UTF-8. A non-UTF-8 file
                        // errors here — a real editor would offer an
                        // encoding fallback; we report and keep the old
                        // contents.
                        match std::fs::read_to_string(&path) {
                            Ok(text) => {
                                state.bytes = text.len();
                                state.path = path;
                                _ = SetWindowTextW(state.hwnd_edit, &HSTRING::from(text));
                            }
                            Err(err) => {
                                _ = MessageBoxW(
                                    Some(hwnd),
                                    &HSTRING::from(format!("open failed — {err}")),
                                    w!("Open"),
                                    (MB_OK | MB_ICONWARNING) as u32,
                                );
                            }
                        }
                    }
                }
                if notification == BN_CLICKED as u32 && control_id == IDC_SAVE as u32 {
                    if let Some(path) = file_dialog(hwnd, true) {
                        // The document's truth lives in the control —
                        // pull it out and write it, byte for byte.
                        let text = window_text(state.hwnd_edit);
                        match std::fs::write(&path, text.as_bytes()) {
                            Ok(()) => {
                                state.bytes = text.len();
                                state.path = path;
                            }
                            Err(err) => {
                                _ = MessageBoxW(
                                    Some(hwnd),
                                    &HSTRING::from(format!("save failed — {err}")),
                                    w!("Save"),
                                    (MB_OK | MB_ICONWARNING) as u32,
                                );
                            }
                        }
                    }
                }

                // Status line: path + size of the last load/save. The
                // current byte count would need GetWindowTextLengthW —
                // call it live so typing shows up immediately.
                let live = GetWindowTextLengthW(state.hwnd_edit) as usize;
                _ = SetWindowTextW(
                    state.hwnd_status,
                    &HSTRING::from(format!(
                        "{} — {} bytes on disk, {} in the edit",
                        if state.path.is_empty() { "untitled" } else { &state.path },
                        state.bytes,
                        live
                    )),
                );
                0
            }
            WM_SIZE => {
                // Layout on resize: buttons are fixed at the top; the
                // EDIT eats everything in the middle; the STATIC pins to
                // the bottom. MoveWindow(x, y, w, h, repaint) — the
                // children's coordinates are parent-client pixels.
                let w = (lparam & 0xffff) as u16 as i32;
                let h = ((lparam >> 16) & 0xffff) as u16 as i32;
                _ = MoveWindow(state.hwnd_edit, 0, TOP, w, (h - TOP - BOTTOM).max(0), true);
                _ = MoveWindow(state.hwnd_status, 0, h - BOTTOM, w, BOTTOM, true);
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
