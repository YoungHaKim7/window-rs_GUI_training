//! e68 — Subclassing: SetWindowSubclass and WM_CHAR interception
//!
//! What you learn:
//! * **Subclassing** — intercepting the messages of a window whose wndproc
//!   you don't own. The old `SetWindowLongPtrW(GWLP_WNDPROC)` trick is
//!   unsafe across modules; the modern, sanctioned way is the ComCtl32
//!   trio `SetWindowSubclass` / `DefSubclassProc` / `RemoveWindowSubclass`
//!   (yes, "subclassing a control" lives in commctrl, feature `commctrl`)
//! * The subclass procedure's shape: your 4 usual parameters PLUS
//!   `uidsubclass` (the id you chose, so several subclasses can tell
//!   themselves apart) and `dwrefdata` (the ref data you passed in — a
//!   free per-subclass context slot)
//! * **Intercepting WM_CHAR**: return 0 for characters you reject (they
//!   vanish — the edit never sees them), pass everything else to
//!   `DefSubclassProc` (the "call the original wndproc" of subclassing;
//!   NEVER DefWindowProcW — that would skip the control's own logic)
//! * Contrast with the built-in `ES_NUMBER`, which only accepts decimal
//!   digits: subclassing is how you accept ANY alphabet (here: hex
//!   [0-9a-fA-F]) — something no style bit offers
//! * Removing the subclass in WM_DESTROY: subclasses stack, so un-stack
//!   yours deterministically before the window dies
//!
//! Type into both edits: the top one (ES_NUMBER) rejects hex letters; the
//! bottom one (subclassed) rejects everything that is not a hex digit.
//! Backspace works in both — a keystroke you must not eat.
//!
//! New Win32 APIs: `SetWindowSubclass`, `DefSubclassProc`,
//! `RemoveWindowSubclass`, `SUBCLASSPROC`, `ES_NUMBER`.
//!
//! Run: `cargo run -p e68_subclassing`

use windows::{Win32::*, core::*};

/// Control id of the subclassed hex edit.
const IDC_HEX: i32 = 1002;

/// Our subclass id. Any value you like; it comes back as the 5th parameter
/// of every subclass call, and RemoveWindowSubclass must pass the SAME
/// (id, proc) pair you installed.
const HEX_SUBCLASS_ID: usize = 1;

#[derive(Default)]
struct State {
    /// How many keystrokes the subclass has eaten so far (painted as proof).
    rejected: u32,
    hwnd_hex: HWND,
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
            lpszClassName: w!("e68_class"),
            style: (CS_HREDRAW | CS_VREDRAW) as u32,
            ..Default::default()
        };
        debug_assert!(RegisterClassW(&wc) != 0);

        let hwnd = CreateWindowExW(
            0,
            w!("e68_class"),
            w!("e68 · Subclassing — an EDIT that only accepts hex"),
            (WS_OVERLAPPEDWINDOW | WS_VISIBLE) as u32,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            560,
            300,
            None,
            None,
            Some(instance),
            None,
        );
        debug_assert!(!hwnd.is_null());

        let state = Box::into_raw(Box::<State>::default());
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, state as isize);

        // ---- Edit #1: the built-in way ------------------------------------
        // ES_NUMBER is a style bit the edit control itself enforces: it
        // swallows every non-digit keystroke for you. Convenient — but the
        // alphabet is hard-wired to decimal digits, and that is all you get.
        let decimal = CreateWindowExW(
            0,
            w!("EDIT"),
            w!("123"),
            (WS_CHILD | WS_VISIBLE | WS_BORDER | ES_AUTOHSCROLL | ES_NUMBER) as u32,
            20,
            60,
            500,
            28,
            Some(hwnd),
            Some(1001usize as HMENU),
            Some(instance),
            None,
        );
        debug_assert!(!decimal.is_null());

        // ---- Edit #2: the subclassed way ----------------------------------
        // A perfectly ordinary edit. The hex filter is NOT a style — it is
        // our code, sitting between the message loop and the edit's real
        // wndproc.
        let hex = CreateWindowExW(
            0,
            w!("EDIT"),
            w!("deadbeef"),
            (WS_CHILD | WS_VISIBLE | WS_BORDER | ES_AUTOHSCROLL) as u32,
            20,
            120,
            500,
            28,
            Some(hwnd),
            Some(IDC_HEX as usize as HMENU),
            Some(instance),
            None,
        );
        debug_assert!(!hex.is_null());

        // Install the subclass. uidsubclass is our id, dwrefdata (here 0) is
        // a free context value delivered to every subclass call — the
        // documented place to smuggle a pointer if you need real state.
        _ = SetWindowSubclass(hex, Some(hex_edit_proc), HEX_SUBCLASS_ID, 0);

        if let Some(s) = state_of(hwnd) {
            s.hwnd_hex = hex;
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

fn rgb(red: u8, green: u8, blue: u8) -> COLORREF {
    (red as u32) | ((green as u32) << 8) | ((blue as u32) << 16)
}

/// The subclass procedure — a wndproc with two extra trailing parameters.
///
/// Runs BEFORE the edit's real wndproc for every message sent to the
/// control. Returning a value directly SWALLOWS the message; calling
/// `DefSubclassProc` forwards it to the next subclass (if any) and then
/// the control's own procedure. That forwarding call is the difference
/// between "filter" and "break".
unsafe extern "system" fn hex_edit_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    _uidsubclass: usize, // == HEX_SUBCLASS_ID; one proc can serve many ids
    _dwrefdata: usize,   // the context value from SetWindowSubclass
) -> LRESULT {
    unsafe {
        // WM_CHAR arrives AFTER TranslateMessage turned a key press into a
        // character, so wparam is the CHARACTER code (not a virtual key).
        // This is the natural interception point for input filtering.
        if message == WM_CHAR as u32 {
            let ch = wparam as u16;
            // Byte-literal ranges are u8, so widen the endpoints explicitly.
            let allowed = (b'0' as u16..=b'9' as u16).contains(&ch)
                || (b'a' as u16..=b'f' as u16).contains(&ch)
                || (b'A' as u16..=b'F' as u16).contains(&ch)
                || ch == 0x08; // backspace — eating it would lock the user in
            if !allowed {
                // Reject: report "handled, do nothing". The edit never sees
                // the character. (Beeper niceties like MessageBeep left out.)
                if let Some(state) = state_of(GetParent(hwnd)) {
                    state.rejected += 1;
                    _ = InvalidateRect(Some(GetParent(hwnd)), None, true);
                }
                return 0;
            }
        }
        // Everything else (including the ALLOWED characters: painting,
        // caret moves, mouse, WM_GETTEXT...) goes to the original proc.
        DefSubclassProc(hwnd, message, wparam, lparam)
    }
}

extern "system" fn wndproc(hwnd: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe {
        let Some(state) = state_of(hwnd) else {
            return DefWindowProcW(hwnd, message, wparam, lparam);
        };

        match message as i32 {
            WM_PAINT => {
                let mut ps = PAINTSTRUCT::default();
                let hdc = BeginPaint(hwnd, &mut ps);

                _ = SetBkMode(hdc, TRANSPARENT);
                _ = SetTextColor(hdc, rgb(0x30, 0x30, 0x30));
                let wide = to_wide("ES_NUMBER edit — decimal digits only (built-in):");
                _ = TextOutW(hdc, 20, 38, PCWSTR::from_raw(wide.as_ptr()), -1);
                let wide = to_wide("Subclassed edit — hex [0-9a-fA-F] only (our code):");
                _ = TextOutW(hdc, 20, 98, PCWSTR::from_raw(wide.as_ptr()), -1);

                _ = SetTextColor(hdc, rgb(0xA0, 0x30, 0x30));
                let report = format!("characters rejected by the subclass: {}", state.rejected);
                let wide = to_wide(&report);
                _ = TextOutW(hdc, 20, 170, PCWSTR::from_raw(wide.as_ptr()), -1);

                _ = EndPaint(hwnd, &ps);
                0
            }
            WM_DESTROY => {
                // Un-stack our subclass before the child dies. Pass the
                // exact (proc, id) pair from SetWindowSubclass. Skipping
                // this leaks the slot until the window is destroyed anyway —
                // but explicit removal is the habit that scales to
                // subclassing windows you don't own.
                if !state.hwnd_hex.is_null() {
                    _ = RemoveWindowSubclass(state.hwnd_hex, Some(hex_edit_proc), HEX_SUBCLASS_ID);
                }
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
