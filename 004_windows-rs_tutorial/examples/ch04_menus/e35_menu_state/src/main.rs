//! e35 — Menu State: check marks, radio groups, grayed items
//!
//! What you learn:
//! * `CheckMenuItem` — the ✓ toggle: `MF_BYCOMMAND | MF_CHECKED` /
//!   `MF_UNCHECKED` on a normal item. Menus don't remember anything —
//!   *you* track the boolean in state and stamp it into the item each
//!   time it flips (the menu is a view, your State is the model)
//! * `CheckMenuRadioItem(menu, first, last, chosen, MF_BYCOMMAND)` — the
//!   bullet ● group: Windows enforces "exactly one of these ids checked",
//!   which is exactly a radio group
//! * `EnableMenuItem` — gray an item out (`MF_GRAYED`: visible but
//!   unclickable, the honest way to say "not applicable now") vs
//!   re-enabling (`MF_ENABLED`)
//! * The payoff of the window-state pattern: menu choices mutate State,
//!   WM_PAINT renders State — the same loop as e13, with menus as the
//!   input surface
//!
//! Watch the sentence below the menu change as you toggle Format items.
//!
//! New Win32 APIs: `CheckMenuItem`, `CheckMenuRadioItem`,
//! `EnableMenuItem` (winuser).
//!
//! Run: `cargo run -p e35_menu_state`

use windows::{Win32::*, core::*};

// Menu item ids (MF_BYCOMMAND identifies items by these).
const ID_BOLD: usize = 3001;
const ID_ITALIC: usize = 3002;
const ID_SMALL: usize = 3003;
const ID_MEDIUM: usize = 3004;
const ID_LARGE: usize = 3005;
const ID_ALLOW_UL: usize = 3006;
const ID_UNDERLINE: usize = 3007;

#[derive(Default)]
struct State {
    bold: bool,
    italic: bool,
    underline_allowed: bool, // gates the Underline item (grayed when false)
    underline: bool,
    font_h: i32, // requested font height (px)
    /// The Format popup handle, kept so WM_COMMAND can update item marks.
    format_menu: HMENU,
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
            lpszClassName: w!("e35_class"),
            style: (CS_HREDRAW | CS_VREDRAW) as u32,
            ..Default::default()
        };
        debug_assert!(RegisterClassW(&wc) != 0);

        let hwnd = CreateWindowExW(
            0,
            w!("e35_class"),
            w!("e35 · Menu State — checks, radios, graying"),
            (WS_OVERLAPPEDWINDOW | WS_VISIBLE) as u32,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            680,
            380,
            None,
            None,
            Some(instance),
            None,
        );
        debug_assert!(!hwnd.is_null());

        // ---- Menu ------------------------------------------------------
        let bar = CreateMenu();
        let format = CreatePopupMenu();
        _ = AppendMenuW(format, MF_STRING as u32, ID_BOLD, w!("&Bold"));
        _ = AppendMenuW(format, MF_STRING as u32, ID_ITALIC, w!("&Italic"));
        _ = AppendMenuW(format, MF_SEPARATOR as u32, 0, w!(""));
        _ = AppendMenuW(format, MF_STRING as u32, ID_SMALL, w!("Small"));
        _ = AppendMenuW(format, MF_STRING as u32, ID_MEDIUM, w!("Medium"));
        _ = AppendMenuW(format, MF_STRING as u32, ID_LARGE, w!("Large"));
        _ = AppendMenuW(format, MF_SEPARATOR as u32, 0, w!(""));
        _ = AppendMenuW(format, MF_STRING as u32, ID_ALLOW_UL, w!("Allow underline"));
        _ = AppendMenuW(format, MF_STRING as u32, ID_UNDERLINE, w!("&Underline"));
        _ = AppendMenuW(bar, MF_POPUP as u32, format as usize, w!("&Format"));
        _ = SetMenu(hwnd, Some(bar));

        // ---- State, then initial menu marks ---------------------------
        let state = Box::into_raw(Box::new(State {
            font_h: 32, // "Medium"
            format_menu: format,
            ..Default::default()
        }));
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, state as isize);

        // Seed the menu to match State: Medium gets the radio bullet,
        // Underline starts grayed because Allow underline is off.
        _ = CheckMenuRadioItem(
            format,
            ID_SMALL as u32,  // first id of the group
            ID_LARGE as u32,  // last id of the group
            ID_MEDIUM as u32, // the one that gets the ●
            MF_BYCOMMAND as u32,
        );
        _ = EnableMenuItem(
            format,
            ID_UNDERLINE as u32,
            (MF_BYCOMMAND | MF_GRAYED) as u32,
        );

        let mut message = MSG::default();
        while GetMessageW(&mut message, None, 0, 0).0 > 0 {
            _ = TranslateMessage(&message);
            DispatchMessageW(&message);
        }

        _ = DestroyMenu(bar);
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

extern "system" fn wndproc(hwnd: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe {
        let Some(state) = state_of(hwnd) else {
            return DefWindowProcW(hwnd, message, wparam, lparam);
        };

        match message as i32 {
            WM_COMMAND => {
                let id = (wparam & 0xffff) as usize;
                // Helper: restamp a ✓ item after its boolean flipped.
                let mark = |id: usize, on: bool| {
                    _ = CheckMenuItem(
                        state.format_menu,
                        id as u32,
                        (MF_BYCOMMAND | if on { MF_CHECKED } else { MF_UNCHECKED }) as u32,
                    );
                };

                match id {
                    ID_BOLD => {
                        state.bold = !state.bold;
                        mark(ID_BOLD, state.bold);
                    }
                    ID_ITALIC => {
                        state.italic = !state.italic;
                        mark(ID_ITALIC, state.italic);
                    }
                    ID_SMALL | ID_MEDIUM | ID_LARGE => {
                        state.font_h = match id {
                            ID_SMALL => 20,
                            ID_MEDIUM => 32,
                            _ => 48,
                        };
                        // Radio semantics in one call: Windows clears the
                        // other two and dots this one. No bookkeeping!
                        _ = CheckMenuRadioItem(
                            state.format_menu,
                            ID_SMALL as u32,
                            ID_LARGE as u32,
                            id as u32,
                            MF_BYCOMMAND as u32,
                        );
                    }
                    ID_ALLOW_UL => {
                        state.underline_allowed = !state.underline_allowed;
                        mark(ID_ALLOW_UL, state.underline_allowed);
                        // Flip the gated item between grayed and enabled.
                        _ = EnableMenuItem(
                            state.format_menu,
                            ID_UNDERLINE as u32,
                            (MF_BYCOMMAND
                                | if state.underline_allowed {
                                    MF_ENABLED
                                } else {
                                    MF_GRAYED
                                }) as u32,
                        );
                        // Disabling also means: forget any underline the
                        // user had set while it was allowed.
                        if !state.underline_allowed {
                            state.underline = false;
                            mark(ID_UNDERLINE, false);
                        }
                    }
                    ID_UNDERLINE => {
                        state.underline = !state.underline;
                        mark(ID_UNDERLINE, state.underline);
                    }
                    _ => return DefWindowProcW(hwnd, message, wparam, lparam),
                }
                _ = InvalidateRect(Some(hwnd), None, true);
                0
            }
            WM_PAINT => {
                let mut ps = PAINTSTRUCT::default();
                let hdc = BeginPaint(hwnd, &mut ps);
                let mut rc = RECT::default();
                _ = GetClientRect(hwnd, &mut rc);

                // The sentence that reflects every menu choice.
                let font = CreateFontW(
                    state.font_h,
                    0,
                    0,
                    0,
                    if state.bold { FW_BOLD } else { FW_NORMAL },
                    state.italic as u32,    // BOOL-ish: 1 = italic
                    state.underline as u32, // 1 = underline
                    0,                      // strikeout
                    DEFAULT_CHARSET as u32,
                    OUT_DEFAULT_PRECIS as u32,
                    CLIP_DEFAULT_PRECIS as u32,
                    CLEARTYPE_QUALITY as u32,
                    (DEFAULT_PITCH | FF_DONTCARE) as u32,
                    w!("Segoe UI"),
                );
                let old_font = SelectObject(hdc, font as HGDIOBJ);

                _ = SetBkMode(hdc, TRANSPARENT);
                _ = SetTextColor(hdc, rgb(0x20, 0x40, 0x80));
                let wide = to_wide("The quick brown fox jumps over the lazy dog");
                _ = TextOutW(hdc, 24, 60, PCWSTR::from_raw(wide.as_ptr()), -1);

                // A little settings readout below, in a small plain font.
                SelectObject(hdc, old_font);
                _ = DeleteObject(font as HGDIOBJ);
                let small = CreateFontW(
                    18,
                    0,
                    0,
                    0,
                    FW_NORMAL,
                    0,
                    0,
                    0,
                    DEFAULT_CHARSET as u32,
                    OUT_DEFAULT_PRECIS as u32,
                    CLIP_DEFAULT_PRECIS as u32,
                    CLEARTYPE_QUALITY as u32,
                    (DEFAULT_PITCH | FF_DONTCARE) as u32,
                    w!("Segoe UI"),
                );
                SelectObject(hdc, small as HGDIOBJ);
                _ = SetTextColor(hdc, rgb(0x60, 0x60, 0x60));
                let report = format!(
                    "bold={} · italic={} · underline={} (allowed={}) · height={}px",
                    state.bold,
                    state.italic,
                    state.underline,
                    state.underline_allowed,
                    state.font_h,
                );
                let wide = to_wide(&report);
                _ = TextOutW(hdc, 24, 130, PCWSTR::from_raw(wide.as_ptr()), -1);
                SelectObject(hdc, old_font);
                _ = DeleteObject(small as HGDIOBJ);

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
