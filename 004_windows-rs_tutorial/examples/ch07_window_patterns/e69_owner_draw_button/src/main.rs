//! e69 — Owner-Draw Buttons: you paint the button, Windows just tells you when
//!
//! What you learn:
//! * `BS_OWNERDRAW` — a style pact: the BUTTON control stops drawing itself
//!   and instead sends its **parent** a `WM_DRAWITEM` whenever its looks
//!   could have changed (first show, press, release, focus change). Inside
//!   that message, `lparam` points to a `DRAWITEMSTRUCT` with everything
//!   you need: the control's id, its `hDC`, its `rcItem`, and — the heart
//!   of it — `itemState` bits like `ODS_SELECTED` and `ODS_FOCUS`.
//! * The classic pressed-button illusion: when `ODS_SELECTED` is set, shift
//!   the whole content one pixel down-and-right before painting. Buttons
//!   have faked depth this way since Windows 3.1.
//! * `DrawFocusRect` — the dotted "keyboard focus" rectangle XOR-drawn on
//!   top; calling it twice with the same rect erases it.
//! * The contract detail people forget: WM_DRAWITEM does NOT redraw on
//!   plain hover — you invalidate through state changes, or the control
//!   invalidates itself on press/release (which is what we ride here).
//!
//! Click the two buttons: each paints its own two-tone face, dents 1px
//! while held, and the window below counts the clicks.
//!
//! New Win32 APIs: `WM_DRAWITEM`, `DRAWITEMSTRUCT`, `ODS_SELECTED`,
//! `ODS_FOCUS`, `ODT_BUTTON`, `DrawFocusRect`, `FrameRect`.
//!
//! Run: `cargo run -p e69_owner_draw_button`

use windows::{Win32::*, core::*};

/// Control ids of the two owner-draw buttons.
const IDC_BLUE: i32 = 1001;
const IDC_GREEN: i32 = 1002;

#[derive(Default)]
struct State {
    /// Click counters, indexed by button 0/1 (mapped from the control id).
    clicks: [u32; 2],
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
            lpszClassName: w!("e69_class"),
            style: (CS_HREDRAW | CS_VREDRAW) as u32,
            ..Default::default()
        };
        debug_assert!(RegisterClassW(&wc) != 0);

        // The e65 flavor: state rides CreateWindowExW's lpparam and is
        // attached in WM_NCCREATE, so WM_CREATE below can already use it.
        let state = Box::into_raw(Box::<State>::default());

        let hwnd = CreateWindowExW(
            0,
            w!("e69_class"),
            w!("e69 · Owner-Draw Buttons — WM_DRAWITEM paints these"),
            (WS_OVERLAPPEDWINDOW | WS_VISIBLE) as u32,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            520,
            300,
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

fn rgb(red: u8, green: u8, blue: u8) -> COLORREF {
    (red as u32) | ((green as u32) << 8) | ((blue as u32) << 16)
}

fn to_wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

/// The look of one button, by control id: a light tone for the top band, a
/// dark tone for the bottom band, and a caption. Returning data instead of
/// painting here keeps WM_DRAWITEM a pure renderer.
fn button_theme(id: i32) -> (&'static str, COLORREF, COLORREF) {
    match id {
        IDC_BLUE => (
            "Blue pill",
            rgb(0x5A, 0x8D, 0xD6), // lighter top
            rgb(0x2B, 0x53, 0x8C), // darker bottom
        ),
        _ => (
            "Green pill",
            rgb(0x6D, 0xB5, 0x7A), // lighter top
            rgb(0x2F, 0x6B, 0x3B), // darker bottom
        ),
    }
}

/// Lay both buttons side by side, called from WM_CREATE and WM_SIZE —
/// the parent owns layout because only it knows the client size.
unsafe fn layout_buttons(hwnd: HWND) {
    unsafe {
        let mut rc = RECT::default();
        _ = GetClientRect(hwnd, &mut rc);
        let gap = 20;
        let width = (rc.right - 3 * gap) / 2;
        for (i, id) in [IDC_BLUE, IDC_GREEN].iter().enumerate() {
            let btn = GetDlgItem(Some(hwnd), *id);
            if !btn.is_null() {
                _ = MoveWindow(btn, gap + i as i32 * (width + gap), 40, width, 90, true);
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
                for id in [IDC_BLUE, IDC_GREEN] {
                    // BS_OWNERDRAW is the whole trick: after this the button
                    // control never paints itself again — every visual now
                    // arrives in our WM_DRAWITEM. It still does ALL the
                    // interaction logic (press detection, clicks, focus).
                    let btn = CreateWindowExW(
                        0,
                        w!("BUTTON"),
                        w!("owner-drawn — you never see this text"),
                        (WS_CHILD | WS_VISIBLE | BS_OWNERDRAW) as u32,
                        0,
                        0,
                        0,
                        0, // sized by layout_buttons
                        Some(hwnd),
                        Some(id as usize as HMENU),
                        Some(instance),
                        None,
                    );
                    debug_assert!(!btn.is_null());
                }
                layout_buttons(hwnd);
                0
            }

            WM_SIZE => {
                layout_buttons(hwnd);
                0
            }

            WM_DRAWITEM => {
                let dis = lparam as *const DRAWITEMSTRUCT;
                if (*dis).CtlType as i32 != ODT_BUTTON {
                    // Not ours (theoretically other control kinds could
                    // owner-draw too) — let the default path have it.
                    return DefWindowProcW(hwnd, message, wparam, lparam);
                }

                let (_, top_color, bottom_color) = button_theme((*dis).CtlID as i32);
                let pressed = (*dis).itemState as i32 & ODS_SELECTED != 0;
                let focused = (*dis).itemState as i32 & ODS_FOCUS != 0;

                let hdc = (*dis).hDC;
                let rc = (*dis).rcItem; // our canvas, in button coords

                // Two-tone fill: a lighter band over the top 3/5, a darker
                // one below — an cheap facsimile of a lit surface. The DC's
                // origin is already the button, so (0,0) is its corner.
                let split = rc.top + (rc.bottom - rc.top) * 3 / 5;
                let brush_top = CreateSolidBrush(top_color);
                _ = FillRect(
                    hdc,
                    &RECT {
                        top: rc.top,
                        bottom: split,
                        ..rc
                    },
                    brush_top,
                );
                _ = DeleteObject(brush_top as HGDIOBJ);
                let brush_bottom = CreateSolidBrush(bottom_color);
                _ = FillRect(
                    hdc,
                    &RECT {
                        top: split,
                        bottom: rc.bottom,
                        ..rc
                    },
                    brush_bottom,
                );
                _ = DeleteObject(brush_bottom as HGDIOBJ);

                // The pressed illusion: nudge the label 1px down-right while
                // the button is held. That single pixel is the entire 3D
                // effect — everything else stayed put.
                let mut label = rc;
                if pressed {
                    label.left += 1;
                    label.top += 1;
                    label.right += 1;
                    label.bottom += 1;
                }

                _ = SetBkMode(hdc, TRANSPARENT);
                _ = SetTextColor(hdc, rgb(0xFF, 0xFF, 0xFF));
                let (caption, _, _) = button_theme((*dis).CtlID as i32);
                let wide = to_wide(caption);
                DrawTextW(
                    hdc,
                    PCWSTR::from_raw(wide.as_ptr()),
                    -1,
                    &mut label,
                    (DT_CENTER | DT_VCENTER | DT_SINGLELINE) as u32,
                );

                // A rim in the dark tone ties the bands together.
                let rim = CreateSolidBrush(bottom_color);
                _ = FrameRect(hdc, &rc, rim);
                _ = DeleteObject(rim as HGDIOBJ);

                // The keyboard-focus dots. DrawFocusRect is XOR-based, which
                // is why it takes no color — and why it vanishes again when
                // the control redraws without ODS_FOCUS.
                if focused {
                    let mut focus = rc;
                    _ = InflateRect(&mut focus, -3, -3);
                    _ = DrawFocusRect(hdc, &focus);
                }

                // "Handled": return nonzero so nobody else tries to draw.
                1
            }

            WM_COMMAND => {
                let notification = ((wparam >> 16) & 0xffff) as u32;
                let control_id = (wparam & 0xffff) as u32;
                // The interaction half is still 100% BUTTON control: clicks
                // arrive as ordinary BN_CLICKED notifications. Owner-draw
                // changed the PAINT, never the input plumbing.
                if notification == BN_CLICKED as u32 {
                    if let Some(state) = state_of(hwnd) {
                        let index = if control_id == IDC_BLUE as u32 { 0 } else { 1 };
                        state.clicks[index] += 1;
                        _ = InvalidateRect(Some(hwnd), None, true);
                    }
                }
                0
            }

            WM_PAINT => {
                let mut ps = PAINTSTRUCT::default();
                let hdc = BeginPaint(hwnd, &mut ps);
                let mut rc = RECT::default();
                _ = GetClientRect(hwnd, &mut rc);

                if let Some(state) = state_of(hwnd) {
                    _ = SetBkMode(hdc, TRANSPARENT);
                    _ = SetTextColor(hdc, rgb(0x30, 0x30, 0x30));
                    let (blue, green) = (&button_theme(IDC_BLUE).0, &button_theme(IDC_GREEN).0);
                    let report = format!(
                        "{}: {} clicks    {}: {} clicks",
                        blue, state.clicks[0], green, state.clicks[1]
                    );
                    let wide = to_wide(&report);
                    let mut area = rc;
                    area.top = rc.bottom - 60;
                    DrawTextW(
                        hdc,
                        PCWSTR::from_raw(wide.as_ptr()),
                        -1,
                        &mut area,
                        (DT_CENTER | DT_SINGLELINE) as u32,
                    );
                    let hint = to_wide("hold a button down: the label shifts 1px (ODS_SELECTED)");
                    let mut hint_area = rc;
                    hint_area.top = rc.bottom - 36;
                    DrawTextW(
                        hdc,
                        PCWSTR::from_raw(hint.as_ptr()),
                        -1,
                        &mut hint_area,
                        (DT_CENTER | DT_SINGLELINE) as u32,
                    );
                }

                _ = EndPaint(hwnd, &ps);
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
