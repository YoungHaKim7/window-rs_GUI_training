//! e48 — Listbox: a visible list, single click vs double click
//!
//! What you learn:
//! * The LISTBOX control — the combobox's always-expanded sibling. Items go
//!   in with `LB_ADDSTRING`, the selection comes out with `LB_GETCURSEL` +
//!   `LB_GETTEXTLEN` + `LB_GETTEXT` (the same three-message dance as e47)
//! * **`LBS_NOTIFY` is not optional**: without it the listbox changes its
//!   selection in silence and the parent never receives `LBN_SELCHANGE` or
//!   `LBN_DBLCLK`. This is the classic "my click handler doesn't fire" bug.
//! * Two notifications, two intents: `LBN_SELCHANGE` (selection moved —
//!   update a status line) vs `LBN_DBLCLK` (user asked for action — open
//!   it, launch it, edit it). Single-click selects, double-click acts.
//! * `LB_SETCURSEL` — selecting programmatically (which, note, does NOT
//!   send LBN_SELCHANGE; the control can't tell you apart from itself)
//!
//! New Win32 APIs: `LB_ADDSTRING`, `LB_SETCURSEL`, `LB_GETCURSEL`,
//! `LB_GETTEXTLEN`, `LB_GETTEXT`, `LBN_SELCHANGE`, `LBN_DBLCLK`.
//!
//! Run: `cargo run -p e48_listbox`

use windows::{Win32::*, core::*};

/// Control id of the listbox (WM_COMMAND's LOWORD).
const IDC_LIST: i32 = 1001;

/// The entries on offer — a quick tour of common Win32 control classes.
const ITEMS: [&str; 11] = [
    "BUTTON     push buttons, checkboxes, radio buttons, group boxes",
    "STATIC     labels, icons, simple frames",
    "EDIT       single- and multi-line text entry",
    "LISTBOX    this very control",
    "COMBOBOX   edit box + listbox folded into one",
    "SCROLLBAR  standalone scrollbars (e49)",
    "msctls_progress32   progress bars (e50)",
    "msctls_trackbar32   sliders (e51)",
    "msctls_updown32     spin buttons (e52)",
    "msctls_statusbar32  status bars (e53)",
    "ToolbarWindow32     toolbars (e54)",
];

#[derive(Default)]
struct State {
    selected: String, // text of the current selection ("" = none)
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
            lpszClassName: w!("e48_class"),
            style: (CS_HREDRAW | CS_VREDRAW) as u32,
            ..Default::default()
        };
        debug_assert!(RegisterClassW(&wc) != 0);

        let hwnd = CreateWindowExW(
            0,
            w!("e48_class"),
            w!("e48 · Listbox — click, then double-click"),
            (WS_OVERLAPPEDWINDOW | WS_VISIBLE) as u32,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            560,
            380,
            None,
            None,
            Some(instance),
            None,
        );
        debug_assert!(!hwnd.is_null());

        let state = Box::into_raw(Box::<State>::default());
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, state as isize);

        // ---- The listbox child ------------------------------------------------
        // LBS_NOTIFY is THE style everyone forgets: it is what makes the
        // listbox report clicks to its parent as WM_COMMAND notifications.
        // Omit it and this example goes silent — the selection still moves,
        // visually, but LBN_SELCHANGE / LBN_DBLCLK never arrive. (The
        // combobox needed no such opt-in; listboxes predate that politeness.)
        // WS_VSCROLL gives the box its own scrollbar once the entries
        // outgrow its height.
        let list = CreateWindowExW(
            WS_EX_CLIENTEDGE as usize as u32,
            w!("LISTBOX"),
            w!(""),
            (WS_CHILD | WS_VISIBLE | LBS_NOTIFY | WS_VSCROLL) as u32,
            20,
            20,
            510,
            240,
            Some(hwnd),
            Some(IDC_LIST as usize as HMENU),
            Some(instance),
            None,
        );
        debug_assert!(!list.is_null());

        for item in ITEMS {
            let wide = to_wide(item);
            SendMessageW(list, LB_ADDSTRING as u32, 0, wide.as_ptr() as LPARAM);
        }
        // Preselect entry 0. Note for later: this does NOT trigger
        // LBN_SELCHANGE — programmatic changes never notify, so update any
        // dependent state yourself right next to the call.
        SendMessageW(list, LB_SETCURSEL as u32, 0, 0);
        if let Some(s) = state_of(hwnd) {
            s.selected = ITEMS[0].to_string();
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

/// Read the text of the listbox's current selection, or None when nothing
/// is selected. Same shape as reading a combobox entry or window text:
/// index, then length, then buffer.
unsafe fn selected_text(list: HWND) -> Option<String> {
    unsafe {
        let index = SendMessageW(list, LB_GETCURSEL as u32, 0, 0) as i32;
        if index < 0 {
            return None; // LB_ERR: no selection
        }
        let len = SendMessageW(list, LB_GETTEXTLEN as u32, index as usize, 0) as i32;
        let mut buf = vec![0u16; len as usize + 1];
        SendMessageW(
            list,
            LB_GETTEXT as u32,
            index as usize,
            buf.as_mut_ptr() as LPARAM,
        );
        Some(String::from_utf16_lossy(&buf[..len.max(0) as usize]))
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

                if control_id != IDC_LIST as u32 {
                    return 0;
                }
                let list = lparam as HWND; // the control's own HWND

                match notification as i32 {
                    // Selection moved: remember it and refresh the status
                    // line. Cheap reaction, no side effects.
                    LBN_SELCHANGE => {
                        state.selected = selected_text(list).unwrap_or_default();
                        _ = InvalidateRect(Some(hwnd), None, true);
                        0
                    }
                    // Double-click: the user means "do something with this".
                    // A double click always selects the item first (so
                    // LB_GETCURSEL is already the clicked row) — which is
                    // why the pattern "select on SELCHANGE, act on DBLCLK"
                    // needs no extra bookkeeping.
                    LBN_DBLCLK => {
                        let what = selected_text(list).unwrap_or_default();
                        // The class name is the first word of the entry.
                        let class = what.split_whitespace().next().unwrap_or("");
                        _ = MessageBoxW(
                            Some(hwnd),
                            &HSTRING::from(format!(
                                "You chose the {class} control.\n\n\
                                 In a real app, LBN_DBLCLK is where you \
                                 would open / launch / edit it."
                            )),
                            w!("Double-click"),
                            (MB_OK | MB_ICONINFORMATION) as u32,
                        );
                        0
                    }
                    _ => 0,
                }
            }
            WM_PAINT => {
                let mut ps = PAINTSTRUCT::default();
                let hdc = BeginPaint(hwnd, &mut ps);
                let mut rect = RECT::default();
                _ = GetClientRect(hwnd, &mut rect);

                // The status line under the list: just text painted from
                // State (repaint-on-state-change keeps it always current).
                let line = if state.selected.is_empty() {
                    "Nothing selected.".to_string()
                } else {
                    format!("Selected: {}", state.selected)
                };
                let wide = to_wide(&line);
                _ = SetBkMode(hdc, TRANSPARENT);
                _ = TextOutW(
                    hdc,
                    20,
                    286,
                    PCWSTR::from_raw(wide.as_ptr()),
                    wide.len() as i32 - 1, // minus the null terminator
                );
                _ = SetBkMode(hdc, OPAQUE);

                _ = EndPaint(hwnd, &ps);
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
