//! e78 — Basic Calculator: a button grid, and an app that is mostly logic
//!
//! What you learn:
//! * **Grid layout math in WM_SIZE** — the calculator's buttons are
//!   placed by arithmetic (`x = margin + col * (bw + gap)`), recomputed
//!   on every resize. No layout engine, no dialogs: a 2D array of
//!   controls and one loop is the whole "layout system".
//! * The **two-operand accumulator** model every pocket calculator uses:
//!   `accumulator <op> current` — digits append to `current`, an operator
//!   computes any PENDING operation first (so `2 + 3 * 4` is 20 here:
//!   left-to-right, no precedence — documented, deliberate, how real
//!   four-function calculators behave; e79 revisits), `=` finishes.
//! * A read-only EDIT as display (`ES_READONLY | ES_RIGHT`): the user
//!   can't type into it, select/copy still works, and we own every
//!   character via SetWindowTextW.
//! * Buttons as a data table: one (label, id, kind) array drives
//!   creation, layout, AND dispatch — adding a button is one line.
//!
//! Run: `cargo run -p e78_calculator_basic`

use windows::{Win32::*, core::*};

const IDC_DISPLAY: i32 = 100;

// Button ids: one constant per key, grouped by kind.
const IDC_DIGIT_BASE: i32 = 110; // 110..119 -> digits 0..9
const IDC_DOT: i32 = 130;
const IDC_DIV: i32 = 131;
const IDC_MUL: i32 = 132;
const IDC_SUB: i32 = 133;
const IDC_ADD: i32 = 134;
const IDC_EQ: i32 = 135;
const IDC_CLEAR: i32 = 140;

/// The keypad, row by row — single source of truth for creation/layout.
const KEYS: [[(i32, &str); 4]; 4] = [
    [(7, "7"), (8, "8"), (9, "9"), (IDC_DIV, "÷")],
    [(4, "4"), (5, "5"), (6, "6"), (IDC_MUL, "×")],
    [(1, "1"), (2, "2"), (3, "3"), (IDC_SUB, "−")],
    [(0, "0"), (IDC_DOT, "."), (IDC_EQ, "="), (IDC_ADD, "+")],
];

struct State {
    /// The left operand (result of everything so far).
    accumulator: f64,
    /// The right operand, as typed — kept as a string so "." and
    /// leading zeros behave like a real calculator's entry line.
    current: String,
    /// Operator waiting for the right operand.
    pending: Option<char>,
}

impl State {
    fn new() -> Self {
        State {
            accumulator: 0.0,
            current: String::from("0"),
            pending: None,
        }
    }

    fn apply_pending(&mut self) {
        if let Some(op) = self.pending.take() {
            if let Ok(rhs) = self.current.parse::<f64>() {
                self.accumulator = match op {
                    '+' => self.accumulator + rhs,
                    '-' => self.accumulator - rhs,
                    '×' | '*' => self.accumulator * rhs,
                    '÷' | '/' => self.accumulator / rhs, // f64: /0.0 = inf
                    _ => rhs,
                };
            }
            self.current = format_value(self.accumulator);
        }
    }
}

/// Display formatting: integers without ".0", non-finite (÷0) as "Error".
fn format_value(v: f64) -> String {
    if v.is_finite() && v.fract() == 0.0 && v.abs() < 1e15 {
        format!("{}", v as i64)
    } else if v.is_finite() {
        format!("{v}")
    } else {
        String::from("Error")
    }
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
            lpszClassName: w!("e78_class"),
            style: (CS_HREDRAW | CS_VREDRAW) as u32,
            ..Default::default()
        };
        debug_assert!(RegisterClassW(&wc) != 0);

        let state = Box::into_raw(Box::new(State::new()));

        let hwnd = CreateWindowExW(
            0,
            w!("e78_class"),
            w!("e78 · Calculator"),
            (WS_OVERLAPPEDWINDOW | WS_VISIBLE) as u32,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            300,
            460,
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
        // Show the entry line while typing, the pending accumulator
        // otherwise — exactly what a four-function calc's LCD does.
        let text = if let Some(state) = state_of(hwnd) {
            if state.current.is_empty() {
                String::from("0")
            } else {
                state.current.clone()
            }
        } else {
            String::from("0")
        };
        let display = GetDlgItem(Some(hwnd), IDC_DISPLAY);
        _ = SetWindowTextW(display, &HSTRING::from(text));
    }
}

/// All geometry, derived only from the client size — so WM_CREATE and
/// WM_SIZE share it (the e70 lesson, as a grid).
unsafe fn layout(hwnd: HWND) {
    unsafe {
        let mut rc = RECT::default();
        _ = GetClientRect(hwnd, &mut rc);
        let w = rc.right; // height is unused: the grid is top-anchored

        let m = 12; // outer margin
        let gap = 8; // gutter between keys
        let cols = 4;
        let rows = KEYS.len() as i32;

        let bw = (w - 2 * m - (cols - 1) * gap) / cols; // key width
        let bh = 46; // key height
        let display_h = 44;
        let grid_top = m + display_h + m;

        let display = GetDlgItem(Some(hwnd), IDC_DISPLAY);
        if !display.is_null() {
            _ = MoveWindow(display, m, m, w - 2 * m, display_h, true);
        }

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

        // C spans the full width under the grid.
        let clear = GetDlgItem(Some(hwnd), IDC_CLEAR);
        if !clear.is_null() {
            let y = grid_top + rows * (bh + gap);
            _ = MoveWindow(clear, m, y, w - 2 * m, bh, true);
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

                // The display: read-only so no keyboard path exists into
                // it (e79 adds one at the window level).
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

                // Keys from the table — same loop later re-lays them out.
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
                let clear = CreateWindowExW(
                    0,
                    w!("BUTTON"),
                    w!("C"),
                    (WS_CHILD | WS_VISIBLE) as u32,
                    0,
                    0,
                    0,
                    0,
                    Some(hwnd),
                    Some(IDC_CLEAR as usize as HMENU),
                    Some(instance),
                    None,
                );
                debug_assert!(!clear.is_null());

                layout(hwnd);
                0
            }

            WM_SIZE => {
                layout(hwnd);
                0
            }

            WM_COMMAND => {
                let notification = ((wparam >> 16) & 0xffff) as u32;
                let id = (wparam & 0xffff) as i32;
                if notification != BN_CLICKED as u32 {
                    return DefWindowProcW(hwnd, message, wparam, lparam);
                }

                // App logic: everything funnels into State mutations, then
                // one display refresh at the end.
                let needs_refresh = true;
                match id {
                    // Digits 110..119 append; the entry resets after an
                    // operator result (flagged by current == formatted
                    // accumulator — simple and predictable for a tutorial).
                    d if (IDC_DIGIT_BASE..IDC_DIGIT_BASE + 10).contains(&d) => {
                        if let Some(state) = state_of(hwnd) {
                            let digit = (d - IDC_DIGIT_BASE) as u8;
                            let stale = state.pending.is_none()
                                && state.current == format_value(state.accumulator)
                                && state.accumulator != 0.0;
                            if state.current == "0" || state.current == "Error" || stale {
                                state.current.clear();
                            }
                            state.current.push((b'0' + digit) as char);
                        }
                    }

                    IDC_DOT => {
                        if let Some(state) = state_of(hwnd)
                            && !state.current.contains('.')
                        {
                            if state.current.is_empty() || state.current == "Error" {
                                state.current = String::from("0");
                            }
                            state.current.push('.');
                        }
                    }

                    IDC_ADD | IDC_SUB | IDC_MUL | IDC_DIV => {
                        if let Some(state) = state_of(hwnd) {
                            // A pending op computes first (left-to-right);
                            // otherwise the entry becomes the accumulator.
                            if state.pending.is_some() {
                                state.apply_pending();
                            } else if let Ok(v) = state.current.parse::<f64>() {
                                state.accumulator = v;
                            }
                            state.pending = Some(match id {
                                IDC_ADD => '+',
                                IDC_SUB => '-',
                                IDC_MUL => '*',
                                _ => '/',
                            });
                            state.current = format_value(state.accumulator);
                        }
                    }

                    IDC_EQ => {
                        if let Some(state) = state_of(hwnd)
                            && state.pending.is_some()
                        {
                            if state.current.is_empty() {
                                // "2 + =" -> repeat-free no-op guard
                                state.current = format_value(state.accumulator);
                            }
                            state.apply_pending();
                        }
                    }

                    IDC_CLEAR => {
                        if let Some(state) = state_of(hwnd) {
                            *state = State::new();
                        }
                    }

                    _ => {
                        return DefWindowProcW(hwnd, message, wparam, lparam);
                    }
                }

                if needs_refresh {
                    update_display(hwnd);
                }
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

fn to_wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}
