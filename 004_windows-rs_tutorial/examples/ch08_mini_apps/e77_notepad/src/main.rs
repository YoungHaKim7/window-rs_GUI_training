//! e77 — Mini Notepad: menus, an EDIT that fills the window, dirty flag
//!
//! What you learn (app-sized recap of chapters 4–6):
//! * A menu bar built in code (`CreateMenu`/`CreatePopupMenu` +
//!   `AppendMenuW`, e33) whose ids drive one WM_COMMAND handler
//! * **A control that IS the window**: one multiline EDIT (e44) resized
//!   to the full client area on every WM_SIZE — the "document view"
//!   pattern every editor uses. The EDIT does scrolling, caret, undo,
//!   selection; we did not write a line of it.
//! * The dirty-flag discipline: `EN_CHANGE` (posted by the EDIT whenever
//!   its text changes) sets the flag, the title bar grows a `*`, and both
//!   exit paths (menu Exit AND the [X] box, via WM_CLOSE) run the same
//!   Yes/No/Cancel confirm: Yes = save first (Save As), No = discard,
//!   Cancel = keep editing. Program-state thinking, not just control
//!   plumbing.
//! * Open/Save with `GetOpenFileNameW`/`GetSaveFileNameW` (e59/e60) and
//!   the double-null filter blob — then plain `std::fs` for the bytes.
//!   Note we SET the text and flag ourselves; the dialog only picks a
//!   path, it never touches the file.
//!
//! Run: `cargo run -p e77_notepad`

use windows::{Win32::*, core::*};

const IDC_EDIT: i32 = 1001;

const IDM_NEW: usize = 4001;
const IDM_OPEN: usize = 4002;
const IDM_SAVE_AS: usize = 4003;
const IDM_EXIT: usize = 4004;
const IDM_ABOUT: usize = 4005;

struct State {
    dirty: bool,
    /// Last saved-to / opened-from file name (display only; Save As
    /// always asks — a full "Save" menu item is a two-line addition).
    file_name: String,
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
            lpszClassName: w!("e77_class"),
            style: (CS_HREDRAW | CS_VREDRAW) as u32,
            ..Default::default()
        };
        debug_assert!(RegisterClassW(&wc) != 0);

        let state = Box::into_raw(Box::new(State {
            dirty: false,
            file_name: String::from("Untitled"),
        }));

        let hwnd = CreateWindowExW(
            0,
            w!("e77_class"),
            w!("e77 · Notepad"),
            (WS_OVERLAPPEDWINDOW | WS_VISIBLE) as u32,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            640,
            460,
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

/// Read a null-terminated wide buffer into a String.
fn wide_to_string(buf: &[u16]) -> String {
    let end = buf.iter().position(|&u| u == 0).unwrap_or(buf.len());
    String::from_utf16_lossy(&buf[..end])
}

/// The e59 double-null filter blob: label, pattern, label, pattern, "".
fn filter(parts: &[&str]) -> Vec<u16> {
    let mut blob = Vec::new();
    for part in parts {
        blob.extend(part.encode_utf16());
        blob.push(0);
    }
    blob.push(0); // end-of-list empty string
    blob
}

/// Shared picker for Open and Save As: `save` swaps the function and the
/// honesty flags. Returns None on cancel.
unsafe fn pick_path(owner: HWND, save: bool) -> Option<String> {
    unsafe {
        let filters = filter(&[
            "Text files (*.txt;*.md)",
            "*.txt;*.md",
            "All files (*.*)",
            "*.*",
        ]);
        let mut file_buf = vec![0u16; 260];

        let mut ofn = OPENFILENAMEW {
            lStructSize: std::mem::size_of::<OPENFILENAMEW>() as u32,
            hwndOwner: owner,
            lpstrFilter: PCWSTR::from_raw(filters.as_ptr()),
            lpstrFile: PWSTR(file_buf.as_mut_ptr()),
            nMaxFile: file_buf.len() as u32,
            nFilterIndex: 1,
            // Open forbids invented paths; Save asks before overwriting.
            Flags: if save {
                (OFN_OVERWRITEPROMPT) as u32
            } else {
                (OFN_PATHMUSTEXIST | OFN_FILEMUSTEXIST) as u32
            },
            ..Default::default()
        };

        let ok = if save {
            GetSaveFileNameW(&mut ofn).0 != 0
        } else {
            GetOpenFileNameW(&mut ofn).0 != 0
        };
        if ok {
            Some(wide_to_string(&file_buf))
        } else {
            None
        }
    }
}

/// Pull the whole EDIT content out as a Rust String (GETLINE loop is the
/// zero-copy way; GETTEXTLENGTH+WM_GETTEXT is the simple one — this is a
/// tutorial, and the simple one is honest about being O(n) once per save).
unsafe fn get_edit_text(edit: HWND) -> String {
    unsafe {
        let len = SendMessageW(edit, WM_GETTEXTLENGTH as u32, 0, 0) as usize;
        let mut buf = vec![0u16; len + 1];
        SendMessageW(
            edit,
            WM_GETTEXT as u32,
            buf.len() as WPARAM,
            buf.as_mut_ptr() as LPARAM,
        );
        wide_to_string(&buf)
    }
}

unsafe fn set_edit_text(edit: HWND, text: &str) {
    unsafe {
        _ = SetWindowTextW(edit, &HSTRING::from(text));
    }
}

/// Title reflects document state: "name [*] — e77 Notepad".
unsafe fn update_title(hwnd: HWND) {
    unsafe {
        if let Some(state) = state_of(hwnd) {
            let star = if state.dirty { " *" } else { "" };
            _ = SetWindowTextW(
                hwnd,
                &HSTRING::from(format!("{}{} — e77 Notepad", state.file_name, star)),
            );
        }
    }
}

/// The dirty-discard gate. Returns true when the caller may proceed
/// (text replaced / window closed); false means the user canceled.
/// Yes = save first (Save As; canceling THAT cancels too), No = discard,
/// Cancel = abort whatever triggered the question.
unsafe fn confirm_discard(hwnd: HWND) -> bool {
    unsafe {
        let choice = MessageBoxW(
            Some(hwnd),
            w!("The text has changed.\n\nSave it first?"),
            w!("e77 Notepad"),
            (MB_YESNOCANCEL | MB_ICONQUESTION) as u32,
        );
        match choice {
            IDYES => {
                // Save As; if the user cancels the picker, the whole
                // action is off.
                match pick_path(hwnd, true) {
                    Some(path) => {
                        let edit = GetDlgItem(Some(hwnd), IDC_EDIT);
                        let _ = std::fs::write(&path, get_edit_text(edit));
                        if let Some(state) = state_of(hwnd) {
                            state.dirty = false;
                            state.file_name = path;
                        }
                        update_title(hwnd);
                        true
                    }
                    None => false,
                }
            }
            IDNO => true, // discard — proceed
            _ => false,   // IDCANCEL or closed box
        }
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
                let cs = lparam as *const CREATESTRUCTW;
                let instance = (*cs).hInstance;

                // The menu bar (e33 recap). MNEMONIC: the & marks the
                // Alt-key letter; \t in items would add shortcut hints
                // (none here — accelerators arrive with e82).
                let bar = CreateMenu();
                let file_menu = CreatePopupMenu();
                _ = AppendMenuW(file_menu, MF_STRING as u32, IDM_NEW, w!("&New"));
                _ = AppendMenuW(file_menu, MF_STRING as u32, IDM_OPEN, w!("&Open..."));
                _ = AppendMenuW(file_menu, MF_STRING as u32, IDM_SAVE_AS, w!("Save &As..."));
                _ = AppendMenuW(file_menu, MF_SEPARATOR as u32, 0, w!(""));
                _ = AppendMenuW(file_menu, MF_STRING as u32, IDM_EXIT, w!("E&xit"));
                _ = AppendMenuW(bar, MF_POPUP as u32, file_menu as usize, w!("&File"));

                let help_menu = CreatePopupMenu();
                _ = AppendMenuW(
                    help_menu,
                    MF_STRING as u32,
                    IDM_ABOUT,
                    w!("&About Notepad..."),
                );
                _ = AppendMenuW(bar, MF_POPUP as u32, help_menu as usize, w!("&Help"));
                _ = SetMenu(hwnd, Some(bar));

                // The whole "document": one EDIT. ES_WANTRETURN not needed
                // (no dialog), but ES_AUTOHSCROLL + WS_HSCROLL give the
                // long-line behavior people expect from Notepad.
                let edit = CreateWindowExW(
                    WS_EX_CLIENTEDGE as u32,
                    w!("EDIT"),
                    w!(""),
                    (WS_CHILD
                        | WS_VISIBLE
                        | WS_VSCROLL
                        | WS_HSCROLL
                        | ES_MULTILINE
                        | ES_AUTOVSCROLL
                        | ES_AUTOHSCROLL) as u32,
                    0,
                    0,
                    0,
                    0, // WM_SIZE below owns the geometry
                    Some(hwnd),
                    Some(IDC_EDIT as usize as HMENU),
                    Some(instance),
                    None,
                );
                debug_assert!(!edit.is_null());

                let mut rc = RECT::default();
                _ = GetClientRect(hwnd, &mut rc);
                _ = MoveWindow(edit, 0, 0, rc.right, rc.bottom, true);
                _ = SetFocus(Some(edit));
                0
            }

            WM_SIZE => {
                let edit = GetDlgItem(Some(hwnd), IDC_EDIT);
                if !edit.is_null() {
                    let w = (lparam & 0xffff) as i16 as i32;
                    let h = ((lparam >> 16) & 0xffff) as i16 as i32;
                    _ = MoveWindow(edit, 0, 0, w, h, true);
                }
                0
            }

            WM_COMMAND => {
                let notification = ((wparam >> 16) & 0xffff) as u32;
                let id = (wparam & 0xffff) as usize;

                // EN_CHANGE: the EDIT text changed under the user (and
                // under us: SetWindowTextW triggers it too — note how
                // load/open below clears the flag AFTER setting text).
                if notification == EN_CHANGE as u32 && id == IDC_EDIT as usize {
                    if let Some(state) = state_of(hwnd) {
                        if !state.dirty {
                            state.dirty = true;
                            update_title(hwnd);
                        }
                    }
                    return 0;
                }

                match id {
                    IDM_NEW => {
                        // Borrow discipline (e65): confirm_discard runs
                        // nested message loops, so no &mut State may be
                        // alive across it — copy the flag out, drop the
                        // borrow, THEN ask.
                        let dirty_now = state_of(hwnd).map(|s| s.dirty).unwrap_or(false);
                        if !dirty_now || confirm_discard(hwnd) {
                            let edit = GetDlgItem(Some(hwnd), IDC_EDIT);
                            set_edit_text(edit, "");
                            if let Some(state) = state_of(hwnd) {
                                state.dirty = false;
                                state.file_name = String::from("Untitled");
                            }
                            update_title(hwnd);
                        }
                        0
                    }

                    IDM_OPEN => {
                        let dirty_now = state_of(hwnd).map(|s| s.dirty).unwrap_or(false);
                        if !dirty_now || confirm_discard(hwnd) {
                            if let Some(path) = pick_path(hwnd, false) {
                                match std::fs::read(&path) {
                                    Ok(bytes) => {
                                        let text = String::from_utf8_lossy(&bytes).into_owned();
                                        let edit = GetDlgItem(Some(hwnd), IDC_EDIT);
                                        set_edit_text(edit, &text);
                                        if let Some(state) = state_of(hwnd) {
                                            state.dirty = false;
                                            state.file_name = path;
                                        }
                                        update_title(hwnd);
                                    }
                                    Err(err) => {
                                        _ = MessageBoxW(
                                            Some(hwnd),
                                            &HSTRING::from(format!(
                                                "Could not read the file:\n{err}"
                                            )),
                                            w!("e77 Notepad"),
                                            (MB_OK | MB_ICONERROR) as u32,
                                        );
                                    }
                                }
                            }
                        }
                        0
                    }

                    IDM_SAVE_AS => {
                        if let Some(path) = pick_path(hwnd, true) {
                            let edit = GetDlgItem(Some(hwnd), IDC_EDIT);
                            match std::fs::write(&path, get_edit_text(edit)) {
                                Ok(()) => {
                                    if let Some(state) = state_of(hwnd) {
                                        state.dirty = false;
                                        state.file_name = path;
                                    }
                                    update_title(hwnd);
                                }
                                Err(err) => {
                                    _ = MessageBoxW(
                                        Some(hwnd),
                                        &HSTRING::from(format!("Could not write the file:\n{err}")),
                                        w!("e77 Notepad"),
                                        (MB_OK | MB_ICONERROR) as u32,
                                    );
                                }
                            }
                        }
                        0
                    }

                    IDM_EXIT => {
                        // Route through WM_CLOSE so the [X] box and the
                        // menu share the one confirm gate below.
                        _ = PostMessageW(Some(hwnd), WM_CLOSE as u32, 0, 0);
                        0
                    }

                    IDM_ABOUT => {
                        _ = MessageBoxW(
                            Some(hwnd),
                            w!("e77 Notepad\n\n\
                                A menu bar, one multiline EDIT, a dirty flag,\n\
                                and the two comdlg32 pickers — chapters 4-6\n\
                                cashed in as a real (tiny) application."),
                            w!("About e77 Notepad"),
                            (MB_OK | MB_ICONINFORMATION) as u32,
                        );
                        0
                    }

                    _ => DefWindowProcW(hwnd, message, wparam, lparam),
                }
            }

            WM_CLOSE => {
                // The single gate for EVERY close path: menu Exit (posted
                // WM_CLOSE above) and the [X] button land here identically.
                let dirty = state_of(hwnd).map(|s| s.dirty).unwrap_or(false);
                if !dirty || confirm_discard(hwnd) {
                    _ = DestroyWindow(hwnd);
                }
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
