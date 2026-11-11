//! e46 — Radio Buttons: grouping with WS_GROUP
//!
//! What you learn:
//! * Radio buttons are BUTTONs with `BS_AUTORADIOBUTTON`; like e45's
//!   checkbox the "auto" means the control manages its own selection
//! * **The WS_GROUP rule**: a radio group runs from one control with
//!   `WS_GROUP` to the *next* control with `WS_GROUP` (or the end of the
//!   dialog). The arrow keys stop there and auto-deselection happens only
//!   inside the group — the flag *starts* a group; the next WS_GROUP ends it
//! * Scanning the group on demand (`BM_GETCHECK`) instead of tracking
//!   every click — radios only matter when you commit ("Apply")
//!
//! New Win32 APIs: `BS_AUTORADIOBUTTON`, `WS_GROUP`, `BM_GETCHECK`.
//!
//! Run: `cargo run -p e46_radio_buttons`

use windows::{Win32::*, core::*};

const IDC_APPLY: i32 = 1004;
const IDC_RADIO_BASE: i32 = 1011; // 1011 = sky, 1012 = moss, 1013 = plum

#[derive(Default)]
struct State {
    radios: [HWND; 3], // default (null) — filled after creation
    color: COLORREF, // current paint color, starts as COLOR_WINDOW gray
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
            lpszClassName: w!("e46_class"),
            style: (CS_HREDRAW | CS_VREDRAW) as u32,
            ..Default::default()
        };
        debug_assert!(RegisterClassW(&wc) != 0);

        let hwnd = CreateWindowExW(
            0,
            w!("e46_class"),
            w!("e46 · Radio Buttons — pick a color, then Apply"),
            (WS_OVERLAPPEDWINDOW | WS_VISIBLE) as u32,
            CW_USEDEFAULT, CW_USEDEFAULT, 420, 300,
            None, None, Some(instance), None,
        );
        debug_assert!(!hwnd.is_null());

        let state = Box::into_raw(Box::<State>::default());
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, state as isize);

        // ---- The radio group ------------------------------------------------
        // THE rule, spelled out: a radio group is *every control from one
        // WS_GROUP up to but not including the next WS_GROUP* (in creation /
        // tab order). The FIRST radio carries WS_GROUP — that is what opens
        // the group. The "Apply" button below also carries WS_GROUP: not
        // because it is a radio, but because its WS_GROUP *closes* the radio
        // group. Remove that WS_GROUP from the button and the group would
        // silently swallow it — arrow keys would hop onto the button and the
        // "one of N" deselect logic would keep running past your radios.
        let labels = ["Sky", "Moss", "Plum"];
        let mut radios = [std::ptr::null_mut(); 3];
        for (i, label) in labels.iter().enumerate() {
            let style = if i == 0 {
                WS_CHILD | WS_VISIBLE | WS_GROUP | BS_AUTORADIOBUTTON // group STARTS here
            } else {
                WS_CHILD | WS_VISIBLE | BS_AUTORADIOBUTTON // plain members
            };
            let wide = to_wide(label);
            radios[i] = CreateWindowExW(
                0,
                w!("BUTTON"),
                PCWSTR::from_raw(wide.as_ptr()),
                style as u32,
                24, 24 + i as i32 * 28, 120, 24,
                Some(hwnd),
                Some((IDC_RADIO_BASE + i as i32) as usize as HMENU),
                Some(instance),
                None,
            );
            debug_assert!(!radios[i].is_null());
        }

        // "Apply" — note WS_GROUP on a push button: it closes the radio
        // group above. This is the detail everyone forgets.
        let hwnd_apply = CreateWindowExW(
            0,
            w!("BUTTON"),
            w!("Apply"),
            (WS_CHILD | WS_VISIBLE | WS_GROUP | BS_PUSHBUTTON | WS_TABSTOP) as u32,
            24, 120, 120, 28,
            Some(hwnd), Some(IDC_APPLY as usize as HMENU), Some(instance), None,
        );
        debug_assert!(!hwnd_apply.is_null());
        let _ = hwnd_apply;

        // Check the first radio so the group starts in a defined state.
        SendMessageW(radios[0], BM_SETCHECK as u32, BST_CHECKED as usize, 0);

        if let Some(s) = state_of(hwnd) {
            s.radios = radios;
            s.color = rgb(0x8F, 0xBC, 0xEB); // "Sky" — matches the preselection
        }

        let mut message = MSG::default();
        while GetMessageW(&mut message, None, 0, 0).0 > 0 {
            _ = TranslateMessage(&message);
            DispatchMessageW(&message);
        }
    }
}

fn to_wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

fn rgb(red: u8, green: u8, blue: u8) -> COLORREF {
    (red as u32) | ((green as u32) << 8) | ((blue as u32) << 16)
}

unsafe fn state_of(hwnd: HWND) -> Option<&'static mut State> {
    unsafe {
        let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut State;
        (!ptr.is_null()).then(|| &mut *ptr)
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
                if notification == BN_CLICKED as u32 && control_id == IDC_APPLY as u32 {
                    // Radios are "commit later" controls: ignore every click,
                    // scan the whole group once when the user applies.
                    // (BM_GETCHECK is the same message the checkbox used in
                    // e45 — same class, remember.)
                    let palette = [
                        rgb(0x8F, 0xBC, 0xEB), // Sky
                        rgb(0x7C, 0xA5, 0x70), // Moss
                        rgb(0xBD, 0x86, 0xB8), // Plum
                    ];
                    for (radio, color) in state.radios.iter().zip(palette) {
                        if SendMessageW(*radio, BM_GETCHECK as u32, 0, 0) as i32 == BST_CHECKED {
                            state.color = color;
                        }
                    }
                    // Repaint the whole client in the chosen color.
                    _ = InvalidateRect(Some(hwnd), None, true);
                }
                0
            }
            WM_PAINT => {
                let mut ps = PAINTSTRUCT::default();
                let hdc = BeginPaint(hwnd, &mut ps);
                let mut rect = RECT::default();
                _ = GetClientRect(hwnd, &mut rect);

                // The preview swatch: solid brush, created fresh per paint
                // and deleted right after — never keep GDI objects around.
                rect.top = 170; // leave the controls' half alone
                let brush = CreateSolidBrush(state.color);
                _ = FillRect(hdc, &rect, brush);
                _ = DeleteObject(brush as HGDIOBJ);

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
