//! e80 — Todo List, Basic: EDIT + LISTBOX + the mirror-array pattern
//!
//! What you learn:
//! * The **mirror array**: the LISTBOX holds strings for display, but
//!   the app's truth lives in a `Vec<String>` in State. Every mutation
//!   updates BOTH, in the same order — the listbox is treated as a
//!   dumb view, never as storage. (Why not read strings back from the
//!   control? You can — but keeping one authoritative model and one
//!   projection is the habit that survives contact with real apps,
//!   where rows gain ids, done-flags, and data no control can hold.)
//! * The listbox message set (e48 recap): `LB_ADDSTRING`,
//!   `LB_GETCURSEL`, `LB_DELETESTRING` — plus `LBN_SELCHANGE`
//!   (requires the `LBS_NOTIFY` style) as the "selection moved"
//!   notification.
//! * `EnableWindow` as UI honesty: the Delete button is grayed whenever
//!   nothing is selected, so the user can never click a dead action —
//!   the cheapest form of input validation there is.
//!
//! Type a task, press Add (or Enter), select rows, Delete.
//!
//! Run: `cargo run -p e80_todo_basic`

use windows::{Win32::*, core::*};

const IDC_ENTRY: i32 = 1001; // input EDIT
const IDC_ADD: i32 = 1002; // Add button
const IDC_LIST: i32 = 1003; // LISTBOX
const IDC_DELETE: i32 = 1004; // Delete button

#[derive(Default)]
struct State {
    /// The mirror of the listbox's contents — same order, same length.
    items: Vec<String>,
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
            lpszClassName: w!("e80_class"),
            style: (CS_HREDRAW | CS_VREDRAW) as u32,
            ..Default::default()
        };
        debug_assert!(RegisterClassW(&wc) != 0);

        let state = Box::into_raw(Box::<State>::default());

        let hwnd = CreateWindowExW(
            0,
            w!("e80_class"),
            w!("e80 · Todo (basic) — add & delete"),
            (WS_OVERLAPPEDWINDOW | WS_VISIBLE) as u32,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            460,
            420,
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

fn to_wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

fn wide_to_string(buf: &[u16]) -> String {
    let end = buf.iter().position(|&u| u == 0).unwrap_or(buf.len());
    String::from_utf16_lossy(&buf[..end])
}

/// Read the entry EDIT's text and clear it. Returns "" when empty.
unsafe fn take_entry(hwnd: HWND) -> String {
    unsafe {
        let entry = GetDlgItem(Some(hwnd), IDC_ENTRY);
        let len = SendMessageW(entry, WM_GETTEXTLENGTH as u32, 0, 0) as usize;
        let mut buf = vec![0u16; len + 1];
        SendMessageW(
            entry,
            WM_GETTEXT as u32,
            buf.len() as WPARAM,
            buf.as_mut_ptr() as LPARAM,
        );
        _ = SetWindowTextW(entry, w!(""));
        wide_to_string(&buf)
    }
}

/// Gray/ungray Delete to match "is a row selected?" — called after
/// every event that can change the answer.
unsafe fn sync_delete_button(hwnd: HWND) {
    unsafe {
        let list = GetDlgItem(Some(hwnd), IDC_LIST);
        let selected = SendMessageW(list, LB_GETCURSEL as u32, 0, 0) as i32;
        let delete = GetDlgItem(Some(hwnd), IDC_DELETE);
        if !delete.is_null() {
            _ = EnableWindow(delete, selected >= 0);
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

                let entry = CreateWindowExW(
                    WS_EX_CLIENTEDGE as u32,
                    w!("EDIT"),
                    w!(""),
                    (WS_CHILD | WS_VISIBLE | ES_AUTOHSCROLL) as u32,
                    12,
                    14,
                    300,
                    26,
                    Some(hwnd),
                    Some(IDC_ENTRY as usize as HMENU),
                    Some(instance),
                    None,
                );
                debug_assert!(!entry.is_null());

                let add = CreateWindowExW(
                    0,
                    w!("BUTTON"),
                    w!("Add"),
                    (WS_CHILD | WS_VISIBLE | BS_DEFPUSHBUTTON) as u32,
                    322,
                    14,
                    80,
                    26, // BS_DEFPUSHBUTTON: Enter in the EDIT fires this
                    Some(hwnd),
                    Some(IDC_ADD as usize as HMENU),
                    Some(instance),
                    None,
                );
                debug_assert!(!add.is_null());

                // LBS_NOTIFY: without it the listbox stays SILENT — no
                // LBN_* notifications ever reach us.
                let list = CreateWindowExW(
                    WS_EX_CLIENTEDGE as u32,
                    w!("LISTBOX"),
                    w!(""),
                    (WS_CHILD | WS_VISIBLE | WS_VSCROLL | LBS_NOTIFY) as u32,
                    12,
                    52,
                    390,
                    280,
                    Some(hwnd),
                    Some(IDC_LIST as usize as HMENU),
                    Some(instance),
                    None,
                );
                debug_assert!(!list.is_null());

                let delete = CreateWindowExW(
                    0,
                    w!("BUTTON"),
                    w!("Delete selected"),
                    (WS_CHILD | WS_VISIBLE) as u32,
                    12,
                    344,
                    140,
                    28,
                    Some(hwnd),
                    Some(IDC_DELETE as usize as HMENU),
                    Some(instance),
                    None,
                );
                debug_assert!(!delete.is_null());

                // Seed a few rows so the app opens alive.
                for seed in ["buy milk", "ship e80", "profit"] {
                    if let Some(state) = state_of(hwnd) {
                        state.items.push(String::from(seed));
                    }
                    let wide = to_wide(seed);
                    SendMessageW(
                        GetDlgItem(Some(hwnd), IDC_LIST),
                        LB_ADDSTRING as u32,
                        0,
                        wide.as_ptr() as LPARAM,
                    );
                }
                sync_delete_button(hwnd);
                _ = SetFocus(Some(entry));
                0
            }

            WM_COMMAND => {
                let notification = ((wparam >> 16) & 0xffff) as i32;
                let id = (wparam & 0xffff) as i32;
                let from = lparam as HWND;

                match (id, notification) {
                    // Add: pull the entry text, mirror-append, clear.
                    // The double bookkeeping (Vec AND control) IS the
                    // pattern this example teaches.
                    (IDC_ADD, n) if n == BN_CLICKED => {
                        let text = take_entry(hwnd).trim().to_string();
                        if !text.is_empty() {
                            if let Some(state) = state_of(hwnd) {
                                state.items.push(text.clone());
                            }
                            let list = GetDlgItem(Some(hwnd), IDC_LIST);
                            let wide = to_wide(&text);
                            SendMessageW(list, LB_ADDSTRING as u32, 0, wide.as_ptr() as LPARAM);
                        }
                        _ = SetFocus(Some(GetDlgItem(Some(hwnd), IDC_ENTRY)));
                        0
                    }

                    // Selection moved → maybe re-enable Delete.
                    (IDC_LIST, n) if n == LBN_SELCHANGE => {
                        sync_delete_button(hwnd);
                        0
                    }

                    (IDC_DELETE, n) if n == BN_CLICKED => {
                        let list = GetDlgItem(Some(hwnd), IDC_LIST);
                        let sel = SendMessageW(list, LB_GETCURSEL as u32, 0, 0) as i32;
                        if sel >= 0 {
                            // Same index, both stores — the mirror rule.
                            SendMessageW(list, LB_DELETESTRING as u32, sel as WPARAM, 0);
                            if let Some(state) = state_of(hwnd) {
                                if (sel as usize) < state.items.len() {
                                    state.items.remove(sel as usize);
                                }
                            }
                            // After a deletion there is NO selection (and
                            // no LBN_SELCHANGE fires for that!) — sync by
                            // hand, or Delete would stay clickable.
                            sync_delete_button(hwnd);
                        }
                        0
                    }

                    _ => {
                        let _ = from;
                        DefWindowProcW(hwnd, message, wparam, lparam)
                    }
                }
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
