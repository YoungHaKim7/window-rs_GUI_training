//! e14 — Keyboard Input: WM_KEYDOWN vs WM_CHAR vs WM_SYSKEYDOWN
//!
//! What you learn:
//! * `WM_KEYDOWN` — raw *virtual key codes* (VK_*): "the A key", not "a".
//!   wparam = VK code; lparam packs repeat count, scan code, and the
//!   "was already down" bit (auto-repeat!)
//! * `WM_CHAR` — produced by `TranslateMessage` from KEYDOWN + the current
//!   keyboard layout: actual *characters* ("a", "A", "!", "é"). This is
//!   the one you draw text from.
//! * `WM_SYSKEYDOWN` — keys pressed *together with Alt*. You almost always
//!   let DefWindowProcW handle them afterwards, or Alt+F4 breaks.
//! * Try: hold Shift and press 1 → KEYDOWN reports VK '1', CHAR reports '!'
//!
//! The window shows the last few events (state pattern from e13 again);
//! the console mirrors them.
//!
//! Run: `cargo run -p e14_keyboard_input`

use windows::{Win32::*, core::*};

/// Rolling log of recent key events, newest last.
#[derive(Default)]
struct State {
    log: Vec<String>,
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
            lpszClassName: w!("e14_class"),
            style: (CS_HREDRAW | CS_VREDRAW) as u32,
            ..Default::default()
        };
        debug_assert!(RegisterClassW(&wc) != 0);

        let hwnd = CreateWindowExW(
            0,
            w!("e14_class"),
            w!("e14 · Keyboard — press keys (try Shift+1)"),
            (WS_OVERLAPPEDWINDOW | WS_VISIBLE) as u32,
            CW_USEDEFAULT, CW_USEDEFAULT, 680, 380,
            None, None, Some(instance), None,
        );
        debug_assert!(!hwnd.is_null());

        let state = Box::into_raw(Box::<State>::default());
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, state as isize);

        let mut message = MSG::default();
        while GetMessageW(&mut message, None, 0, 0).0 > 0 {
            _ = TranslateMessage(&message); // KEYDOWN -> CHAR happens here
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

/// A friendly name for the virtual keys worth naming. Constants are legal
/// match patterns — only `as` casts aren't.
fn vk_name(vk: i32) -> String {
    let name = match vk {
        VK_RETURN => "Enter",
        VK_ESCAPE => "Esc",
        VK_BACK => "Backspace",
        VK_TAB => "Tab",
        VK_SPACE => "Space",
        VK_SHIFT => "Shift",
        VK_CONTROL => "Ctrl",
        VK_MENU => "Alt",
        VK_LEFT => "Left",
        VK_RIGHT => "Right",
        VK_UP => "Up",
        VK_DOWN => "Down",
        VK_DELETE => "Delete",
        _ => return format!("vk {vk:#04x}"),
    };
    name.to_string()
}

fn rgb(red: u8, green: u8, blue: u8) -> COLORREF {
    (red as u32) | ((green as u32) << 8) | ((blue as u32) << 16)
}

extern "system" fn wndproc(hwnd: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe {
        let Some(state) = state_of(hwnd) else {
            return DefWindowProcW(hwnd, message, wparam, lparam);
        };

        match message as i32 {
            WM_KEYDOWN => {
                // lparam layout for key messages:
                //   bits  0..15  repeat count
                //   bits 16..23  OEM scan code
                //   bit  30      1 = key was already down (auto-repeat)
                let repeat = (lparam & 0xffff) as usize;
                let held = lparam & (1 << 30) != 0;
                let vk = wparam as i32;

                let detail = if held { " (auto-repeat)" } else { "" };
                let label = match vk {
                    0x30..=0x39 | 0x41..=0x5A => {
                        format!("'{}'", vk as u8 as char)
                    }
                    _ => vk_name(vk),
                };
                let line = format!("WM_KEYDOWN   {label:<12} repeat={repeat}{detail}");
                state.log.push(line.clone());
                println!("{line}");
                if state.log.len() > 12 {
                    state.log.remove(0);
                }
                _ = InvalidateRect(Some(hwnd), None, true);
                0
            }
            WM_CHAR => {
                // wparam is a UTF-16 code unit, translated from the key +
                // layout by TranslateMessage. THIS is the message for text.
                let unit = wparam as u32;
                let shown = char::from_u32(unit)
                    .map(|c| if c.is_control() { format!("U+{unit:04X}") } else { c.to_string() })
                    .unwrap_or_else(|| format!("U+{unit:04X}"));
                let line = format!("WM_CHAR      '{shown}'");
                state.log.push(line.clone());
                println!("{line}");
                if state.log.len() > 12 {
                    state.log.remove(0);
                }
                _ = InvalidateRect(Some(hwnd), None, true);
                0
            }
            WM_KEYUP => {
                state.log.push(format!("WM_KEYUP     {}", vk_name(wparam as i32)));
                if state.log.len() > 12 {
                    state.log.remove(0);
                }
                _ = InvalidateRect(Some(hwnd), None, true);
                0
            }
            WM_SYSKEYDOWN => {
                // Alt+key. Log it, then hand it to DefWindowProc — swallow
                // this and Alt+F4, Alt+Space, the system menu all stop
                // working (try replacing the last line of this arm with 0!).
                state.log.push(format!("WM_SYSKEYDOWN {} (with Alt)", vk_name(wparam as i32)));
                if state.log.len() > 12 {
                    state.log.remove(0);
                }
                _ = InvalidateRect(Some(hwnd), None, true);
                DefWindowProcW(hwnd, message, wparam, lparam)
            }
            WM_PAINT => {
                let mut ps = PAINTSTRUCT::default();
                let hdc = BeginPaint(hwnd, &mut ps);

                // Monospace font keeps the log columns aligned.
                let font = CreateFontW(
                    20, 0, 0, 0, FW_NORMAL, 0, 0, 0, DEFAULT_CHARSET as u32,
                    OUT_DEFAULT_PRECIS as u32, CLIP_DEFAULT_PRECIS as u32,
                    CLEARTYPE_QUALITY as u32, (DEFAULT_PITCH | FF_MODERN) as u32,
                    w!("Consolas"),
                );
                let old_font = SelectObject(hdc, font as HGDIOBJ);
                _ = SetBkMode(hdc, TRANSPARENT);

                for (i, line) in state.log.iter().enumerate() {
                    let y = 24 + i as i32 * 26;
                    // Color-code by message kind for instant recognition.
                    let color = if line.starts_with("WM_CHAR") {
                        rgb(0x20, 0x70, 0x20) // chars: green
                    } else if line.starts_with("WM_SYS") {
                        rgb(0x90, 0x20, 0x90) // sys: purple
                    } else {
                        rgb(0x20, 0x20, 0x80) // raw keys: blue
                    };
                    _ = SetTextColor(hdc, color);
                    let wide = to_wide(line);
                    _ = TextOutW(hdc, 24, y, PCWSTR::from_raw(wide.as_ptr()), -1);
                }

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

fn to_wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}
