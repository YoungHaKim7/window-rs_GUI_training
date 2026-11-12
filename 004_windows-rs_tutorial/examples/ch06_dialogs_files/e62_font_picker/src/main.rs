//! e62 — Font Picker: ChooseFontW, LOGFONTW, CreateFontIndirectW
//!
//! What you learn:
//! * `ChooseFontW` — the Fonts dialog. It works on a `LOGFONTW` (a font
//!   *description*: height, weight, italic, face name — 14 fields, no
//!   handle yet) that the dialog edits IN PLACE
//! * The flag trio: `CF_INITTOLOGFONTSTRUCT` (open the dialog showing the
//!   font our LOGFONTW describes), `CF_SCREENFONTS` (offer screen fonts,
//!   not printer fonts), `CF_EFFECTS` (show the strikeout/underline/color
//!   controls — color comes back in `CHOOSEFONTW.rgbColors`)
//! * `CreateFontIndirectW` — LOGFONTW (description) → HFONT (handle). The
//!   same struct is the file format of RTF "font info", the registry's
//!   saved fonts, and this dialog — one struct to learn, everywhere used
//! * **Font lifetime**: an HFONT is a GDI object — delete it when done,
//!   but ONLY after nothing selects it anymore. The swap discipline here:
//!   create new → store → delete old (never while a DC holds it)
//!
//! New Win32 APIs: `ChooseFontW`, `CHOOSEFONTW`, `LOGFONTW`,
//! `CreateFontIndirectW`, `SelectObject` with HFONT.
//!
//! Run: `cargo run -p e62_font_picker`

use windows::{Win32::*, core::*};

/// Main-window button that opens the font dialog.
const IDC_FONT: i32 = 1001;

#[derive(Default)]
struct State {
    logfont: LOGFONTW, // the current font DESCRIPTION (dialog edits this)
    font: HFONT,       // the compiled handle (null until first created)
    color: COLORREF,   // CF_EFFECTS color; 0 = black
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
            lpszClassName: w!("e62_class"),
            style: (CS_HREDRAW | CS_VREDRAW) as u32,
            ..Default::default()
        };
        debug_assert!(RegisterClassW(&wc) != 0);

        let hwnd = CreateWindowExW(
            0,
            w!("e62_class"),
            w!("e62 · Font Picker — the sample wears the choice"),
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

        // Seed a starting description: lfHeight NEGATIVE means "em height
        // in pixels" (positive means cell height including internal
        // leading — almost never what you want). FW_NORMAL = 400,
        // FW_BOLD = 700; the rest zero is fine. Copy the face name into
        // the fixed [WCHAR; 32] array, null included.
        if let Some(s) = state_of(hwnd) {
            s.logfont.lfHeight = -28;
            s.logfont.lfWeight = FW_NORMAL as i32;
            for (i, u) in "Segoe UI".encode_utf16().enumerate() {
                s.logfont.lfFaceName[i] = u;
            }
            s.font = CreateFontIndirectW(&s.logfont);
            debug_assert!(!s.font.is_null());
        }

        let font_button = CreateWindowExW(
            0,
            w!("BUTTON"),
            w!("Font..."),
            (WS_CHILD | WS_VISIBLE | BS_PUSHBUTTON) as u32,
            20,
            20,
            100,
            32,
            Some(hwnd),
            Some(IDC_FONT as usize as HMENU),
            Some(instance),
            None,
        );
        debug_assert!(!font_button.is_null());

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

extern "system" fn wndproc(hwnd: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe {
        let Some(state) = state_of(hwnd) else {
            return DefWindowProcW(hwnd, message, wparam, lparam);
        };

        match message as i32 {
            WM_COMMAND => {
                let notification = ((wparam >> 16) & 0xffff) as u32;
                let control_id = (wparam & 0xffff) as u32;
                if notification == BN_CLICKED as u32 && control_id == IDC_FONT as u32 {
                    let mut cf = CHOOSEFONTW {
                        lStructSize: std::mem::size_of::<CHOOSEFONTW>() as u32,
                        hwndOwner: hwnd,
                        // The dialog reads this to preset its controls
                        // (CF_INITTOLOGFONTSTRUCT) and writes the user's
                        // choices back into the same struct on OK.
                        lpLogFont: &mut state.logfont,
                        Flags: (CF_INITTOLOGFONTSTRUCT | CF_SCREENFONTS | CF_EFFECTS) as u32,
                        rgbColors: state.color, // in/out with CF_EFFECTS
                        ..Default::default()
                    };
                    if ChooseFontW(&mut cf).0 != 0 {
                        state.color = cf.rgbColors;

                        // LOGFONTW → HFONT, then the careful swap:
                        let new_font = CreateFontIndirectW(&state.logfont);
                        let old_font = state.font;
                        state.font = new_font;
                        // Delete the OLD one only now — after the swap, and
                        // only because no DC selects it at this moment
                        // (WM_PAINT always restores before returning).
                        // Deleting a font a DC still holds = undefined.
                        if !old_font.is_null() {
                            _ = DeleteObject(old_font as HGDIOBJ);
                        }
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

                let old_font = SelectObject(hdc, state.font as HGDIOBJ);
                let old_color = SetTextColor(hdc, state.color);
                _ = SetBkMode(hdc, TRANSPARENT);

                let sample = to_wide("Sphinx of black quartz, judge my vow! 0123");
                let mut r = rect;
                r.left += 20;
                r.top += 90;
                r.right -= 20;
                _ = DrawTextW(
                    hdc,
                    PCWSTR::from_raw(sample.as_ptr()),
                    -1,
                    &mut r,
                    (DT_SINGLELINE | DT_VCENTER) as u32,
                );

                // Describe what's on screen straight from the LOGFONTW.
                let face_end = state
                    .logfont
                    .lfFaceName
                    .iter()
                    .position(|&u| u == 0)
                    .unwrap_or(0);
                let face = String::from_utf16_lossy(&state.logfont.lfFaceName[..face_end]);
                let info = to_wide(&format!(
                    "{} {} — height {} px, weight {}",
                    face,
                    if state.logfont.lfWeight == FW_BOLD as i32 {
                        "bold"
                    } else {
                        "regular"
                    },
                    -state.logfont.lfHeight,
                    state.logfont.lfWeight
                ));
                let info_color = SetTextColor(hdc, 0x404040);
                _ = TextOutW(
                    hdc,
                    20,
                    60,
                    PCWSTR::from_raw(info.as_ptr()),
                    info.len() as i32 - 1,
                );
                SetTextColor(hdc, info_color);

                SetTextColor(hdc, old_color);
                SelectObject(hdc, old_font);
                _ = SetBkMode(hdc, OPAQUE);
                _ = EndPaint(hwnd, &ps);
                0
            }
            WM_DESTROY => {
                // The LAST font dies here — still exactly one delete per
                // create, just at the other end of the lifetime.
                if !state.font.is_null() {
                    _ = DeleteObject(state.font as HGDIOBJ);
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
