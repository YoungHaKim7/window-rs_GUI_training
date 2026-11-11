//! e72 — Layered Windows: per-window alpha and one-color transparency
//!
//! What you learn:
//! * `WS_EX_LAYERED` — the extended style that lets Windows treat the
//!   whole window as a composited surface. It must be set BEFORE you call
//!   `SetLayeredWindowAttributes` (here: at creation, in CreateWindowExW's
//!   first argument).
//! * `SetLayeredWindowAttributes` with `LWA_ALPHA`: one byte (0..255)
//!   fades the ENTIRE window — frame, menubar, children, everything —
//!   over whatever is behind it. A trackbar drives it live from 30..255.
//! * The same call with `LWA_COLORKEY`: one exact COLORREF becomes fully
//!   transparent (binary: match or not), the classic cheap trick for
//!   "shaped" windows. Its caveat: the cut is per-pixel exact, so
//!   antialiased edges (rounded shapes, ClearType text) that blend toward
//!   the key color stay VISIBLE — jagged halos. Per-pixel alpha via
//!   UpdateLayeredWindow is the pro version; e73's SetWindowRgn has the
//!   same jagged-edge issue by construction.
//! * The trackbar from e51, this time controlling a real property: its
//!   WM_HSCROLL notifications read TBM_GETPOS and push the new alpha.
//!
//! Drag the slider to fade the whole window (30% floor so it can't
//! vanish). Press C to toggle color-key mode: the magenta patch below
//! turns into a see-through hole.
//!
//! New Win32 APIs: `WS_EX_LAYERED`, `SetLayeredWindowAttributes`,
//! `LWA_ALPHA`, `LWA_COLORKEY` (winuser); trackbar recap from e51.
//!
//! Run: `cargo run -p e72_layered_window`

use windows::{Win32::*, core::*};

/// Control id of the trackbar.
const IDC_ALPHA: i32 = 1001;

/// The color that becomes transparent in LWA_COLORKEY mode.
const KEY_COLOR: COLORREF = 0x00FF_00FF; // magenta — nothing else uses it

#[derive(Default)]
struct State {
    /// false = LWA_COLORKEY mode (slider disabled), true = LWA_ALPHA.
    alpha_mode: bool,
    /// Last known trackbar position (only meaningful in alpha mode).
    alpha: i32,
}

fn main() {
    unsafe {
        // The trackbar class must be registered before first use.
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
            lpszClassName: w!("e72_class"),
            style: (CS_HREDRAW | CS_VREDRAW) as u32,
            ..Default::default()
        };
        debug_assert!(RegisterClassW(&wc) != 0);

        let state = Box::into_raw(Box::new(State { alpha_mode: true, alpha: 255 }));

        // WS_EX_LAYERED goes in dwExStyle (argument 1). The layered
        // machinery is inert until SetLayeredWindowAttributes configures
        // it — we do that in WM_CREATE once the trackbar exists.
        let hwnd = CreateWindowExW(
            WS_EX_LAYERED as u32,
            w!("e72_class"),
            w!("e72 · Layered Window — drag the slider, press C"),
            (WS_OVERLAPPEDWINDOW | WS_VISIBLE) as u32,
            CW_USEDEFAULT, CW_USEDEFAULT, 520, 360,
            None, None, Some(instance),
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

/// Re-apply the layered attribute for the current mode. One function, so
/// WM_CREATE / slider moves / the C key all share the exact same call.
unsafe fn apply_window_attributes(hwnd: HWND) {
    unsafe {
        let Some(state) = state_of(hwnd) else { return };
        if state.alpha_mode {
            // LWA_ALPHA: the crKey argument is ignored; bAlpha is the whole
            // window's opacity, children included.
            _ = SetLayeredWindowAttributes(
                hwnd,
                0,
                state.alpha.clamp(30, 255) as u8,
                LWA_ALPHA as u32,
            );
        } else {
            // LWA_COLORKEY: bAlpha is ignored; every pixel EXACTLY equal to
            // crKey is punched out (no partial blending, no antialiasing —
            // see the jagged caveat painted around the patch).
            _ = SetLayeredWindowAttributes(hwnd, KEY_COLOR, 0, LWA_COLORKEY as u32);
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

                let track = CreateWindowExW(
                    0,
                    w!("msctls_trackbar32"),
                    w!(""),
                    (WS_CHILD | WS_VISIBLE) as u32, // TBS_HORZ = 0, the default
                    30, 270, 440, 40,
                    Some(hwnd), Some(IDC_ALPHA as usize as HMENU), Some(instance), None,
                );
                debug_assert!(!track.is_null());

                // Range 30..255 (the 30 floor keeps the window findable),
                // then park the thumb at fully opaque. Both ends of the
                // range ride ONE lparam, 16 bits each (e51 explains).
                SendMessageW(
                    track,
                    TBM_SETRANGE as u32,
                    1, // redraw afterwards
                    (30 | (255 << 16)) as LPARAM,
                );
                SendMessageW(track, TBM_SETPOS as u32, 1, 255);

                apply_window_attributes(hwnd);
                0
            }

            WM_HSCROLL => {
                // The trackbar reports here; lparam is its HWND.
                let track = lparam as HWND;
                if !track.is_null() {
                    let pos = SendMessageW(track, TBM_GETPOS as u32, 0, 0) as i32;
                    if let Some(state) = state_of(hwnd) {
                        state.alpha = pos;
                    }
                    apply_window_attributes(hwnd);
                }
                0
            }

            WM_KEYDOWN => {
                // C toggles the transparency MODE. In color-key mode the
                // slider is pointless (bAlpha ignored), so it gets grayed
                // out via EnableWindow — controls explaining themselves.
                if wparam as i32 == 'C' as i32 {
                    if let Some(state) = state_of(hwnd) {
                        state.alpha_mode = !state.alpha_mode;
                        let track = GetDlgItem(Some(hwnd), IDC_ALPHA);
                        if !track.is_null() {
                            _ = EnableWindow(track, state.alpha_mode);
                        }
                        apply_window_attributes(hwnd);
                        _ = InvalidateRect(Some(hwnd), None, true);
                    }
                    0
                } else {
                    DefWindowProcW(hwnd, message, wparam, lparam)
                }
            }

            WM_PAINT => {
                let mut ps = PAINTSTRUCT::default();
                let hdc = BeginPaint(hwnd, &mut ps);
                let mut rc = RECT::default();
                _ = GetClientRect(hwnd, &mut rc);

                // A small scene worth seeing fade: two-tone bands.
                let bands = [rgb(0x2D, 0x6C, 0xAF), rgb(0x4F, 0x9D, 0x69), rgb(0xE0, 0x9B, 0x2E)];
                let band_h = 40;
                for (i, color) in bands.iter().enumerate() {
                    let brush = CreateSolidBrush(*color);
                    _ = FillRect(
                        hdc,
                        &RECT { left: 30, top: 60 + i as i32 * band_h, right: 250, bottom: 60 + (i as i32 + 1) * band_h },
                        brush,
                    );
                    _ = DeleteObject(brush as HGDIOBJ);
                }

                // THE color-key patch: in LWA_COLORKEY mode this exact
                // rectangle becomes a window to whatever is behind. Note
                // the Ellipse right below it — the same key color, but as
                // a curved GDI shape whose edge pixels only APPROACH
                // magenta. Those survive the cut: the proof of the
                // jagged-edge caveat, visible as a ragged rim.
                let brush = CreateSolidBrush(KEY_COLOR);
                _ = FillRect(
                    hdc,
                    &RECT { left: 280, top: 60, right: 470, bottom: 180 },
                    brush,
                );
                _ = SelectObject(hdc, brush as HGDIOBJ);
                _ = Ellipse(hdc, 300, 190, 450, 250);
                _ = SelectObject(hdc, GetStockObject(NULL_BRUSH));
                _ = DeleteObject(brush as HGDIOBJ);

                _ = SetBkMode(hdc, TRANSPARENT);
                _ = SetTextColor(hdc, rgb(0x30, 0x30, 0x30));
                let lines = [
                    "slider: whole-window alpha (LWA_ALPHA)",
                    "C: toggle color-key mode (LWA_COLORKEY)",
                    "in color-key mode the magenta patch + ellipse edge show",
                    "the per-pixel cut: exact matches vanish, blended edges",
                    "survive as jaggies — the documented trade-off.",
                ];
                for (i, line) in lines.iter().enumerate() {
                    let wide = to_wide(line);
                    _ = TextOutW(hdc, 30, 190 + i as i32 * 18, PCWSTR::from_raw(wide.as_ptr()), -1);
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
