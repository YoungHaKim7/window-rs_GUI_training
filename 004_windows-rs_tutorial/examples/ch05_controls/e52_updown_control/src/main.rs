//! e52 — Updown Control (spin button) and the "buddy" concept
//!
//! What you learn:
//! * The updown control (`msctls_updown32`) — the little ▲/▼ arrow pair.
//! * **Buddies**: an updown by itself is just two arrows. Pair it with
//!   another control (almost always an EDIT) via `UDM_SETBUDDY` and the
//!   two ACT AS ONE: arrows clicked → the updown renumbers the buddy's
//!   text. `UDS_SETBUDDYINT` turns that sync on; `UDS_ALIGNRIGHT` glues
//!   the arrows onto the buddy's right edge (resize the buddy to fit);
//!   `UDS_ARROWKEYS` lets the up/down keys do the clicking.
//!   (Without UDM_SETBUDDY you'd need `UDS_AUTOBUDDY` to grab the
//!   previous sibling in z-order — explicit is better.)
//! * The buddy text is the read-out: because the updown writes it, you
//!   read the value back the way you'd read any EDIT (`GetWindowTextW`).
//!   `UDM_GETPOS32` exists too, but text keeps one source of truth.
//!
//! New Win32 APIs: `UDM_SETBUDDY`, `UDM_SETRANGE32`, `UDS_SETBUDDYINT`,
//! `UDS_ALIGNRIGHT`, `UDS_ARROWKEYS`, `GetWindowTextW`.
//!
//! Run: `cargo run -p e52_updown_control`

use windows::{Win32::*, core::*};

const IDC_SHOW: i32 = 1001; // "Show value" button
const IDC_DOUBLE: i32 = 1002; // "×2" button — writes the buddy text back

#[derive(Default)]
struct State {
    hwnd_buddy: HWND, // the EDIT the updown is attached to
}

fn main() {
    unsafe {
        // The updown family registers under ICC_UPDOWN_CLASS.
        let icc = INITCOMMONCONTROLSEX {
            dwSize: std::mem::size_of::<INITCOMMONCONTROLSEX>() as u32,
            dwICC: ICC_UPDOWN_CLASS as u32,
        };
        _ = InitCommonControlsEx(&icc);

        let instance = GetModuleHandleW(PCWSTR::null());
        debug_assert!(!instance.is_null());

        let wc = WNDCLASSW {
            lpfnWndProc: Some(wndproc),
            hInstance: instance,
            hCursor: LoadCursorW(None, PCWSTR(IDC_ARROW.0.cast())),
            hbrBackground: ((COLOR_WINDOW + 1) as isize) as HBRUSH,
            lpszClassName: w!("e52_class"),
            style: (CS_HREDRAW | CS_VREDRAW) as u32,
            ..Default::default()
        };
        debug_assert!(RegisterClassW(&wc) != 0);

        let hwnd = CreateWindowExW(
            0,
            w!("e52_class"),
            w!("e52 · Updown — arrows renumber the buddy EDIT"),
            (WS_OVERLAPPEDWINDOW | WS_VISIBLE) as u32,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            460,
            220,
            None,
            None,
            Some(instance),
            None,
        );
        debug_assert!(!hwnd.is_null());

        let state = Box::into_raw(Box::<State>::default());
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, state as isize);

        // ---- The buddy: an ordinary single-line EDIT -----------------------
        // Pre-fill it: UDS_SETBUDDYINT only syncs buddy ← updown on clicks,
        // so the initial text is ours to set (and the updown picks it up as
        // its starting position).
        let edit = CreateWindowExW(
            WS_EX_CLIENTEDGE as usize as u32,
            w!("EDIT"),
            w!(""),
            (WS_CHILD | WS_VISIBLE) as u32,
            20,
            24,
            150,
            26,
            Some(hwnd),
            None,
            Some(instance),
            None,
        );
        debug_assert!(!edit.is_null());
        _ = SetWindowTextW(edit, w!("7"));
        _ = SetFocus(Some(edit));

        // ---- The updown ------------------------------------------------------
        // Note the width is only the arrows (16 px) — with UDS_ALIGNRIGHT
        // the updown will MOVE ITSELF to the buddy's right edge and shrink
        // the buddy's usable area. The pair now behaves like one widget.
        let updown = CreateWindowExW(
            0,
            w!("msctls_updown32"),
            w!(""),
            (WS_CHILD | WS_VISIBLE | UDS_SETBUDDYINT | UDS_ALIGNRIGHT | UDS_ARROWKEYS) as u32,
            0,
            0,
            16,
            26, // position is irrelevant — ALIGNRIGHT overrides it
            Some(hwnd),
            None,
            Some(instance),
            None,
        );
        debug_assert!(!updown.is_null());

        // Adopt the EDIT as buddy. After this the arrows click 7 → 8 → 9...
        // and the updown clamps its clicks to the range set next.
        SendMessageW(updown, UDM_SETBUDDY as u32, edit as usize, 0);
        // 32-bit range (the old UDM_SETRANGE had the same 16-bit packing
        // problem as TBM_SETRANGE in e51; this Ex form takes full i32s).
        SendMessageW(updown, UDM_SETRANGE32 as u32, 0, 99);

        // Buttons to prove the buddy text really is the value.
        let show = CreateWindowExW(
            0,
            w!("BUTTON"),
            w!("Show value"),
            (WS_CHILD | WS_VISIBLE | BS_PUSHBUTTON) as u32,
            20,
            70,
            110,
            28,
            Some(hwnd),
            Some(IDC_SHOW as usize as HMENU),
            Some(instance),
            None,
        );
        debug_assert!(!show.is_null());
        let double = CreateWindowExW(
            0,
            w!("BUTTON"),
            w!("Double it (writes back)"),
            (WS_CHILD | WS_VISIBLE | BS_PUSHBUTTON) as u32,
            140,
            70,
            200,
            28,
            Some(hwnd),
            Some(IDC_DOUBLE as usize as HMENU),
            Some(instance),
            None,
        );
        debug_assert!(!double.is_null());

        if let Some(s) = state_of(hwnd) {
            s.hwnd_buddy = edit;
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

/// Read a control's whole text into a String (length first, then bytes —
/// the two-call dance from e43, reused on the buddy).
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
                    IDC_SHOW => {
                        // The buddy text IS the updown's value (kept current
                        // by UDS_SETBUDDYINT). Parse it like any user input.
                        let text = window_text(state.hwnd_buddy);
                        let value: i32 = text.trim().parse().unwrap_or(-1);
                        _ = MessageBoxW(
                            Some(hwnd),
                            &HSTRING::from(format!(
                                "buddy text: \"{text}\"\nparsed value: {value}"
                            )),
                            w!("Updown value"),
                            (MB_OK | MB_ICONINFORMATION) as u32,
                        );
                        0
                    }
                    IDC_DOUBLE => {
                        // Writing the buddy text yourself does NOT move the
                        // updown's internal position — the sync runs one
                        // way (updown → buddy) on clicks. The honest way to
                        // set a value programmatically is UDM_SETPOS32; we
                        // keep it simple and only rewrite the text, which
                        // the NEXT arrow click will re-clamp into range.
                        let value: i32 = window_text(state.hwnd_buddy).trim().parse().unwrap_or(0);
                        let doubled = (value * 2).clamp(0, 99);
                        _ = SetWindowTextW(state.hwnd_buddy, &HSTRING::from(doubled.to_string()));
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
