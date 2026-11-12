//! e29 — Resize-Aware Drawing: WM_SIZE and layout that follows the window
//!
//! What you learn:
//! * `WM_SIZE` — arrives after every client-area size change; wparam says
//!   *why* (SIZE_RESTORED / SIZE_MINIMIZED / SIZE_MAXIMIZED), lparam packs
//!   the new client size (LOWORD = width, HIWORD = height)
//! * Why `CS_HREDRAW | CS_VREDRAW` matter for layout: those class styles
//!   tell Windows to invalidate the *entire* client area when the window's
//!   width/height changes. Without them only the newly exposed strip gets
//!   a WM_PAINT — centered or anchored content smears because the old
//!   pixels simply stay where they were
//! * Layout-from-the-client-rect: don't cache sizes, don't move windows —
//!   just re-read `GetClientRect` (or the WM_SIZE lparam) in WM_PAINT and
//!   place everything relative to it
//!
//! Drag an edge or maximize the window: rings re-center, corner markers
//! stay glued to their corners, the diagonals re-aim, and the status line
//! reports the client size plus the reason code of the last WM_SIZE.
//!
//! New Win32 APIs: `WM_SIZE`, the `SIZE_*` reason codes (winuser).
//!
//! Run: `cargo run -p e29_resize_aware`

use windows::{Win32::*, core::*};

#[derive(Default)]
struct State {
    /// Human-readable reason of the most recent WM_SIZE, so we can show it.
    last_reason: &'static str,
    /// Client size as of the last WM_SIZE (0x0 while minimized).
    client: (i32, i32),
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
            lpszClassName: w!("e29_class"),
            // CS_HREDRAW / CS_VREDRAW: redraw the whole client area on any
            // horizontal / vertical size change. For a layout that tracks
            // the window shape these are what make resize look correct —
            // comment them out and watch centered content smear.
            style: (CS_HREDRAW | CS_VREDRAW) as u32,
            ..Default::default()
        };
        debug_assert!(RegisterClassW(&wc) != 0);

        let hwnd = CreateWindowExW(
            0,
            w!("e29_class"),
            w!("e29 · Resize — content follows the window"),
            (WS_OVERLAPPEDWINDOW | WS_VISIBLE) as u32,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            640,
            440,
            None,
            None,
            Some(instance),
            None,
        );
        debug_assert!(!hwnd.is_null());

        let state = Box::into_raw(Box::<State>::default());
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, state as isize);

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
            WM_SIZE => {
                // wparam tells you WHY the size changed. User programs can
                // mostly ignore it (layout is the same), but it matters for
                // things you only do on genuine user resizes — and it is a
                // lifesaver for logging/debugging size storms.
                state.last_reason = match wparam as i32 {
                    SIZE_RESTORED => "SIZE_RESTORED (drag/restore)",
                    SIZE_MINIMIZED => "SIZE_MINIMIZED",
                    SIZE_MAXIMIZED => "SIZE_MAXIMIZED",
                    _ => "other (see SIZE_MAXSHOW/SIZE_MAXHIDE, MDI only)",
                };
                // The new client size comes packed in lparam — signed 16-bit
                // halves, same layout as mouse coordinates.
                state.client = (
                    (lparam & 0xffff) as u16 as i32,
                    ((lparam >> 16) & 0xffff) as u16 as i32,
                );
                // No InvalidateRect needed: with CS_HREDRAW|CS_VREDRAW the
                // whole client area was already invalidated before WM_SIZE
                // reached us. (It still reaches us before the WM_PAINT —
                // the classic place to re-layout children or re-wrap text.)
                0
            }
            WM_PAINT => {
                let mut ps = PAINTSTRUCT::default();
                let hdc = BeginPaint(hwnd, &mut ps);

                let mut rc = RECT::default();
                _ = GetClientRect(hwnd, &mut rc);
                let (w, h) = (rc.right, rc.bottom);
                // The layout anchor: everything below is computed from the
                // live client rect, never from a cached size. That single
                // rule IS "resize awareness".
                let (cx, cy) = (w / 2, h / 2);

                // ---- Concentric rings, dead center ------------------------
                // Radius tracks min(w,h) so the rings fit any aspect ratio.
                let pen = CreatePen(PS_SOLID, 2, rgb(0x30, 0x60, 0xB0));
                let old_pen = SelectObject(hdc, pen as HGDIOBJ);
                let base = (w.min(h) / 2 - 20).max(10);
                for i in 1..=4 {
                    let r = base * i / 4;
                    _ = Ellipse(hdc, cx - r, cy - r, cx + r, cy + r);
                }

                // ---- Diagonals corner-to-corner: they visibly re-aim -----
                let thin = CreatePen(PS_DOT, 1, rgb(0xA0, 0x40, 0x40));
                SelectObject(hdc, thin as HGDIOBJ);
                _ = MoveToEx(hdc, 0, 0, None);
                _ = LineTo(hdc, w, h);
                _ = MoveToEx(hdc, w, 0, None);
                _ = LineTo(hdc, 0, h);
                SelectObject(hdc, old_pen);
                _ = DeleteObject(thin as HGDIOBJ);

                // ---- Corner markers: content anchored to an edge ---------
                // Same idea as centering, but anchored: position measured
                // from rc.right / rc.bottom instead of the center.
                let brush = CreateSolidBrush(rgb(0xE0, 0x90, 0x20));
                for (x, y) in [(8, 8), (w - 36, 8), (8, h - 36), (w - 36, h - 36)] {
                    _ = FillRect(
                        hdc,
                        &RECT {
                            left: x,
                            top: y,
                            right: x + 28,
                            bottom: y + 28,
                        },
                        brush,
                    );
                }
                _ = DeleteObject(brush as HGDIOBJ);

                // ---- Status text: size + reason of the last WM_SIZE ------
                _ = SetBkMode(hdc, TRANSPARENT);
                _ = SetTextColor(hdc, rgb(0x20, 0x20, 0x20));
                let line1 = format!("client: {} × {} px", state.client.0, state.client.1);
                let wide1 = to_wide(&line1);
                _ = TextOutW(hdc, 10, h - 64, PCWSTR::from_raw(wide1.as_ptr()), -1);
                let line2 = format!("last WM_SIZE: {}", state.last_reason);
                let wide2 = to_wide(&line2);
                _ = TextOutW(hdc, 10, h - 42, PCWSTR::from_raw(wide2.as_ptr()), -1);

                // Centered caption via DrawTextW with the full client rect
                // and DT_CENTER|DT_VCENTER — it reflows all by itself.
                _ = SetTextColor(hdc, rgb(0x30, 0x60, 0xB0));
                DrawTextW(
                    hdc,
                    w!("everything on this canvas is placed from the live client rect"),
                    -1,
                    &mut rc,
                    (DT_CENTER | DT_VCENTER | DT_SINGLELINE) as u32,
                );

                SelectObject(hdc, old_pen);
                _ = DeleteObject(pen as HGDIOBJ);
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
