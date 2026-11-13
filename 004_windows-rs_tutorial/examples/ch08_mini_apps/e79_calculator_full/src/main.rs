//! e79 — Full Calculator: chaining, ±, %, backspace, and a keyboard
//!
//! What you learn:
//! * **Chained operations, left to right** — `12 + 3 × 4` evaluates as
//!   `(12 + 3) × 4 = 60`, NOT 24: there is no operator precedence, each
//!   operator immediately consumes the pending one. This is exactly how
//!   four-function pocket calculators (and Windows Calc in Standard
//!   mode) work — documented behavior, not a bug.
//! * The keyboard layer, via **WM_CHAR**: digits, `.`, the four
//!   operators, Enter (`\r`) = equals, Esc (`\x1b`) = C, Backspace
//!   (`\x08`) = backspace, `%`. One catch: keys go to the FOCUSED
//!   window, and the focused window is usually the last-clicked BUTTON
//!   — which eats them. The fix (used here): hand focus back to the
//!   main window after every button click, so the keyboard and mouse
//!   share one dispatcher.
//! * `±` (negate the entry) and `%` (here: entry / 100) — pure entry
//!   transforms; they never touch the accumulator.
//! * A **history line**: a STATIC above the display showing
//!   "accumulator op" — window controls as cheap read-only viewports.
//!
//! Run: `cargo run -p e79_calculator_full`

use windows::{Win32::*, core::*};

const IDC_DISPLAY: i32 = 100;
const IDC_HISTORY: i32 = 101;

const IDC_DIGIT_BASE: i32 = 110; // 110..119 -> digits 0..9
const IDC_DOT: i32 = 130;
const IDC_DIV: i32 = 131;
const IDC_MUL: i32 = 132;
const IDC_SUB: i32 = 133;
const IDC_ADD: i32 = 134;
const IDC_EQ: i32 = 135;
const IDC_CLEAR: i32 = 140;
const IDC_BACK: i32 = 141;
const IDC_SIGN: i32 = 142;
const IDC_PERCENT: i32 = 143;

const IDM_ABOUT: i32 = 5001;

/// 4 columns x 5 rows — the whole keypad in one table.
const KEYS: [[(i32, &str); 4]; 5] = [
    [(7, "7"), (8, "8"), (9, "9"), (IDC_DIV, "÷")],
    [(4, "4"), (5, "5"), (6, "6"), (IDC_MUL, "×")],
    [(1, "1"), (2, "2"), (3, "3"), (IDC_SUB, "−")],
    [(IDC_SIGN, "±"), (0, "0"), (IDC_DOT, "."), (IDC_ADD, "+")],
    [
        (IDC_CLEAR, "C"),
        (IDC_BACK, "←"),
        (IDC_PERCENT, "%"),
        (IDC_EQ, "="),
    ],
];

struct State {
    accumulator: f64,
    current: String,
    pending: Option<char>,
    /// True after = : a digit starts a NEW number, an operator continues
    /// from the result (the two post-equals behaviors every calc needs).
    just_evaluated: bool,
}

impl State {
    fn new() -> Self {
        State {
            accumulator: 0.0,
            current: String::from("0"),
            pending: None,
            just_evaluated: false,
        }
    }

    /// Chain step: fold the pending operator with the current entry.
    fn apply_pending(&mut self) {
        if let Some(op) = self.pending.take() {
            if let Ok(rhs) = self.current.parse::<f64>() {
                self.accumulator = match op {
                    '+' => self.accumulator + rhs,
                    '-' => self.accumulator - rhs,
                    '*' => self.accumulator * rhs,
                    '/' => self.accumulator / rhs,
                    _ => rhs,
                };
            }
            self.current = format_value(self.accumulator);
        }
    }
}

fn format_value(v: f64) -> String {
    if v.is_finite() && v.fract() == 0.0 && v.abs() < 1e15 {
        format!("{}", v as i64)
    } else if v.is_finite() {
        format!("{v}")
    } else {
        String::from("Error")
    }
}

fn to_wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

fn main() {
    unsafe {
        let instance = GetModuleHandleW(PCWSTR::null());
        debug_assert!(!instance.is_null());

        let wc = WNDCLASSW {
            lpfnWndProc: Some(wndproc),
            hInstance: instance,
            hCursor: LoadCursorW(None, PCWSTR(IDC_ARROW.0.cast())),
            hbrBackground: ((COLOR_BTNFACE + 1) as isize) as HBRUSH,
            lpszClassName: w!("e79_class"),
            style: (CS_HREDRAW | CS_VREDRAW) as u32,
            ..Default::default()
        };
        debug_assert!(RegisterClassW(&wc) != 0);

        let state = Box::into_raw(Box::new(State::new()));

        let hwnd = CreateWindowExW(
            0,
            w!("e79_class"),
            w!("e79 · Calculator (full) — mouse or keyboard"),
            (WS_OVERLAPPEDWINDOW | WS_VISIBLE) as u32,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            320,
            520,
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

unsafe fn update_display(hwnd: HWND) {
    unsafe {
        if let Some(state) = state_of(hwnd) {
            let entry = if state.current.is_empty() {
                "0"
            } else {
                &state.current
            };
            let hist = match state.pending {
                Some(op) => format!("{} {}", format_value(state.accumulator), symbol(op)),
                None => String::new(),
            };
            let display = GetDlgItem(Some(hwnd), IDC_DISPLAY);
            _ = SetWindowTextW(display, &HSTRING::from(entry));
            let history = GetDlgItem(Some(hwnd), IDC_HISTORY);
            _ = SetWindowTextW(history, &HSTRING::from(hist));
        }
    }
}

fn symbol(op: char) -> char {
    match op {
        '+' => '+',
        '-' => '−',
        '*' => '×',
        '/' => '÷',
        _ => '?',
    }
}

/// The ONE dispatcher both WM_COMMAND and WM_CHAR feed into — that is
/// the whole trick behind a working keyboard layer.
unsafe fn on_key(hwnd: HWND, key: i32) {
    unsafe {
        match key {
            k if (b'0' as i32..=b'9' as i32).contains(&k) => {
                if let Some(state) = state_of(hwnd) {
                    // After '=' (or after an operator's folded result), a
                    // digit replaces the display; mid-entry it appends.
                    if state.just_evaluated || state.pending.is_none() && entry_is_stale(state) {
                        state.current.clear();
                        state.just_evaluated = false;
                    }
                    if state.current == "0" || state.current == "Error" {
                        state.current.clear();
                    }
                    state.current.push(k as u8 as char);
                }
            }

            k if k == b'.' as i32 => {
                if let Some(state) = state_of(hwnd) {
                    if state.just_evaluated {
                        state.current = String::from("0");
                        state.just_evaluated = false;
                    }
                    if !state.current.contains('.') {
                        if state.current.is_empty() || state.current == "Error" {
                            state.current = String::from("0");
                        }
                        state.current.push('.');
                    }
                }
            }

            k if k == b'+' as i32 || k == b'-' as i32 || k == b'*' as i32 || k == b'/' as i32 => {
                if let Some(state) = state_of(hwnd) {
                    let op = k as u8 as char;
                    // CHAINING: a pending op folds in first — left to
                    // right, no precedence (see header). That is what
                    // makes "12 + 3 × 4" come out 60.
                    if state.pending.is_some() {
                        state.apply_pending();
                    } else if let Ok(v) = state.current.parse::<f64>() {
                        state.accumulator = v;
                    }
                    state.pending = Some(op);
                    state.just_evaluated = false;
                }
            }

            k if k == b'=' as i32 || k == b'\r' as i32 => {
                if let Some(state) = state_of(hwnd)
                    && state.pending.is_some() {
                    state.apply_pending();
                    state.just_evaluated = true;
                }
            }

            k if k == b'C' as i32 || k == b'c' as i32 || k == 0x1b => {
                if let Some(state) = state_of(hwnd) {
                    *state = State::new();
                }
            }

            0x08 => {
                // Backspace: trim the entry, never the accumulator.
                if let Some(state) = state_of(hwnd)
                    && !state.just_evaluated {
                    state.current.pop();
                    if state.current.is_empty() || state.current == "-" {
                        state.current = String::from("0");
                    }
                }
            }

            k if k == b'%' as i32 => {
                if let Some(state) = state_of(hwnd)
                    && let Ok(v) = state.current.parse::<f64>() {
                    // Simple %-model: the entry becomes entry/100
                    // ("50 + 10%" as "50 + 0.1"). The "percent OF the
                    // accumulator" model is a two-line upgrade.
                    state.current = format_value(v / 100.0);
                }
            }

            k if k == b'#' as i32 => {
                // "±" — the button sends IDC_SIGN; the keyboard analog
                // uses '#' only because WM_CHAR has no ± key.
                if let Some(state) = state_of(hwnd) {
                    negate(state);
                }
            }

            _ => return, // unknown key: no refresh
        }
        update_display(hwnd);
    }
}

fn entry_is_stale(state: &State) -> bool {
    !state.just_evaluated
        && state.pending.is_none()
        && state.current == format_value(state.accumulator)
}

fn negate(state: &mut State) {
    if state.current.starts_with('-') {
        state.current.remove(0);
    } else if state.current != "0" && state.current != "Error" {
        state.current.insert(0, '-');
    }
}

unsafe fn layout(hwnd: HWND) {
    unsafe {
        let mut rc = RECT::default();
        _ = GetClientRect(hwnd, &mut rc);
        let w = rc.right;

        let m = 12;
        let gap = 8;
        let cols = 4;
        let hist_h = 18;
        let display_h = 36;
        let bh = 44;

        let history = GetDlgItem(Some(hwnd), IDC_HISTORY);
        if !history.is_null() {
            _ = MoveWindow(history, m, m, w - 2 * m, hist_h, true);
        }
        let display = GetDlgItem(Some(hwnd), IDC_DISPLAY);
        if !display.is_null() {
            _ = MoveWindow(display, m, m + hist_h, w - 2 * m, display_h, true);
        }

        let bw = (w - 2 * m - (cols - 1) * gap) / cols;
        let grid_top = m + hist_h + display_h + m;
        for (r, row) in KEYS.iter().enumerate() {
            for (c, (id, _)) in row.iter().enumerate() {
                let key = GetDlgItem(Some(hwnd), *id);
                if !key.is_null() {
                    _ = MoveWindow(
                        key,
                        m + c as i32 * (bw + gap),
                        grid_top + r as i32 * (bh + gap),
                        bw,
                        bh,
                        true,
                    );
                }
            }
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

                // About menu — one item, keeps the keypad clean.
                let bar = CreateMenu();
                let help_menu = CreatePopupMenu();
                _ = AppendMenuW(
                    help_menu,
                    MF_STRING as u32,
                    IDM_ABOUT as usize,
                    w!("&About e79..."),
                );
                _ = AppendMenuW(bar, MF_POPUP as u32, help_menu as usize, w!("&Help"));
                _ = SetMenu(hwnd, Some(bar));

                let history = CreateWindowExW(
                    0,
                    w!("STATIC"),
                    w!(""),
                    (WS_CHILD | WS_VISIBLE | SS_RIGHT) as u32,
                    0,
                    0,
                    0,
                    0,
                    Some(hwnd),
                    Some(IDC_HISTORY as usize as HMENU),
                    Some(instance),
                    None,
                );
                debug_assert!(!history.is_null());

                let display = CreateWindowExW(
                    WS_EX_CLIENTEDGE as u32,
                    w!("EDIT"),
                    w!("0"),
                    (WS_CHILD | WS_VISIBLE | ES_READONLY | ES_RIGHT) as u32,
                    0,
                    0,
                    0,
                    0,
                    Some(hwnd),
                    Some(IDC_DISPLAY as usize as HMENU),
                    Some(instance),
                    None,
                );
                debug_assert!(!display.is_null());

                for row in KEYS.iter() {
                    for (id, label) in row.iter() {
                        let wide = to_wide(label);
                        let key = CreateWindowExW(
                            0,
                            w!("BUTTON"),
                            PCWSTR::from_raw(wide.as_ptr()),
                            (WS_CHILD | WS_VISIBLE) as u32,
                            0,
                            0,
                            0,
                            0,
                            Some(hwnd),
                            Some(*id as usize as HMENU),
                            Some(instance),
                            None,
                        );
                        debug_assert!(!key.is_null());
                    }
                }

                layout(hwnd);
                // Keyboard starts at the MAIN window (see WM_COMMAND).
                _ = SetFocus(Some(hwnd));
                0
            }

            WM_SIZE => {
                layout(hwnd);
                0
            }

            WM_CHAR => {
                // wparam is the CHARACTER code (post-TranslateMessage),
                // so digits/operators arrive as their ASCII values and
                // Enter/Esc/Backspace as \r / \x1b / \x08.
                on_key(hwnd, wparam as i32);
                0
            }

            WM_COMMAND => {
                let notification = ((wparam >> 16) & 0xffff) as u32;
                let id = (wparam & 0xffff) as i32;
                if notification != BN_CLICKED as u32 {
                    return DefWindowProcW(hwnd, message, wparam, lparam);
                }

                match id {
                    IDM_ABOUT => {
                        _ = MessageBoxW(
                            Some(hwnd),
                            w!("e79 Calculator\n\n\
                                Chained left-to-right: 12 + 3 × 4 = 60 (no precedence).\n\n\
                                Keyboard: digits, . + - * / % , # = ± ,\n\
                                Enter = equals , Backspace , Esc = C."),
                            w!("About e79"),
                            (MB_OK | MB_ICONINFORMATION) as u32,
                        );
                        // A MessageBox also steals focus; take it back.
                        _ = SetFocus(Some(hwnd));
                        return 0;
                    }

                    IDC_SIGN => {
                        if let Some(state) = state_of(hwnd) {
                            negate(state);
                        }
                    }

                    IDC_BACK => on_key(hwnd, 0x08),
                    IDC_CLEAR => on_key(hwnd, 0x1b),
                    IDC_PERCENT => on_key(hwnd, b'%' as i32),
                    IDC_EQ => on_key(hwnd, b'=' as i32),
                    IDC_DOT => on_key(hwnd, b'.' as i32),
                    IDC_ADD => on_key(hwnd, b'+' as i32),
                    IDC_SUB => on_key(hwnd, b'-' as i32),
                    IDC_MUL => on_key(hwnd, b'*' as i32),
                    IDC_DIV => on_key(hwnd, b'/' as i32),

                    d if (IDC_DIGIT_BASE..IDC_DIGIT_BASE + 10).contains(&d) => {
                        on_key(hwnd, b'0' as i32 + (d - IDC_DIGIT_BASE));
                    }

                    _ => return DefWindowProcW(hwnd, message, wparam, lparam),
                }

                update_display(hwnd);
                // THE keyboard fix: buttons keep focus after a click (and
                // would swallow every subsequent keystroke), so hand it
                // back to the main window — WM_CHAR lives there.
                _ = SetFocus(Some(hwnd));
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
