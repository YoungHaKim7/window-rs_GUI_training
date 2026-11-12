//! e30 — DPI Awareness: crisp on every monitor, sized in real units
//!
//! What you learn:
//! * `SetProcessDpiAwarenessContext(PER_MONITOR_AWARE_V2)` — the modern
//!   opt-in, called *before* any window exists: "we read pixels ourselves,
//!   don't bitmap-stretch us". Without it Windows lies to your process
//!   (reports 96 dpi) and scales your window blurry to compensate
//! * `GetDpiForWindow` — the authoritative dpi of the monitor a window is
//!   on; per-monitor-aware windows get told the truth
//! * `WM_DPICHANGED` — fired when the window moves to a monitor with a
//!   different scale factor. wparam's HIWORD is the new dpi; lparam points
//!   at a RECT Windows *suggests* for the new size — applying it keeps the
//!   window's physical size stable across monitors
//! * The one formula that rules them all: `pixels = logical * dpi / 96`.
//!   The ruler drawn here spaces its ticks at `dpi/4` px — a quarter inch
//!   of *physical* screen, whatever the display scale
//!
//! Drag the window between monitors with different scale factors and watch
//! the ruler keep its physical length while the tick count adapts.
//!
//! New Win32 APIs: `SetProcessDpiAwarenessContext`, `GetDpiForWindow`,
//! `WM_DPICHANGED`, `SetWindowPos` (winuser / windef).
//!
//! Run: `cargo run -p e30_dpi_awareness`

use windows::{Win32::*, core::*};

#[derive(Default)]
struct State {
    /// dpi of the monitor the window currently sits on (96 = 100%).
    dpi: u32,
}

fn main() {
    unsafe {
        // Per-Monitor-V2 must be declared before the first HWND of the
        // process is created. Returns FALSE if something already locked the
        // process into another awareness mode (e.g. a manifest, or an
        // earlier call) — for a tutorial we assert, real apps log and go on.
        debug_assert!(
            SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2).as_bool()
        );

        let instance = GetModuleHandleW(PCWSTR::null());
        debug_assert!(!instance.is_null());

        let wc = WNDCLASSW {
            lpfnWndProc: Some(wndproc),
            hInstance: instance,
            hCursor: LoadCursorW(None, PCWSTR(IDC_ARROW.0.cast())),
            hbrBackground: ((COLOR_WINDOW + 1) as isize) as HBRUSH,
            lpszClassName: w!("e30_class"),
            style: (CS_HREDRAW | CS_VREDRAW) as u32,
            ..Default::default()
        };
        debug_assert!(RegisterClassW(&wc) != 0);

        let hwnd = CreateWindowExW(
            0,
            w!("e30_class"),
            w!("e30 · DPI — a ruler that keeps its physical size"),
            (WS_OVERLAPPEDWINDOW | WS_VISIBLE) as u32,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            760,
            300,
            None,
            None,
            Some(instance),
            None,
        );
        debug_assert!(!hwnd.is_null());

        let state = Box::into_raw(Box::<State>::default());
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, state as isize);

        // Prime the dpi now that the window exists (it arrives on whatever
        // monitor the OS chose — CW_USEDEFAULT).
        let dpi = GetDpiForWindow(hwnd);
        (*state).dpi = if dpi == 0 { 96 } else { dpi };

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
            WM_DPICHANGED => {
                // HIWORD(wparam) = the new dpi (LOWORD = horizontal dpi —
                // identical on every real display today).
                state.dpi = ((wparam >> 16) & 0xffff) as u32;

                // lparam points at the rect Windows suggests our window
                // occupy after the scale change — same physical size, just
                // rescaled. Applying it is the recommended behavior; skip
                // it and the window visibly grows/shrinks between monitors.
                let suggested = &*(lparam as *const RECT);
                _ = SetWindowPos(
                    hwnd,
                    None,
                    suggested.left,
                    suggested.top,
                    suggested.right - suggested.left,
                    suggested.bottom - suggested.top,
                    // NOZORDER/NOACTIVATE: a pure resize, don't shuffle Z.
                    (SWP_NOZORDER | SWP_NOACTIVATE) as u32,
                );
                _ = InvalidateRect(Some(hwnd), None, true);
                0
            }
            WM_PAINT => {
                let mut ps = PAINTSTRUCT::default();
                let hdc = BeginPaint(hwnd, &mut ps);
                let mut rc = RECT::default();
                _ = GetClientRect(hwnd, &mut rc);
                let dpi = state.dpi.max(1);

                _ = SetBkMode(hdc, TRANSPARENT);

                // ---- The ruler along the top -------------------------------
                // A "logical inch" is 96 units; scaled by dpi it becomes a
                // physical inch. Quarter-inch ticks: dpi/4 device pixels.
                let ruler_h = 64;
                let pen = CreatePen(PS_SOLID, 2, rgb(0x30, 0x30, 0x30));
                let old_pen = SelectObject(hdc, pen as HGDIOBJ);

                // Ruler body outline.
                _ = MoveToEx(hdc, 0, ruler_h, None);
                _ = LineTo(hdc, rc.right, ruler_h);

                let quarter = (dpi / 4).max(4) as i32; // px per quarter inch
                let mut inch = 0;
                let mut x = quarter; // start one tick in
                while x < rc.right {
                    // Every 4th tick is a full inch (long + labeled), every
                    // 2nd a half (medium), others a quarter (short).
                    let (len, label) = match inch % 4 {
                        0 => (ruler_h - 8, true),
                        2 => (ruler_h / 2, false),
                        _ => (ruler_h / 4, false),
                    };
                    _ = MoveToEx(hdc, x, ruler_h, None);
                    _ = LineTo(hdc, x, ruler_h - len);
                    if label {
                        let text = format!("{}", inch / 4 + 1);
                        let wide = to_wide(&text);
                        _ = SetTextColor(hdc, rgb(0x30, 0x30, 0x30));
                        _ = TextOutW(
                            hdc,
                            x + 4,
                            ruler_h - len - 18,
                            PCWSTR::from_raw(wide.as_ptr()),
                            -1,
                        );
                    }
                    x += quarter;
                    inch += 1;
                }
                SelectObject(hdc, old_pen);
                _ = DeleteObject(pen as HGDIOBJ);

                // ---- Report the current scale ------------------------------
                let scale = dpi as f32 / 96.0;
                let line1 = format!("GetDpiForWindow = {} ({}%)", dpi, (scale * 100.0) as u32);
                let wide1 = to_wide(&line1);
                _ = SetTextColor(hdc, rgb(0xB0, 0x30, 0x30));
                _ = TextOutW(hdc, 12, ruler_h + 20, PCWSTR::from_raw(wide1.as_ptr()), -1);

                let line2 = format!(
                    "tick spacing = dpi/4 = {} px ≈ one quarter of a physical inch",
                    dpi / 4
                );
                let wide2 = to_wide(&line2);
                _ = SetTextColor(hdc, rgb(0x40, 0x50, 0x60));
                _ = TextOutW(hdc, 12, ruler_h + 46, PCWSTR::from_raw(wide2.as_ptr()), -1);

                let line3 = "drag me between monitors with different scale factors";
                let wide3 = to_wide(line3);
                _ = TextOutW(hdc, 12, ruler_h + 72, PCWSTR::from_raw(wide3.as_ptr()), -1);

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
