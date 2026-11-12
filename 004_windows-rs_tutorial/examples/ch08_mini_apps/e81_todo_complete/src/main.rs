//! e81 — Todo List, Complete: done-flags, a status bar, filtered views
//!
//! What you learn:
//! * Rows become data, not just strings: `Vec<(String, bool)>` is the
//!   model; the listbox shows a PROJECTION — "[ ] text" / "[x] text" —
//!   regenerated from state after every change. Controls as views,
//!   state as truth (e80's mirror pattern, one level up).
//! * **Filtering with an index map**: when the View menu shows "only
//!   open", the listbox rows no longer line up with state indices. The
//!   fix is `shown: Vec<usize>` — "row r displays state[shown[r]]" —
//!   rebuilt on every refill. Forgetting this mapping is THE classic
//!   filtered-list bug (delete/toggle the wrong row).
//! * The status bar (`STATUSCLASSNAMEW`, commctrl — e53 recap):
//!   `SB_SETTEXTW` writes its cells; we drive it with "3 done / 7
//!   total" after every mutation.
//! * Menu-driven UI state: `CheckMenuRadioItem` marks the active View
//!   filter; `LBN_DBLCLK` toggles an item (needs LBS_NOTIFY, e48).
//!
//! Double-click rows to toggle done; use View to filter; File → Clear
//! done removes finished items.
//!
//! Run: `cargo run -p e81_todo_complete`

use windows::{Win32::*, core::*};

const IDC_ENTRY: usize = 1001;
const IDC_ADD: usize = 1002;
const IDC_LIST: usize = 1003;
const IDC_STATUS: usize = 1004;

const IDM_ALL: usize = 4001;
const IDM_OPEN: usize = 4002;
const IDM_DONE: usize = 4003;
const IDM_CLEAR_DONE: usize = 4004;

#[derive(Clone, Copy, PartialEq)]
enum Filter {
    All,
    OnlyOpen,
    OnlyDone,
}

#[derive(Default)]
struct State {
    /// (text, done) — the model. Everything on screen is derived.
    items: Vec<(String, bool)>,
    filter: Option<Filter>,
    /// row -> index into items, rebuilt by refill_list (see header).
    shown: Vec<usize>,
}

fn main() {
    unsafe {
        // Status bar is a common control — register its classes.
        let icc = INITCOMMONCONTROLSEX {
            dwSize: std::mem::size_of::<INITCOMMONCONTROLSEX>() as u32,
            dwICC: ICC_BAR_CLASSES as u32,
        };
        _ = InitCommonControlsEx(&icc);

        let instance = GetModuleHandleW(PCWSTR::null());
        debug_assert!(!instance.is_null());

        let wc = WNDCLASSW {
            lpfnWndProc: Some(wndproc),
            hInstance: instance,
            hCursor: LoadCursorW(None, PCWSTR(IDC_ARROW.0.cast())),
            hbrBackground: ((COLOR_WINDOW + 1) as isize) as HBRUSH,
            lpszClassName: w!("e81_class"),
            style: (CS_HREDRAW | CS_VREDRAW) as u32,
            ..Default::default()
        };
        debug_assert!(RegisterClassW(&wc) != 0);

        let state = Box::into_raw(Box::new(State {
            items: vec![
                (String::from("wire the status bar"), true),
                (String::from("filter the list"), true),
                (String::from("ship e81"), false),
                (String::from("start e82"), false),
            ],
            filter: Some(Filter::All),
            shown: Vec::new(),
        }));

        let hwnd = CreateWindowExW(
            0,
            w!("e81_class"),
            w!("e81 · Todo (complete) — double-click toggles"),
            (WS_OVERLAPPEDWINDOW | WS_VISIBLE) as u32,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            500,
            440,
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

/// Rebuild the listbox from state under the current filter, refreshing
/// the shown-index map. ONE function does all listbox writing — that
/// is what makes the projection impossible to get out of sync.
unsafe fn refill_list(hwnd: HWND) {
    unsafe {
        let Some(state) = state_of(hwnd) else { return };
        let list = GetDlgItem(Some(hwnd), IDC_LIST as i32);

        SendMessageW(list, LB_RESETCONTENT as u32, 0, 0);
        state.shown.clear();

        let filter = state.filter.unwrap_or(Filter::All);
        let items = std::mem::take(&mut state.items);
        for (i, (text, done)) in items.iter().enumerate() {
            let pass = match filter {
                Filter::All => true,
                Filter::OnlyOpen => !*done,
                Filter::OnlyDone => *done,
            };
            if pass {
                let label = if *done {
                    format!("[x] {text}")
                } else {
                    format!("[ ] {text}")
                };
                let wide = to_wide(&label);
                SendMessageW(list, LB_ADDSTRING as u32, 0, wide.as_ptr() as LPARAM);
                state.shown.push(i); // row r shows items[shown[r]]
            }
        }
        state.items = items;

        update_status(hwnd);
    }
}

/// "3 done / 7 total" — one status cell, recalculated from state.
unsafe fn update_status(hwnd: HWND) {
    unsafe {
        let Some(state) = state_of(hwnd) else { return };
        let total = state.items.len();
        let done = state.items.iter().filter(|(_, d)| *d).count();
        let status = GetDlgItem(Some(hwnd), IDC_STATUS as i32);
        if !status.is_null() {
            let wide = to_wide(&format!("{done} done / {total} total"));
            SendMessageW(status, SB_SETTEXTW as u32, 0, wide.as_ptr() as LPARAM);
        }
    }
}

unsafe fn layout(hwnd: HWND) {
    unsafe {
        let mut rc = RECT::default();
        _ = GetClientRect(hwnd, &mut rc);

        // The status bar sizes itself; WM_SIZE just forwards (SB_resize
        // message = WM_SIZE to the bar itself).
        let status = GetDlgItem(Some(hwnd), IDC_STATUS as i32);
        if !status.is_null() {
            _ = MoveWindow(status, 0, rc.bottom - 24, rc.right, 24, true);
        }
        let list = GetDlgItem(Some(hwnd), IDC_LIST as i32);
        if !list.is_null() {
            _ = MoveWindow(list, 12, 52, rc.right - 24, rc.bottom - 52 - 32, true);
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

                // Menus: View (radio-style filters) + File (Clear done).
                let bar = CreateMenu();
                let file_menu = CreatePopupMenu();
                _ = AppendMenuW(
                    file_menu,
                    MF_STRING as u32,
                    IDM_CLEAR_DONE,
                    w!("Clear &done"),
                );
                _ = AppendMenuW(bar, MF_POPUP as u32, file_menu as usize, w!("&File"));

                let view_menu = CreatePopupMenu();
                _ = AppendMenuW(view_menu, MF_STRING as u32, IDM_ALL, w!("&All"));
                _ = AppendMenuW(view_menu, MF_STRING as u32, IDM_OPEN, w!("Only &open"));
                _ = AppendMenuW(view_menu, MF_STRING as u32, IDM_DONE, w!("Only &done"));
                _ = AppendMenuW(bar, MF_POPUP as u32, view_menu as usize, w!("&View"));
                _ = SetMenu(hwnd, Some(bar));
                // Radio-dot the default filter — the menu IS view state.
                _ = CheckMenuRadioItem(
                    view_menu,
                    IDM_ALL as u32,
                    IDM_DONE as u32,
                    IDM_ALL as u32,
                    MF_BYCOMMAND as u32,
                );

                let entry = CreateWindowExW(
                    WS_EX_CLIENTEDGE as u32,
                    w!("EDIT"),
                    w!(""),
                    (WS_CHILD | WS_VISIBLE | ES_AUTOHSCROLL) as u32,
                    12,
                    14,
                    330,
                    26,
                    Some(hwnd),
                    Some(IDC_ENTRY as HMENU),
                    Some(instance),
                    None,
                );
                debug_assert!(!entry.is_null());
                let add = CreateWindowExW(
                    0,
                    w!("BUTTON"),
                    w!("Add"),
                    (WS_CHILD | WS_VISIBLE | BS_DEFPUSHBUTTON) as u32,
                    352,
                    14,
                    80,
                    26,
                    Some(hwnd),
                    Some(IDC_ADD as HMENU),
                    Some(instance),
                    None,
                );
                debug_assert!(!add.is_null());
                let list = CreateWindowExW(
                    WS_EX_CLIENTEDGE as u32,
                    w!("LISTBOX"),
                    w!(""),
                    (WS_CHILD | WS_VISIBLE | WS_VSCROLL | LBS_NOTIFY) as u32,
                    0,
                    0,
                    0,
                    0, // sized in layout()
                    Some(hwnd),
                    Some(IDC_LIST as HMENU),
                    Some(instance),
                    None,
                );
                debug_assert!(!list.is_null());

                // STATUSCLASSNAMEW = "msctls_statusbar32" (e53).
                let status = CreateWindowExW(
                    0,
                    STATUSCLASSNAMEW,
                    w!(""),
                    (WS_CHILD | WS_VISIBLE) as u32,
                    0,
                    0,
                    0,
                    0,
                    Some(hwnd),
                    Some(IDC_STATUS as HMENU),
                    Some(instance),
                    None,
                );
                debug_assert!(!status.is_null());

                layout(hwnd);
                refill_list(hwnd);
                _ = SetFocus(Some(entry));
                0
            }

            WM_SIZE => {
                layout(hwnd);
                0
            }

            WM_COMMAND => {
                let notification = ((wparam >> 16) & 0xffff) as i32;
                let id = (wparam & 0xffff) as usize;

                match id {
                    IDC_ADD if notification == BN_CLICKED as i32 => {
                        let entry = GetDlgItem(Some(hwnd), IDC_ENTRY as i32);
                        let len = SendMessageW(entry, WM_GETTEXTLENGTH as u32, 0, 0) as usize;
                        let mut buf = vec![0u16; len + 1];
                        SendMessageW(
                            entry,
                            WM_GETTEXT as u32,
                            buf.len() as WPARAM,
                            buf.as_mut_ptr() as LPARAM,
                        );
                        _ = SetWindowTextW(entry, w!(""));
                        let text = wide_to_string(&buf).trim().to_string();
                        if !text.is_empty() {
                            if let Some(state) = state_of(hwnd) {
                                state.items.push((text, false));
                            }
                            refill_list(hwnd);
                        }
                        _ = SetFocus(Some(GetDlgItem(Some(hwnd), IDC_ENTRY as i32)));
                        0
                    }

                    // Double-click = toggle. LBN_DBLCLK needs LBS_NOTIFY.
                    IDC_LIST if notification == LBN_DBLCLK as i32 => {
                        let list = GetDlgItem(Some(hwnd), IDC_LIST as i32);
                        let sel = SendMessageW(list, LB_GETCURSEL as u32, 0, 0) as i32;
                        if sel >= 0 {
                            if let Some(state) = state_of(hwnd) {
                                if let Some(&actual) = state.shown.get(sel as usize) {
                                    // Map display row -> model index
                                    // (the whole point of `shown`).
                                    state.items[actual].1 = !state.items[actual].1;
                                }
                            }
                            refill_list(hwnd);
                        }
                        0
                    }

                    IDM_ALL | IDM_OPEN | IDM_DONE => {
                        let filter = match id {
                            IDM_OPEN => Filter::OnlyOpen,
                            IDM_DONE => Filter::OnlyDone,
                            _ => Filter::All,
                        };
                        if let Some(state) = state_of(hwnd) {
                            state.filter = Some(filter);
                        }
                        // Move the radio dot to the picked item.
                        let bar = GetMenu(hwnd);
                        let view_menu = GetSubMenu(bar, 1); // 0=File, 1=View
                        _ = CheckMenuRadioItem(
                            view_menu,
                            IDM_ALL as u32,
                            IDM_DONE as u32,
                            id as u32,
                            MF_BYCOMMAND as u32,
                        );
                        refill_list(hwnd);
                        0
                    }

                    IDM_CLEAR_DONE => {
                        if let Some(state) = state_of(hwnd) {
                            state.items.retain(|(_, done)| !done);
                        }
                        refill_list(hwnd);
                        0
                    }

                    _ => DefWindowProcW(hwnd, message, wparam, lparam),
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
