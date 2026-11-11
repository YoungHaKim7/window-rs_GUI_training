//! e57 — A Real Settings Dialog: controls in the template, data both ways
//!
//! What you learn:
//! * The canonical settings-dialog pattern, end to end: controls declared
//!   IN the in-memory template (from e55) — STATIC label, EDIT,
//!   checkbox, a radio pair, OK/Cancel — prefilled on open, read back on
//!   OK, ignored on Cancel
//! * **Passing data through `dwInitParam`**: box up the settings,
//!   `Box::into_raw`, hand the pointer to `DialogBoxIndirectParamW`, catch
//!   it in `WM_INITDIALOG`'s lparam, stash it in the dialog's own
//!   `GWLP_USERDATA` — every later message can get it back. This is THE
//!   way a dialog and your data find each other without globals.
//! * `GetDlgItem` — address a template control by the id you gave it
//!   (no HWNDs stored anywhere); `CheckDlgButton` / `IsDlgButtonChecked`
//!   — check state as a two-way street
//! * Radio groups: `BS_AUTORADIOBUTTON` un-checks siblings; `WS_GROUP` on
//!   the first radio closes the group so arrow keys hop within it
//!
//! The main window paints "Hello, {name}" with the chosen weight/color —
//! commit happens only when the dialog returns IDOK.
//!
//! New Win32 APIs: `GetDlgItem`, `CheckDlgButton`, `IsDlgButtonChecked`,
//! `SetWindowLongPtrW(GWLP_USERDATA)` on a dialog, `CreateFontW`.
//!
//! Run: `cargo run -p e57_dialog_controls`

use windows::{Win32::*, core::*};

/// Main-window button that opens Settings.
const IDC_SETTINGS: i32 = 1001;
// Dialog control ids (the template carries them; WM_COMMAND reports them).
const IDC_NAME: i32 = 2001; // EDIT
const IDC_BOLD: i32 = 2002; // checkbox
const IDC_BLUE: i32 = 2003; // radio 1
const IDC_RED: i32 = 2004; // radio 2

/// The settings this dialog edits. Clone in, edit inside the dialog,
/// commit back only on OK.
#[derive(Clone, Default)]
struct Settings {
    name: String,
    bold: bool,
    blue: bool, // false = red
}

#[derive(Default)]
struct State {
    settings: Settings, // committed values; the greeting paints from here
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
            lpszClassName: w!("e57_class"),
            style: (CS_HREDRAW | CS_VREDRAW) as u32,
            ..Default::default()
        };
        debug_assert!(RegisterClassW(&wc) != 0);

        let hwnd = CreateWindowExW(
            0,
            w!("e57_class"),
            w!("e57 · Settings Dialog — the canonical pattern"),
            (WS_OVERLAPPEDWINDOW | WS_VISIBLE) as u32,
            CW_USEDEFAULT, CW_USEDEFAULT, 520, 300,
            None, None, Some(instance), None,
        );
        debug_assert!(!hwnd.is_null());

        let state = Box::into_raw(Box::<State>::default());
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, state as isize);

        let settings = CreateWindowExW(
            0,
            w!("BUTTON"),
            w!("Settings..."),
            (WS_CHILD | WS_VISIBLE | BS_PUSHBUTTON) as u32,
            20, 30, 110, 32,
            Some(hwnd), Some(IDC_SETTINGS as usize as HMENU), Some(instance), None,
        );
        debug_assert!(!settings.is_null());

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

fn rgb(red: u8, green: u8, blue: u8) -> COLORREF {
    (red as u32) | ((green as u32) << 8) | ((blue as u32) << 16)
}

fn to_wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

unsafe fn window_text(hwnd: HWND) -> String {
    unsafe {
        let len = GetWindowTextLengthW(hwnd);
        let mut buf = vec![0u16; len as usize + 1];
        let copied = GetWindowTextW(hwnd, PWSTR(buf.as_mut_ptr()), buf.len() as i32);
        String::from_utf16_lossy(&buf[..copied.max(0) as usize])
    }
}

// ======================================================================
// Template builder — same machinery as e55/e56, now with 7 items.
// ======================================================================
struct TemplateBuilder {
    words: Vec<u16>,
    start: usize,
    pos: usize,
}

impl TemplateBuilder {
    fn new() -> Self {
        let words = vec![0u16; 512];
        let start = (words.as_ptr() as usize % 4 != 0) as usize;
        Self { words, start, pos: start }
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

/// Append one item to the template (fields explained per call site).
fn item(
    t: &mut TemplateBuilder,
    style: u32,
    ex_style: u32,
    pos: [i16; 4], // x, y, cx, cy in dialog units
    id: i32,
    class_atom: u16,
    text: &str,
) {
    t.align_dword(); // every DLGITEMTEMPLATE starts DWORD-aligned
    t.dword(style);
    t.dword(ex_style);
    t.i16s(&pos);
    t.word(id as u16);
    t.word(0xFFFF); // "class comes as an atom next"
    t.word(class_atom); // 0x0080 BUTTON, 0x0081 EDIT, 0x0082 STATIC
    t.string(text); // window text = label / initial content
    t.word(0); // no creation data
}

fn build_template() -> TemplateBuilder {
    let mut t = TemplateBuilder::new();

    t.dword(WS_POPUP as u32 | (WS_CAPTION | WS_SYSMENU | DS_MODALFRAME | DS_CENTER) as u32);
    t.dword(0);
    t.word(7); // cdit: 7 items
    t.i16s(&[0, 0, 190, 92]);
    t.word(0); // no menu
    t.word(0); // built-in dialog class
    t.string("Settings");

    // Label: a plain STATIC. id 0xFFFF (= -1) is the convention for
    // controls nobody will ever address — statics, group frames.
    item(&mut t, (WS_CHILD | WS_VISIBLE | SS_LEFT) as u32, 0,
         [10, 14, 30, 8], -1, 0x0082, "Name:");
    // The EDIT: WS_EX_CLIENTEDGE rides in dwExtendedStyle — this is where
    // extended styles live in a template (there is no second style field).
    item(&mut t, (WS_CHILD | WS_VISIBLE | WS_TABSTOP | ES_AUTOHSCROLL) as u32,
         WS_EX_CLIENTEDGE as u32,
         [45, 12, 130, 12], IDC_NAME, 0x0081, "");
    // Checkbox.
    item(&mut t, (WS_CHILD | WS_VISIBLE | WS_TABSTOP | BS_AUTOCHECKBOX) as u32, 0,
         [10, 30, 60, 10], IDC_BOLD, 0x0080, "Bold text");
    // Radio pair: WS_GROUP on the FIRST radio opens a group — arrow keys
    // cycle within it, and BS_AUTORADIOBUTTON auto-unchecks the sibling.
    item(&mut t,
         (WS_CHILD | WS_VISIBLE | WS_TABSTOP | WS_GROUP | BS_AUTORADIOBUTTON) as u32, 0,
         [10, 44, 45, 10], IDC_BLUE, 0x0080, "Blue");
    item(&mut t, (WS_CHILD | WS_VISIBLE | BS_AUTORADIOBUTTON) as u32, 0,
         [60, 44, 45, 10], IDC_RED, 0x0080, "Red");
    // OK is the default button; ids 1/2 are the standard IDOK/IDCANCEL,
    // so the dialog manager's Enter/Esc mapping finds them automatically.
    item(&mut t, (WS_CHILD | WS_VISIBLE | WS_TABSTOP | BS_DEFPUSHBUTTON) as u32, 0,
         [85, 68, 45, 14], IDOK, 0x0080, "OK");
    item(&mut t, (WS_CHILD | WS_VISIBLE | WS_TABSTOP | BS_PUSHBUTTON) as u32, 0,
         [138, 68, 45, 14], IDCANCEL, 0x0080, "Cancel");

    t
}

// ======================================================================
// The dialog procedure — prefill on init, copy back on OK.
// ======================================================================
extern "system" fn dlgproc(
    hdlg: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> INT_PTR {
    unsafe {
        match message as i32 {
            // lparam IS the dwInitParam we passed to DialogBoxIndirect-
            // ParamW — the *mut Settings. Stash it in the dialog's own
            // GWLP_USERDATA: from now on every message can recover it.
            WM_INITDIALOG => {
                SetWindowLongPtrW(hdlg, GWLP_USERDATA, lparam as isize);
                let ptr = GetWindowLongPtrW(hdlg, GWLP_USERDATA) as *mut Settings;

                // Prefill the controls from the settings.
                let s = &*ptr;
                _ = SetWindowTextW(GetDlgItem(Some(hdlg), IDC_NAME), &HSTRING::from(s.name.clone()));
                _ = CheckDlgButton(hdlg, IDC_BOLD, s.bold as u32);
                // Radios: check exactly one.
                _ = CheckDlgButton(
                    hdlg,
                    if s.blue { IDC_BLUE } else { IDC_RED },
                    BST_CHECKED as u32,
                );
                1
            }
            WM_COMMAND => {
                let id = (wparam & 0xffff) as u32;
                match id as i32 {
                    IDOK => {
                        // Copy control state back into the Settings box.
                        // GetDlgItem resolves id → HWND on the fly; we
                        // stored no child handles anywhere.
                        let ptr = GetWindowLongPtrW(hdlg, GWLP_USERDATA) as *mut Settings;
                        let s = &mut *ptr;
                        s.name = window_text(GetDlgItem(Some(hdlg), IDC_NAME));
                        s.bold = IsDlgButtonChecked(hdlg, IDC_BOLD) == BST_CHECKED as u32;
                        s.blue = IsDlgButtonChecked(hdlg, IDC_BLUE) == BST_CHECKED as u32;
                        _ = EndDialog(hdlg, IDOK as INT_PTR);
                        1
                    }
                    IDCANCEL => {
                        // Cancel: just leave. The box is untouched — the
                        // caller checks the return value before committing.
                        _ = EndDialog(hdlg, IDCANCEL as INT_PTR);
                        1
                    }
                    _ => 0,
                }
            }
            WM_CLOSE => {
                _ = EndDialog(hdlg, IDCANCEL as INT_PTR);
                1
            }
            _ => 0,
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
                if notification == BN_CLICKED as u32 && control_id == IDC_SETTINGS as u32 {
                    // ---- The data round-trip --------------------------------
                    // 1. Work on a copy: Cancel must be able to walk away
                    //    without a scratch on the live settings.
                    let work = Box::into_raw(Box::new(state.settings.clone()));
                    // 2. Modal call — blocks until EndDialog; the pointer
                    //    travels in as dwInitParam, the box gets edited in
                    //    place by the dlgproc.
                    let result = DialogBoxIndirectParamW(
                        Some(GetModuleHandleW(PCWSTR::null())),
                        build_template().as_lpcdlgtemplate(),
                        Some(hwnd),
                        Some(dlgproc),
                        work as LPARAM,
                    );
                    // 3. Reclaim the box (the dialog is gone; nobody else
                    //    holds the pointer) and commit only on OK.
                    let work = Box::from_raw(work);
                    if result == IDOK as INT_PTR {
                        state.settings = *work;
                        _ = InvalidateRect(Some(hwnd), None, true);
                    }
                }
                0
            }
            WM_PAINT => {
                let mut ps = PAINTSTRUCT::default();
                let hdc = BeginPaint(hwnd, &mut ps);
                let mut rect = RECT::default();
                _ = GetClientRect(hwnd, &mut rect);

                let s = &state.settings;
                // Bold = a heavier font. CreateFontW's 14 parameters reduce
                // to: height, weight, charset/quality, face name (null =
                // default GUI font face). e62 replaces the hand-rolled
                // fields with a real LOGFONTW from the font picker.
                let font = CreateFontW(
                    32, 0, 0, 0,
                    if s.bold { FW_BOLD as i32 } else { FW_NORMAL as i32 },
                    0, 0, 0,
                    DEFAULT_CHARSET as u32,
                    OUT_DEFAULT_PRECIS as u32,
                    CLIP_DEFAULT_PRECIS as u32,
                    CLEARTYPE_QUALITY as u32,
                    (DEFAULT_PITCH | FF_DONTCARE) as u32,
                    PCWSTR::null(),
                );
                let old_font = SelectObject(hdc, font as HGDIOBJ);
                let old_color = SetTextColor(
                    hdc,
                    if s.blue { rgb(0x30, 0x50, 0xC0) } else { rgb(0xC0, 0x30, 0x30) },
                );

                let greeting = to_wide(&format!("Hello, {}", if s.name.is_empty() { "?" } else { &s.name }));
                let mut r = rect;
                r.top += 120;
                _ = DrawTextW(
                    hdc,
                    PCWSTR::from_raw(greeting.as_ptr()),
                    -1,
                    &mut r,
                    (DT_CENTER | DT_SINGLELINE) as u32,
                );

                SetTextColor(hdc, old_color);
                SelectObject(hdc, old_font);
                _ = DeleteObject(font as HGDIOBJ);
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
