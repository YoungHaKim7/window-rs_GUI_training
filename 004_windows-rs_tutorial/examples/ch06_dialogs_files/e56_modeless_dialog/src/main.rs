//! e56 — Modeless Dialog: CreateDialogIndirect + the IsDialogMessage pump
//!
//! What you learn:
//! * `CreateDialogIndirectParamW` — the modeless twin of e55's
//!   `DialogBoxIndirectParamW`: same in-memory template, same DLGPROC, but
//!   it RETURNS IMMEDIATELY with the dialog's HWND. You keep running;
//!   owner and dialog are both alive and clickable at once.
//! * **The pump gains one line** — `IsDialogMessage(&mut msg)`: without it
//!   the dialog paints but is brain-dead, because nobody does the dialog
//!   keyboard logic. IsDialogMessage implements Tab/arrow navigation
//!   between WS_TABSTOP controls, Enter → WM_COMMAND/IDOK, Esc →
//!   WM_COMMAND/IDCANCEL, and the Alt-mnemonic dance. If it returns TRUE
//!   it already dispatched the message — don't dispatch twice.
//! * A modeless dialog closes with **`DestroyWindow`**, not `EndDialog` —
//!   and you must remember to clear the HWND you stashed for the pump
//!   (here: in the dialog's WM_DESTROY, which also repaints the owner).
//! * A control IN the dialog reaching state OF the owner: `GetParent(hdlg)`
//!   → the owner HWND → its GWLP_USERDATA. The window-state pattern from
//!   e13, applied across windows.
//!
//! The dialog's checkbox flips what the main window paints — proof that
//! two windows are genuinely sharing one run loop.
//!
//! New Win32 APIs: `CreateDialogIndirectParamW`, `IsDialogMessageW`,
//! `DestroyWindow`, `GetParent`, `IsDlgButtonChecked`.
//!
//! Run: `cargo run -p e56_modeless_dialog`

use windows::{Win32::*, core::*};

/// Main-window button: (re)create the modeless dialog.
const IDC_TOGGLE: i32 = 1001;
/// Dialog checkbox: flips the owner's paint.
const IDC_BANNER: i32 = 2001;
/// Dialog Close button.
const IDC_CLOSE: i32 = 2002;

#[derive(Default)]
struct State {
    hwnd_dialog: HWND, // null while no dialog exists — the pump tests it
    banner: bool,      // the checkbox's latest state
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
            lpszClassName: w!("e56_class"),
            style: (CS_HREDRAW | CS_VREDRAW) as u32,
            ..Default::default()
        };
        debug_assert!(RegisterClassW(&wc) != 0);

        let hwnd = CreateWindowExW(
            0,
            w!("e56_class"),
            w!("e56 · Modeless Dialog — both windows stay alive"),
            (WS_OVERLAPPEDWINDOW | WS_VISIBLE) as u32,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            480,
            300,
            None,
            None,
            Some(instance),
            None,
        );
        debug_assert!(!hwnd.is_null());

        // Attach state (e13 pattern) and remember the raw pointer in main:
        // the message pump below needs live access to hwnd_dialog.
        let state = Box::into_raw(Box::<State>::default());
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, state as isize);

        let toggle = CreateWindowExW(
            0,
            w!("BUTTON"),
            w!("Show / hide the modeless dialog"),
            (WS_CHILD | WS_VISIBLE | BS_PUSHBUTTON) as u32,
            20,
            30,
            220,
            32,
            Some(hwnd),
            Some(IDC_TOGGLE as usize as HMENU),
            Some(instance),
            None,
        );
        debug_assert!(!toggle.is_null());

        // ---- The pump, with its new dialog line ----------------------------
        let mut message = MSG::default();
        while GetMessageW(&mut message, None, 0, 0).0 > 0 {
            // IsDialogMessage: THE dialog keyboard manager. It looks at the
            // message; if it belongs to our modeless dialog (focus within
            // it, or an accelerator-style key), it performs Tab navigation,
            // maps Enter/Esc to WM_COMMAND and dispatches — returning TRUE
            // meaning "consumed". Everything else falls through to the
            // ordinary Translate/Dispatch path.
            if !(*state).hwnd_dialog.is_null()
                && IsDialogMessageW((*state).hwnd_dialog, &mut message).0 != 0
            {
                continue;
            }
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

// ======================================================================
// The template builder — identical machinery to e55 (self-contained).
// ======================================================================
struct TemplateBuilder {
    words: Vec<u16>,
    start: usize,
    pos: usize,
}

impl TemplateBuilder {
    fn new() -> Self {
        // DWORD-align the template start inside a u16 buffer (e55 explains
        // the rounding trick; fixed capacity means the base never moves).
        let words = vec![0u16; 256];
        let start = (!(words.as_ptr() as usize).is_multiple_of(4)) as usize;
        Self {
            words,
            start,
            pos: start,
        }
    }
    fn word(&mut self, w: u16) {
        self.words[self.pos] = w;
        self.pos += 1;
    }
    fn dword(&mut self, dw: u32) {
        self.word(dw as u16);
        self.word((dw >> 16) as u16);
    }
    fn i16s(&mut self, values: &[i16]) {
        for &v in values {
            self.word(v as u16);
        }
    }
    fn string(&mut self, s: &str) {
        for u in s.encode_utf16() {
            self.word(u);
        }
        self.word(0);
    }
    fn align_dword(&mut self) {
        if self.pos % 2 == 1 {
            self.word(0);
        }
    }
    fn as_lpcdlgtemplate(&self) -> LPCDLGTEMPLATEW {
        unsafe { self.words.as_ptr().add(self.start) as LPCDLGTEMPLATEW }
    }
}

/// Dialog: a checkbox + a Close button.
fn build_template() -> TemplateBuilder {
    let mut t = TemplateBuilder::new();

    // Header (see e55 for the field-by-field walkthrough).
    t.dword(WS_POPUP | (WS_CAPTION | WS_SYSMENU | DS_MODALFRAME | DS_CENTER) as u32);
    t.dword(0); // dwExtendedStyle
    t.word(2); // cdit: checkbox + close button
    t.i16s(&[0, 0, 170, 70]); // x, y, cx, cy (dialog units)
    t.word(0); // menu: none
    t.word(0); // class: built-in "#32770"
    t.string("Modeless"); // caption

    // Item 1: the checkbox. BS_AUTOCHECKBOX flips itself on click —
    // dialog manager included, no code needed to toggle the box art.
    t.align_dword();
    t.dword((WS_CHILD | WS_VISIBLE | WS_TABSTOP | BS_AUTOCHECKBOX) as u32);
    t.dword(0);
    t.i16s(&[10, 12, 130, 10]);
    t.word(IDC_BANNER as u16);
    t.word(0xFFFF);
    t.word(0x0080); // atom: BUTTON
    t.string("Paint a banner on the owner");
    t.word(0); // no creation data

    // Item 2: Close — destroys the modeless dialog (see dlgproc).
    t.align_dword();
    t.dword((WS_CHILD | WS_VISIBLE | WS_TABSTOP | BS_PUSHBUTTON) as u32);
    t.dword(0);
    t.i16s(&[60, 40, 50, 14]);
    t.word(IDC_CLOSE as u16);
    t.word(0xFFFF);
    t.word(0x0080); // atom: BUTTON
    t.string("Close");
    t.word(0);

    t
}

// ======================================================================
// The dialog procedure.
// ======================================================================
extern "system" fn dlgproc(hdlg: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> INT_PTR {
    unsafe {
        match message as i32 {
            WM_INITDIALOG => {
                let _ = lparam; // dwInitParam would arrive here (e57 uses it)
                1
            }
            WM_COMMAND => {
                let id = (wparam & 0xffff) as u32;
                match id as i32 {
                    IDC_BANNER => {
                        // Reach across to the OWNER's state: GetParent gives
                        // the window that owns this dialog, and its
                        // GWLP_USERDATA carries the same State the main
                        // wndproc sees. Two windows, one struct.
                        if let Some(state) = state_of(GetParent(hdlg)) {
                            state.banner =
                                IsDlgButtonChecked(hdlg, IDC_BANNER) == BST_CHECKED as u32;
                            _ = InvalidateRect(Some(GetParent(hdlg)), None, true);
                        }
                        1
                    }
                    // Close button: modeless dialogs END with DestroyWindow.
                    // EndDialog would do nothing useful here — there is no
                    // DialogBox call waiting to unblock.
                    IDC_CLOSE => {
                        _ = DestroyWindow(hdlg);
                        1
                    }
                    _ => 0,
                }
            }
            // The [X]: same treatment as the Close button.
            WM_CLOSE => {
                _ = DestroyWindow(hdlg);
                1
            }
            WM_DESTROY => {
                // The pump must STOP calling IsDialogMessage with a dead
                // HWND — so zero the stored handle now, while it's still
                // technically valid, and repaint the owner so the banner
                // state doesn't linger visually.
                if let Some(state) = state_of(GetParent(hdlg)) {
                    state.hwnd_dialog = std::ptr::null_mut();
                    _ = InvalidateRect(Some(GetParent(hdlg)), None, true);
                }
                0
            }
            _ => 0, // dialog manager handles the rest (no DefWindowProcW!)
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
                if notification == BN_CLICKED as u32 && control_id == IDC_TOGGLE as u32 {
                    if state.hwnd_dialog.is_null() {
                        // No dialog yet: create one. Returns at once (that
                        // is what modeless MEANS) with the live HWND.
                        let dialog = CreateDialogIndirectParamW(
                            Some(GetModuleHandleW(PCWSTR::null())),
                            build_template().as_lpcdlgtemplate(),
                            Some(hwnd),
                            Some(dlgproc),
                            0,
                        );
                        debug_assert!(!dialog.is_null());
                        state.hwnd_dialog = dialog;
                        _ = ShowWindow(dialog, SW_SHOWNORMAL);
                    } else {
                        // Already up: hide = destroy in this minimal app.
                        _ = DestroyWindow(state.hwnd_dialog);
                    }
                }
                0
            }
            WM_PAINT => {
                let mut ps = PAINTSTRUCT::default();
                let hdc = BeginPaint(hwnd, &mut ps);
                _ = SetBkMode(hdc, TRANSPARENT);
                let (text, y) = if state.banner {
                    ("BANNER ON — the dialog checkbox paints this", 150)
                } else {
                    ("banner off — tick the dialog's checkbox", 150)
                };
                let wide = to_wide(text);
                _ = TextOutW(
                    hdc,
                    20,
                    y,
                    PCWSTR::from_raw(wide.as_ptr()),
                    wide.len() as i32 - 1,
                );
                _ = SetBkMode(hdc, OPAQUE);
                _ = EndPaint(hwnd, &ps);
                0
            }
            WM_DESTROY => {
                // Close the modeless dialog too if it's still around, so
                // its WM_DESTROY runs before we drop the shared State.
                if !state.hwnd_dialog.is_null() {
                    _ = DestroyWindow(state.hwnd_dialog);
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
