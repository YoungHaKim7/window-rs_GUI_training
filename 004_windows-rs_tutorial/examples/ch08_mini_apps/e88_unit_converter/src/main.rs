//! e88 — Unit Converter: live conversion as you type
//!
//! What you learn:
//! * `EN_CHANGE` — an EDIT fires it every time its text changes (typing,
//!   paste, programmatic SetWindowTextW), the basis of "live" forms
//! * `CBN_SELCHANGE` — the combobox's "user picked something else"
//! * Assembling a small data-driven app: the unit table is the entire
//!   "business logic"; the window code just wires events to `convert()`
//! * The difference between *ratio* units (length: multiply by a factor)
//!   and *affine* units (temperature: °C and °F don't share a zero point —
//!   a factor table cannot express that; see the note at the bottom)
//!
//! Type in the left box: the right box converts as you type. Pick
//! different units; Swap exchanges them.
//!
//! New Win32 APIs: nothing new — which is the point: by now you know all
//! the parts; apps are just arrangements of them.
//!
//! Run: `cargo run -p e88_unit_converter`

use windows::{Win32::*, core::*};

/// No LOWORD/HIWORD helpers in this crate version — the inline forms.
fn loword(v: usize) -> i32 {
    (v & 0xffff) as u16 as i32
}
fn hiword(v: usize) -> i32 {
    ((v >> 16) & 0xffff) as u16 as i32
}

/// Control ids (WM_COMMAND's LOWORD).
const IDC_VALUE_IN: i32 = 1001;
const IDC_UNIT_IN: i32 = 1002;
const IDC_VALUE_OUT: i32 = 1003;
const IDC_UNIT_OUT: i32 = 1004;
const IDC_SWAP: i32 = 1005;

/// The entire unit system: name + length in meters. Converting is always
///   out = in * from_factor / to_factor
const UNITS: [(&str, f64); 7] = [
    ("millimeter", 0.001),
    ("centimeter", 0.01),
    ("meter", 1.0),
    ("kilometer", 1000.0),
    ("inch", 0.0254),
    ("foot", 0.3048),
    ("mile", 1609.344),
];

#[derive(Default)]
struct State {
    edit_in: HWND,
    combo_in: HWND,
    edit_out: HWND,
    combo_out: HWND,
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
            lpszClassName: w!("e88_class"),
            style: (CS_HREDRAW | CS_VREDRAW) as u32,
            ..Default::default()
        };
        debug_assert!(RegisterClassW(&wc) != 0);

        let hwnd = CreateWindowExW(
            0,
            w!("e88_class"),
            w!("e88 · Unit Converter"),
            (WS_OVERLAPPEDWINDOW | WS_VISIBLE) as u32,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            460,
            300,
            None,
            None,
            Some(instance),
            None,
        );
        debug_assert!(!hwnd.is_null());

        let state = Box::into_raw(Box::<State>::default());
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, state as isize);

        // Children created after the state pointer is attached (the e13
        // flavor; e65's WM_NCCREATE flavor would allow doing it in
        // WM_CREATE — same result here).
        let children = (
            CreateWindowExW(
                0,
                w!("EDIT"),
                w!("1"),
                (WS_CHILD | WS_VISIBLE | WS_BORDER | ES_AUTOHSCROLL) as u32,
                0,
                0,
                10,
                10,
                Some(hwnd),
                Some(IDC_VALUE_IN as usize as HMENU),
                Some(instance),
                None,
            ),
            CreateWindowExW(
                0,
                w!("COMBOBOX"),
                PCWSTR::null(),
                (WS_CHILD | WS_VISIBLE | CBS_DROPDOWNLIST) as u32,
                0,
                0,
                10,
                200,
                Some(hwnd),
                Some(IDC_UNIT_IN as usize as HMENU),
                Some(instance),
                None,
            ),
            CreateWindowExW(
                0,
                w!("EDIT"),
                PCWSTR::null(),
                // ES_READONLY: the output box refuses typing; a plain
                // SendMessageW(EM_SETREADONLY) later would work too.
                (WS_CHILD | WS_VISIBLE | WS_BORDER | ES_READONLY) as u32,
                0,
                0,
                10,
                10,
                Some(hwnd),
                Some(IDC_VALUE_OUT as usize as HMENU),
                Some(instance),
                None,
            ),
            CreateWindowExW(
                0,
                w!("COMBOBOX"),
                PCWSTR::null(),
                (WS_CHILD | WS_VISIBLE | CBS_DROPDOWNLIST) as u32,
                0,
                0,
                10,
                200,
                Some(hwnd),
                Some(IDC_UNIT_OUT as usize as HMENU),
                Some(instance),
                None,
            ),
        );

        let s = &mut *(GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut State);
        (s.edit_in, s.combo_in, s.edit_out, s.combo_out) = children;

        for (combo, pick) in [(s.combo_in, 2usize), (s.combo_out, 5usize)] {
            for (name, _) in UNITS {
                let wide = to_wide(name);
                // LPARAM is a plain isize alias — no tuple constructor.
                _ = SendMessageW(combo, CB_ADDSTRING as u32, 0, wide.as_ptr() as isize);
            }
            // Programmatic selection: no CBN_SELCHANGE fires, so we
            // must trigger the initial conversion ourselves below.
            _ = SendMessageW(combo, CB_SETCURSEL as u32, pick as WPARAM, 0);
        }

        // Button too — position fixed up by the first WM_SIZE, which
        // Windows sends during CreateWindowExW of this very child.
        _ = CreateWindowExW(
            0,
            w!("BUTTON"),
            w!("Swap"),
            (WS_CHILD | WS_VISIBLE | BS_PUSHBUTTON) as u32,
            0,
            0,
            10,
            10,
            Some(hwnd),
            Some(IDC_SWAP as usize as HMENU),
            Some(instance),
            None,
        );

        convert(hwnd);

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

unsafe fn window_text(hwnd: HWND) -> String {
    unsafe {
        let len = GetWindowTextLengthW(hwnd);
        let mut buf = vec![0u16; (len + 1) as usize];
        let copied = GetWindowTextW(hwnd, PWSTR(buf.as_mut_ptr()), len + 1);
        String::from_utf16_lossy(&buf[..copied.max(0) as usize])
    }
}

/// Read the left side, write the converted right side.
unsafe fn convert(hwnd: HWND) {
    unsafe {
        let Some(s) = state_of(hwnd) else { return };

        let out = match window_text(s.edit_in).trim().parse::<f64>() {
            Ok(value) => {
                let from = SendMessageW(s.combo_in, CB_GETCURSEL as u32, 0, 0) as usize;
                let to = SendMessageW(s.combo_out, CB_GETCURSEL as u32, 0, 0) as usize;
                // CB_ERR (-1) means "nothing selected" — guard the index.
                if from >= UNITS.len() || to >= UNITS.len() {
                    String::new()
                } else {
                    let meters = value * UNITS[from].1;
                    let result = meters / UNITS[to].1;
                    // Trim trailing zeros so 1.500000 reads as 1.5.
                    let text = format!("{result:.6}");
                    text.trim_end_matches('0').trim_end_matches('.').to_string()
                }
            }
            // Empty input clears the box; garbage shows the dash.
            Err(_) if window_text(s.edit_in).trim().is_empty() => String::new(),
            Err(_) => "—".to_string(),
        };
        _ = SetWindowTextW(s.edit_out, &HSTRING::from(out));
    }
}

extern "system" fn wndproc(hwnd: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe {
        match message as i32 {
            WM_COMMAND => {
                let id = loword(wparam);
                let code = hiword(wparam);
                match (id, code) {
                    // Every text change in the input box: reconvert. This
                    // fires per keystroke — the work here is trivial, but
                    // remember this hook when "live" means "expensive".
                    (IDC_VALUE_IN, EN_CHANGE) => {
                        convert(hwnd);
                        0
                    }
                    (IDC_UNIT_IN, CBN_SELCHANGE) | (IDC_UNIT_OUT, CBN_SELCHANGE) => {
                        convert(hwnd);
                        0
                    }
                    (IDC_SWAP, BN_CLICKED) => {
                        if let Some(s) = state_of(hwnd) {
                            // Swap the selections and move the result into
                            // the input box, then reconvert (which also
                            // updates the output for the new direction).
                            let a = SendMessageW(s.combo_in, CB_GETCURSEL as u32, 0, 0);
                            let b = SendMessageW(s.combo_out, CB_GETCURSEL as u32, 0, 0);
                            _ = SendMessageW(s.combo_in, CB_SETCURSEL as u32, b as WPARAM, 0);
                            _ = SendMessageW(s.combo_out, CB_SETCURSEL as u32, a as WPARAM, 0);
                            let right = window_text(s.edit_out);
                            _ = SetWindowTextW(s.edit_in, &HSTRING::from(right));
                            convert(hwnd);
                        }
                        0
                    }
                    _ => DefWindowProcW(hwnd, message, wparam, lparam),
                }
            }
            WM_SIZE => {
                if let Some(s) = state_of(hwnd) {
                    let (w, h) = (
                        (lparam & 0xffff) as u16 as i32,
                        ((lparam >> 16) & 0xffff) as u16 as i32,
                    );
                    let margin = 24;
                    let gap = 16;
                    let col = (w - 2 * margin - gap) / 2;
                    let row_y = [h / 2 - 90, h / 2 - 30];
                    let box_h = 28;
                    // Combos need headroom for the dropped list — height
                    // here is the open height, not the closed one.
                    _ = MoveWindow(s.edit_in, margin, row_y[0], col, box_h, true);
                    _ = MoveWindow(s.combo_in, margin, row_y[1], col, 200, true);
                    _ = MoveWindow(s.edit_out, margin + col + gap, row_y[0], col, box_h, true);
                    _ = MoveWindow(s.combo_out, margin + col + gap, row_y[1], col, 200, true);
                    _ = MoveWindow(
                        GetDlgItem(Some(hwnd), IDC_SWAP),
                        margin,
                        h - margin - 32,
                        100,
                        32,
                        true,
                    );
                }
                0
            }
            WM_PAINT => {
                let mut ps = PAINTSTRUCT::default();
                let hdc = BeginPaint(hwnd, &mut ps);
                _ = SetBkMode(hdc, TRANSPARENT);
                _ = TextOutW(
                    hdc,
                    24,
                    24,
                    w!("Type a length — it converts as you type. Swap flips the units."),
                    -1,
                );
                _ = TextOutW(
                    hdc,
                    24,
                    44,
                    w!("(Temperature needs an affine formula, not a factor — exercise!)"),
                    -1,
                );
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
