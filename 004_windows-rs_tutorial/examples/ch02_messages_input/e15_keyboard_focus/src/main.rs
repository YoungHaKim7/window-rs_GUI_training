//! e15 — Keyboard Focus & the Caret: a one-line text box, by hand
//!
//! What you learn:
//! * *Focus*: exactly one window in the system receives keyboard messages.
//!   `WM_SETFOCUS` / `WM_KILLFOCUS` tell you when you gain and lose it.
//! * The *caret* (the blinking text cursor) is a shared system resource:
//!   create it when you GET focus, destroy it when you LOSE focus — never
//!   hold it while unfocused.
//! * Building text input from `WM_CHAR`: append characters, handle
//!   Backspace, measure the text to place the caret
//! * `GetTextExtentPoint32W` — "how many pixels wide is this string?"
//!
//! Click another window and back: the border color and caret follow the
//! focus. Then appreciate e43, where the EDIT control does all this for you.
//!
//! New Win32 APIs: `CreateCaret`, `SetCaretPos`, `ShowCaret`, `HideCaret`,
//! `DestroyCaret`, `GetTextExtentPoint32W`, `FrameRect`.
//!
//! Run: `cargo run -p e15_keyboard_focus`

use windows::{Win32::*, core::*};

const TEXT_X: i32 = 32; // where the text line starts
const TEXT_Y: i32 = 60;
const LINE_HEIGHT: i32 = 24;

#[derive(Default)]
struct State {
    text: String,
    focused: bool,
}

fn main() {
    unsafe {
        let instance = GetModuleHandleW(PCWSTR::null());
        debug_assert!(!instance.is_null());

        let wc = WNDCLASSW {
            lpfnWndProc: Some(wndproc),
            hInstance: instance,
            // An I-beam says "you can type here".
            hCursor: LoadCursorW(None, PCWSTR(IDC_IBEAM.0.cast())),
            hbrBackground: ((COLOR_WINDOW + 1) as isize) as HBRUSH,
            lpszClassName: w!("e15_class"),
            style: (CS_HREDRAW | CS_VREDRAW) as u32,
            ..Default::default()
        };
        debug_assert!(RegisterClassW(&wc) != 0);

        let hwnd = CreateWindowExW(
            0,
            w!("e15_class"),
            w!("e15 · Keyboard Focus — type something"),
            (WS_OVERLAPPEDWINDOW | WS_VISIBLE) as u32,
            CW_USEDEFAULT, CW_USEDEFAULT, 640, 220,
            None, None, Some(instance), None,
        );
        debug_assert!(!hwnd.is_null());

        let state = Box::into_raw(Box::<State>::default());
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, state as isize);

        // The window was created (and activated) BEFORE the state pointer
        // existed, so its first WM_SETFOCUS took the default path. Ask for
        // focus again now that the state is attached — this delivers a
        // fresh WM_SETFOCUS that our handler can act on.
        _ = SetFocus(Some(hwnd));

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

/// Pixel width of the current text in the window's default font — that's
/// where the caret belongs.
unsafe fn text_width(hwnd: HWND, text: &str) -> i32 {
    unsafe {
        let hdc = GetDC(Some(hwnd));
        let wide: Vec<u16> = text.encode_utf16().collect();
        let mut size = SIZE::default();
        _ = GetTextExtentPoint32W(hdc, PCWSTR::from_raw(wide.as_ptr()), wide.len() as i32, &mut size);
        ReleaseDC(Some(hwnd), hdc);
        size.cx
    }
}

extern "system" fn wndproc(hwnd: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe {
        let Some(state) = state_of(hwnd) else {
            return DefWindowProcW(hwnd, message, wparam, lparam);
        };

        match message as i32 {
            WM_SETFOCUS => {
                // We now own the keyboard. The caret is a system-wide
                // singleton: create ours (2px wide, one line tall), put it
                // after the text, and make it visible.
                state.focused = true;
                _ = CreateCaret(hwnd, None, 2, LINE_HEIGHT);
                _ = SetCaretPos(TEXT_X + text_width(hwnd, &state.text), TEXT_Y);
                _ = ShowCaret(Some(hwnd));
                _ = InvalidateRect(Some(hwnd), None, true);
                0
            }
            WM_KILLFOCUS => {
                // Someone else gets the keyboard: give the caret back.
                state.focused = false;
                _ = HideCaret(Some(hwnd));
                _ = DestroyCaret();
                _ = InvalidateRect(Some(hwnd), None, true);
                0
            }
            WM_CHAR => {
                match wparam as u32 {
                    0x08 => {
                        // Backspace arrives as a control character.
                        state.text.pop();
                    }
                    // Ignore the other control characters (Enter, Esc, Tab).
                    unit if unit < 0x20 => {}
                    unit => {
                        if let Some(ch) = char::from_u32(unit) {
                            state.text.push(ch);
                        }
                    }
                }
                // The text changed, so the caret moves and we repaint.
                _ = SetCaretPos(TEXT_X + text_width(hwnd, &state.text), TEXT_Y);
                _ = InvalidateRect(Some(hwnd), None, true);
                0
            }
            WM_PAINT => {
                let mut ps = PAINTSTRUCT::default();
                let hdc = BeginPaint(hwnd, &mut ps);

                let mut rect = RECT::default();
                _ = GetClientRect(hwnd, &mut rect);

                // Focus indicator: a highlight-colored frame when focused,
                // a muted gray one when not.
                let frame = GetSysColorBrush(if state.focused { COLOR_HIGHLIGHT } else { COLOR_GRAYTEXT });
                let mut border = rect;
                border.left += 16;
                border.top += 44;
                border.right -= 16;
                border.bottom = TEXT_Y + LINE_HEIGHT + 12;
                FrameRect(hdc, &border, frame);

                let status = if state.focused {
                    w!("Focused — keystrokes come here")
                } else {
                    w!("Not focused — click to type")
                };
                _ = TextOutW(hdc, 16, 16, status, -1);

                let wide = to_wide(&state.text);
                _ = TextOutW(hdc, TEXT_X, TEXT_Y, PCWSTR::from_raw(wide.as_ptr()), -1);

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
