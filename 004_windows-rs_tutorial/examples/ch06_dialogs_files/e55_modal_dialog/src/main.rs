//! e55 — Modal Dialog from an In-Memory Template: DLGTEMPLATE by hand
//!
//! What you learn:
//! * Dialogs are normally described in a .rc resource script — but nothing
//!   requires that. `DialogBoxIndirectParamW` accepts a template you build
//!   as a **byte-exact buffer in memory**: a `DLGTEMPLATE` header followed
//!   by one `DLGITEMTEMPLATE` per control, each field commented below.
//! * The two alignment rules that make or break indirect templates: the
//!   buffer must start on a **DWORD (4-byte) boundary**, and every
//!   DLGITEMTEMPLATE must *also* begin on one — the builder pads with a
//!   zero WORD when the running size is odd. Misaligned templates are the
//!   classic cause of ERROR_NOACCESS (998) / dialogs that never appear.
//! * **Dialog coordinates are dialog units, not pixels** — 1/4 of the
//!   average dialog-font character width/height, so dialogs scale with
//!   font DPI settings.
//! * The `DLGPROC` contract: return TRUE (1) for handled messages — and
//!   from `WM_INITDIALOG` specifically, 1 means "set focus yourself";
//!   there is NO DefWindowProcW — returning 0 (FALSE) lets the built-in
//!   dialog manager (window class #32770) handle the rest.
//! * `EndDialog` is the only exit: both IDOK and IDCANCEL and the [X]
//!   button (WM_CLOSE) funnel through it; its second argument becomes
//!   `DialogBoxIndirectParamW`'s return value.
//!
//! Modal means the call BLOCKS until the dialog closes — the main window's
//! button handler is still on the stack the whole time.
//!
//! New Win32 APIs: `DialogBoxIndirectParamW`, `DLGTEMPLATE`,
//! `DLGITEMTEMPLATE`, `EndDialog`, `WM_INITDIALOG`.
//!
//! Run: `cargo run -p e55_modal_dialog`

use windows::{Win32::*, core::*};

/// Main-window button that opens the dialog.
const IDC_SHOW: i32 = 1001;

fn main() {
    unsafe {
        let instance = GetModuleHandleW(PCWSTR::null());
        debug_assert!(!instance.is_null());

        let wc = WNDCLASSW {
            lpfnWndProc: Some(wndproc),
            hInstance: instance,
            hCursor: LoadCursorW(None, PCWSTR(IDC_ARROW.0.cast())),
            hbrBackground: ((COLOR_WINDOW + 1) as isize) as HBRUSH,
            lpszClassName: w!("e55_class"),
            style: (CS_HREDRAW | CS_VREDRAW) as u32,
            ..Default::default()
        };
        debug_assert!(RegisterClassW(&wc) != 0);

        let hwnd = CreateWindowExW(
            0,
            w!("e55_class"),
            w!("e55 · Modal Dialog — built in memory, no .rc file"),
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

        let show = CreateWindowExW(
            0,
            w!("BUTTON"),
            w!("Show the modal dialog"),
            (WS_CHILD | WS_VISIBLE | BS_PUSHBUTTON) as u32,
            20,
            30,
            180,
            32,
            Some(hwnd),
            Some(IDC_SHOW as usize as HMENU),
            Some(instance),
            None,
        );
        debug_assert!(!show.is_null());

        let mut message = MSG::default();
        while GetMessageW(&mut message, None, 0, 0).0 > 0 {
            _ = TranslateMessage(&message);
            DispatchMessageW(&message);
        }
    }
}

// ======================================================================
// The template builder — a cursor over a u16 buffer.
// ======================================================================
struct TemplateBuilder {
    words: Vec<u16>, // fixed capacity: the base address never moves
    start: usize,    // first index of the template (see alignment note)
    pos: usize,      // cursor, in u16 units
}

impl TemplateBuilder {
    fn new() -> Self {
        // Vec<u16> guarantees only 2-byte alignment (its element type), but
        // the API demands 4 for the DLGTEMPLATE start. If the allocation
        // happens to sit at an odd word (2 mod 4 bytes), skip one word.
        // The fixed capacity means no reallocation can move the base.
        let words = vec![0u16; 256];
        let start = (words.as_ptr() as usize % 4 != 0) as usize;
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
        self.word(dw as u16); // low half first (little-endian)
        self.word((dw >> 16) as u16);
    }
    fn i16s(&mut self, values: &[i16]) {
        for &v in values {
            self.word(v as u16);
        }
    }
    /// Null-terminated wide string (the template has no length prefixes).
    fn string(&mut self, s: &str) {
        for u in s.encode_utf16() {
            self.word(u);
        }
        self.word(0);
    }
    /// Pad to the next DWORD boundary before a DLGITEMTEMPLATE.
    fn align_dword(&mut self) {
        if self.pos % 2 == 1 {
            self.word(0);
        }
    }
    fn as_lpcdlgtemplate(&self) -> LPCDLGTEMPLATEW {
        unsafe { self.words.as_ptr().add(self.start) as LPCDLGTEMPLATEW }
    }
}

/// Build the whole template: one dialog + one OK button. Field by field.
fn build_template() -> TemplateBuilder {
    let mut t = TemplateBuilder::new();

    // ---- DLGTEMPLATE header (18 bytes, if you're counting) ---------------
    t.dword(
        // WS_POPUP + WS_CAPTION + WS_SYSMENU = a normal titled dialog box.
        // DS_MODALFRAME = the thick modal border. DS_CENTER = open centered
        // on the owner (x/y below are then relative offsets of 0).
        // (WS_POPUP is a u32 constant in this crate — mind the cast dance.)
        WS_POPUP as u32 | (WS_CAPTION | WS_SYSMENU | DS_MODALFRAME | DS_CENTER) as u32,
    );
    t.dword(0); // dwExtendedStyle: none
    t.word(1); // cdit: count of DLGITEMTEMPLATEs that follow — one button

    // x, y, cx, cy — DIALOG UNITS (≈ 1/4 of the dialog font's average char
    // width / 1/8 of its height), not pixels. 180×60 du ≈ a small message
    // box. These are i16s in the wire format.
    t.i16s(&[0, 0, 180, 60]);

    // menu: 0 = this dialog has no menu. (A WORD here means "none"; a
    // nonzero WORD would be a menu ordinal, a string a menu name.)
    t.word(0);
    // class: 0 = the built-in dialog class "#32770" — Windows' own window
    // procedure with the dialog manager behavior (Tab handling, IDOK...).
    t.word(0);
    // title: null-terminated string → the dialog's caption bar text.
    t.string("Hello from a template");

    // ---- DLGITEMTEMPLATE: the OK button ----------------------------------
    // RULE: every item must START on a DWORD boundary. Our header so far is
    // 9 words of fixed fields + title words + 1 null; if that's odd, pad.
    t.align_dword();

    t.dword(
        // Same style vocabulary as CreateWindowExW children: WS_TABSTOP is
        // what makes dialogs' Tab-key navigation include this button.
        // BS_DEFPUSHBUTTON = default button (Enter fires it → WM_COMMAND
        // with this id).
        (WS_CHILD | WS_VISIBLE | WS_TABSTOP | BS_DEFPUSHBUTTON) as u32,
    );
    t.dword(0); // dwExtendedStyle: none
    t.i16s(&[65, 38, 50, 14]); // x, y, cx, cy — dialog units again
    t.word(IDOK as u16); // id: what WM_COMMAND reports for this control

    // class: either a window-class string, or the shortcut — 0xFFFF
    // followed by a predefined class ATOM: 0x0080 BUTTON, 0x0081 EDIT,
    // 0x0082 STATIC, 0x0083 LISTBOX, 0x0084 SCROLLBAR, 0x0085 COMBOBOX.
    t.word(0xFFFF);
    t.word(0x0080); // BUTTON

    // title: the button's caption.
    t.string("OK");
    // creation data: a WORD byte-count (0 = "none") then that many bytes.
    // Only exotic controls ever need this.
    t.word(0);

    t
}

// ======================================================================
// The dialog procedure — a different beast from a wndproc.
// ======================================================================
extern "system" fn dlgproc(hdlg: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> INT_PTR {
    unsafe {
        match message as i32 {
            // First message the dialog ever receives — the controls exist
            // already (the manager created them from the template). Return
            // TRUE so the manager sets focus to the (first WS_TABSTOP)
            // control; return FALSE only if you called SetFocus yourself.
            // Forgetting "return 1" here is the #1 first-dialog bug.
            WM_INITDIALOG => {
                let _ = (hdlg, lparam); // e57 uses lparam (dwInitParam)
                1
            }
            // Same WM_COMMAND shape as chapter 5: LOWORD = control id.
            // Enter → IDOK, Esc → IDCANCEL (the dialog manager maps the
            // keys for you — that is a large part of what "dialog" buys).
            WM_COMMAND => {
                let id = (wparam & 0xffff) as u32;
                match id as i32 {
                    IDOK | IDCANCEL => {
                        // The ONLY way a modal dialog ends. The second
                        // argument rides back as DialogBoxIndirect's
                        // return value — callers use it like a result code.
                        _ = EndDialog(hdlg, id as INT_PTR);
                        1 // "handled"
                    }
                    _ => 0,
                }
            }
            // The [X] button posts WM_CLOSE — without this arm, a modal
            // dialog would NOT actually end (DestroyWindow alone doesn't
            // settle a DialogBox; EndDialog does).
            WM_CLOSE => {
                _ = EndDialog(hdlg, IDCANCEL as INT_PTR);
                1
            }
            // CRITICAL difference from wndproc: no DefWindowProcW. Return 0
            // ("not handled") and the built-in dialog manager does the
            // default; returning 1 for something you didn't handle breaks
            // the manager's bookkeeping.
            _ => 0,
        }
    }
}

extern "system" fn wndproc(hwnd: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe {
        match message as i32 {
            WM_COMMAND => {
                let notification = ((wparam >> 16) & 0xffff) as u32;
                let control_id = (wparam & 0xffff) as u32;
                if notification == BN_CLICKED as u32 && control_id == IDC_SHOW as u32 {
                    // Build the template fresh on every click (the dialog
                    // machinery reads it synchronously; it could as well be
                    // built once and reused).
                    let template = build_template();
                    // THE call. Blocks here — this WM_COMMAND handler is on
                    // the stack, pumping the dialog's own messages, until
                    // EndDialog fires. The last argument (dwInitParam,
                    // 0 here) would surface as WM_INITDIALOG's lparam —
                    // that's e57's whole trick.
                    let result = DialogBoxIndirectParamW(
                        Some(GetModuleHandleW(PCWSTR::null())),
                        template.as_lpcdlgtemplate(),
                        Some(hwnd),
                        Some(dlgproc),
                        0,
                    );
                    // -1 means the call itself failed (bad template...).
                    let what = match result as i32 {
                        IDOK => "IDOK",
                        IDCANCEL => "IDCANCEL",
                        _ => "call failed",
                    };
                    _ = SetWindowTextW(
                        hwnd,
                        &HSTRING::from(format!("e55 · dialog returned {what}")),
                    );
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
