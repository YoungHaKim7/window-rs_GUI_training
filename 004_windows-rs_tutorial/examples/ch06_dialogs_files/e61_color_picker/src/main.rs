//! e61 — Color Picker: ChooseColorW and the 16 custom-color slots
//!
//! What you learn:
//! * `ChooseColorW` — the classic Colors dialog: fill a `CHOOSECOLORW`,
//!   call, read `rgbResult`
//! * **The lpCustColors shape**: `*mut COLORREF` pointing at EXACTLY 16
//!   entries — the "Custom colors:" grid in the dialog's corner. The
//!   dialog READS them on open and WRITES whatever the user defines back
//!   into your array — so keeping the array in the window state (e13
//!   pattern) makes custom colors persist for the whole run, exactly like
//!   in real apps (persisting across runs means saving them yourself)
//! * `CC_RGBINIT` = open with `rgbResult` preselected; `CC_FULLOPEN` =
//!   skip the silly "Define Custom Colors >>" click and show the mixer
//!   expanded immediately
//! * Feeding the result into `WM_PAINT`: the chosen COLORREF becomes a
//!   solid brush for the whole client area
//!
//! New Win32 APIs: `ChooseColorW`, `CHOOSECOLORW`, `CreateSolidBrush`.
//!
//! Run: `cargo run -p e61_color_picker`

use windows::{Win32::*, core::*};

/// Main-window button that opens the color dialog.
const IDC_PICK: i32 = 1001;

#[derive(Default)]
struct State {
    background: COLORREF, // 0 = pure black is a fine starting point
    // The custom-color grid: 16 slots the dialog reads AND writes. Living
    // here (inside the boxed state) means the pointer stays valid and the
    // user's definitions survive between opens — the whole persistence
    // story for free.
    custom_colors: [COLORREF; 16],
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
            lpszClassName: w!("e61_class"),
            style: (CS_HREDRAW | CS_VREDRAW) as u32,
            ..Default::default()
        };
        debug_assert!(RegisterClassW(&wc) != 0);

        let hwnd = CreateWindowExW(
            0,
            w!("e61_class"),
            w!("e61 · Color Picker — the window wears the choice"),
            (WS_OVERLAPPEDWINDOW | WS_VISIBLE) as u32,
            CW_USEDEFAULT, CW_USEDEFAULT, 480, 320,
            None, None, Some(instance), None,
        );
        debug_assert!(!hwnd.is_null());

        let state = Box::into_raw(Box::<State>::default());
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, state as isize);

        let pick = CreateWindowExW(
            0,
            w!("BUTTON"),
            w!("Pick color..."),
            (WS_CHILD | WS_VISIBLE | BS_PUSHBUTTON) as u32,
            20, 20, 110, 32,
            Some(hwnd), Some(IDC_PICK as usize as HMENU), Some(instance), None,
        );
        debug_assert!(!pick.is_null());

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
                if notification == BN_CLICKED as u32 && control_id == IDC_PICK as u32 {
                    let mut cc = CHOOSECOLORW {
                        lStructSize: std::mem::size_of::<CHOOSECOLORW>() as u32,
                        hwndOwner: hwnd,
                        // In: the initially selected color (because of
                        // CC_RGBINIT below). Out: the user's choice.
                        rgbResult: state.background,
                        // The 16 custom slots — a plain mutable pointer,
                        // which is why the array must outlive the call
                        // (it lives in the boxed state, so it does).
                        lpCustColors: state.custom_colors.as_mut_ptr(),
                        Flags: (CC_FULLOPEN | CC_RGBINIT) as u32,
                        ..Default::default()
                    };
                    // Returns 0 on cancel.
                    if ChooseColorW(&mut cc).0 != 0 {
                        state.background = cc.rgbResult;
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

                // COLORREF is 0x00BBGGRR (the byte order surprises RGB
                // people; e47's rgb() helper builds it correctly).
                let brush = CreateSolidBrush(state.background);
                _ = FillRect(hdc, &rect, brush);
                _ = DeleteObject(brush as HGDIOBJ);

                // Label on top of the fill. Outline so it stays readable
                // on any background.
                let text = to_wide(&format!(
                    "background = COLORREF 0x{:06X} — pick again",
                    state.background
                ));
                _ = SetBkMode(hdc, TRANSPARENT);
                _ = TextOutW(hdc, 20, 70, PCWSTR::from_raw(text.as_ptr()), text.len() as i32 - 1);
                _ = SetBkMode(hdc, OPAQUE);

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
