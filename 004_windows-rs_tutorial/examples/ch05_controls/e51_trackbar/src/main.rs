//! e51 — Trackbar (slider): WM_HSCROLL from a control, position on demand
//!
//! What you learn:
//! * The trackbar (`msctls_trackbar32`, `ICC_BAR_CLASSES` in
//!   `InitCommonControlsEx`): `TBM_SETRANGE` (min and max PACKED into one
//!   lparam — 16 bits each, the old MAKEWPARAM game), `TBM_SETPOS`,
//!   `TBM_SETTIC` for individually placed tick marks (`TBS_AUTOTICKS` in
//!   the style is the lazy alternative)
//! * The slider reports through `WM_HSCROLL` (vertical trackbars use
//!   `WM_VSCROLL`) with `lparam` = the trackbar's HWND — the exact
//!   control-vs-window-scrollbar distinction from e49
//! * Unlike a scrollbar, the trackbar OWNS its position arithmetic: you
//!   don't compute pages and lines yourself, you just ask `TBM_GETPOS`
//!   whenever a notification arrives
//!
//! Drag the slider; the "volume" bar below scales with it.
//!
//! New Win32 APIs: `TBM_SETRANGE`, `TBM_SETPOS`, `TBM_SETTIC`, `TBM_GETPOS`.
//!
//! Run: `cargo run -p e51_trackbar`

use windows::{Win32::*, core::*};

const VOLUME_MIN: i32 = 0;
const VOLUME_MAX: i32 = 100;

#[derive(Default)]
struct State {
    hwnd_trackbar: HWND,
    volume: i32,
}

fn main() {
    unsafe {
        // ICC_BAR_CLASSES registers the toolbar/trackbar/statusbar family
        // (one flag covers several related classes — that's why it isn't
        // called ICC_TRACKBAR_CLASS).
        let icc = INITCOMMONCONTROLSEX {
            dwSize: std::mem::size_of::<INITCOMMONCONTROLSEX>() as u32,
            dwICC: ICC_BAR_CLASSES as u32,
        };
        _ = InitCommonControlsEx(&icc);

        let instance = GetModuleHandleW(PCWSTR::null());
        debug_assert!(!instance.is_null());

        let wc = WNDCLASSW {
            lpfnWndProc: Some(wndproc),
            hInstance: instance,
            hCursor: LoadCursorW(None, PCWSTR(IDC_ARROW.0.cast())),
            hbrBackground: ((COLOR_WINDOW + 1) as isize) as HBRUSH,
            lpszClassName: w!("e51_class"),
            style: (CS_HREDRAW | CS_VREDRAW) as u32,
            ..Default::default()
        };
        debug_assert!(RegisterClassW(&wc) != 0);

        let hwnd = CreateWindowExW(
            0,
            w!("e51_class"),
            w!("e51 · Trackbar — drag the slider"),
            (WS_OVERLAPPEDWINDOW | WS_VISIBLE) as u32,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            440,
            300,
            None,
            None,
            Some(instance),
            None,
        );
        debug_assert!(!hwnd.is_null());

        let state = Box::into_raw(Box::<State>::default());
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, state as isize);

        // Horizontal by default (TBS_HORZ, value 0); TBS_VERT would rotate
        // it — and switch its notifications to WM_VSCROLL.
        let track = CreateWindowExW(
            0,
            w!("msctls_trackbar32"),
            w!(""),
            (WS_CHILD | WS_VISIBLE) as u32,
            20,
            20,
            380,
            40,
            Some(hwnd),
            None,
            Some(instance),
            None,
        );
        debug_assert!(!track.is_null());

        // TBM_SETRANGE packs BOTH ends into lparam — low 16 bits = min,
        // high 16 = max (the ancient MAKELPARAM layout). wparam = whether
        // to redraw afterwards.
        SendMessageW(
            track,
            TBM_SETRANGE as u32,
            1, // TRUE: redraw after setting the range
            ((VOLUME_MIN as usize) | ((VOLUME_MAX as usize) << 16)) as LPARAM,
        );
        // Park the thumb somewhere visible.
        SendMessageW(track, TBM_SETPOS as u32, 1, 30); // wparam=redraw, lparam=pos
        // Tick marks, one message each: 0 / 25 / 50 / 75 / 100. (They only
        // draw on the side you style — TBS_BOTH would do both sides; the
        // default bottom-only is fine here.) TBS_AUTOTICKS in the style
        // would instead tick automatically once per range increment.
        for &tic in &[0, 25, 50, 75, 100] {
            SendMessageW(track, TBM_SETTIC as u32, 0, tic as LPARAM);
        }

        if let Some(s) = state_of(hwnd) {
            s.hwnd_trackbar = track;
            s.volume = 30;
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

extern "system" fn wndproc(hwnd: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe {
        let Some(state) = state_of(hwnd) else {
            return DefWindowProcW(hwnd, message, wparam, lparam);
        };

        match message as i32 {
            // Trackbar notifications ride WM_HSCROLL. Same rule as e49:
            // lparam == 0 means a *window* scrollbar; lparam != 0 is that
            // HWND — ours. Compare explicitly so extra controls later can't
            // be mistaken for the slider.
            WM_HSCROLL => {
                if lparam as HWND != state.hwnd_trackbar {
                    return DefWindowProcW(hwnd, message, wparam, lparam);
                }
                // The LOWORD of wparam says WHY it moved (SB_THUMBTRACK
                // while dragging, SB_LINEUP/DOWN for the arrow keys,
                // SB_PAGEUP/DOWN for page keys/clicks on the track) — but
                // unlike a scrollbar the trackbar has already done its own
                // line/page math and updated its position. One question to
                // ask, one answer to use:
                state.volume = SendMessageW(state.hwnd_trackbar, TBM_GETPOS as u32, 0, 0) as i32;
                _ = InvalidateRect(Some(hwnd), None, true);
                0
            }
            WM_PAINT => {
                let mut ps = PAINTSTRUCT::default();
                let hdc = BeginPaint(hwnd, &mut ps);
                let mut rect = RECT::default();
                _ = GetClientRect(hwnd, &mut rect);

                // The volume gauge: a frame plus a fill proportional to the
                // trackbar position — the canonical "slider drives a bar"
                // pairing you see in every volume control ever shipped.
                let x0 = 20;
                let x1 = rect.right - 20;
                let y0 = 120;
                let h = 40;
                let fill_w = ((x1 - x0) * state.volume) / VOLUME_MAX;

                let frame = CreatePen(PS_SOLID, 1, rgb(0x80, 0x80, 0x80));
                let old_pen = SelectObject(hdc, frame as HGDIOBJ);
                _ = Rectangle(hdc, x0, y0, x1 + 1, y0 + h);
                let fill = CreateSolidBrush(rgb(0x50, 0xA0, 0x40));
                _ = FillRect(
                    hdc,
                    &RECT {
                        left: x0 + 1,
                        top: y0 + 1,
                        right: x0 + fill_w,
                        bottom: y0 + h,
                    },
                    fill,
                );
                _ = DeleteObject(fill as HGDIOBJ);
                SelectObject(hdc, old_pen);
                _ = DeleteObject(frame as HGDIOBJ);

                let text = to_wide(&format!("volume = {} / 100", state.volume));
                _ = SetBkMode(hdc, TRANSPARENT);
                _ = TextOutW(
                    hdc,
                    x0,
                    y0 + h + 12,
                    PCWSTR::from_raw(text.as_ptr()),
                    text.len() as i32 - 1,
                );
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
