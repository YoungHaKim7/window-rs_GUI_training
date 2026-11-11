//! e26 — Double Buffering: draw offscreen, blit once
//!
//! What you learn:
//! * The three-step recipe: `CreateCompatibleDC` (a scratch DC) +
//!   `CreateCompatibleBitmap` (its pixel buffer, sized to the client
//!   area) + `BitBlt` (copy the finished frame to the screen in one go)
//! * The classic trap: `CreateCompatibleBitmap(hdc_of_memory_dc, ...)`
//!   yields a 1×1 *monochrome* bitmap. The bitmap must be compatible
//!   with the SCREEN (`hdc`), while it gets *selected* into the memory DC
//! * `WM_ERASEBKGND` → return 1 = "I erased it myself". The default
//!   handler fills the window with the class brush — that's the white
//!   flash you see without this
//! * Why flicker disappears: without buffering, every primitive lands on
//!   the visible screen (the user watches half-drawn frames between the
//!   erase and the last shape). With buffering, the only visible
//!   operation is a single BitBlt of a finished frame
//! * Full cleanup discipline: old bitmap out of the DC before
//!   `DeleteObject(bitmap)`, then `DeleteDC` for DCs created with
//!   CreateCompatibleDC (never ReleaseDC those!)
//!
//! Resize this window aggressively — e04/e13 under the same abuse show
//! the flicker this example doesn't have.
//!
//! New Win32 APIs: `CreateCompatibleDC`, `CreateCompatibleBitmap`,
//! `BitBlt`, `DeleteDC` (all wingdi).
//!
//! Run: `cargo run -p e26_double_buffering`

use windows::{Win32::*, core::*};

fn main() {
    unsafe {
        let instance = GetModuleHandleW(PCWSTR::null());
        debug_assert!(!instance.is_null());

        let wc = WNDCLASSW {
            lpfnWndProc: Some(wndproc),
            hInstance: instance,
            hCursor: LoadCursorW(None, PCWSTR(IDC_ARROW.0.cast())),
            // The class brush would only ever paint invisibly (we handle
            // WM_ERASEBKGND), so it stays as a never-used default.
            hbrBackground: ((COLOR_WINDOW + 1) as isize) as HBRUSH,
            lpszClassName: w!("e26_class"),
            style: (CS_HREDRAW | CS_VREDRAW) as u32,
            ..Default::default()
        };
        debug_assert!(RegisterClassW(&wc) != 0);

        let hwnd = CreateWindowExW(
            0,
            w!("e26_class"),
            w!("e26 · Double Buffering (resize me hard)"),
            (WS_OVERLAPPEDWINDOW | WS_VISIBLE) as u32,
            CW_USEDEFAULT, CW_USEDEFAULT, 640, 400,
            None, None, Some(instance), None,
        );
        debug_assert!(!hwnd.is_null());

        let mut message = MSG::default();
        while GetMessageW(&mut message, None, 0, 0).0 > 0 {
            _ = TranslateMessage(&message);
            DispatchMessageW(&message);
        }
    }
}

fn rgb(red: u8, green: u8, blue: u8) -> COLORREF {
    (red as u32) | ((green as u32) << 8) | ((blue as u32) << 16)
}

/// The "document": draw the whole scene into any DC/size given. Being
/// size-driven (rather than fixed) is what makes resize reflow for free.
unsafe fn draw_scene(hdc: HDC, w: i32, h: i32) {
    unsafe {
        // 1) Erase OUR way, into the offscreen buffer: a vertical
        //    gradient faked with 64 solid bands (GradientFill would also
        //    work; bands keep the example dependency-free and show the
        //    create/fill/delete etiquette in a loop).
        let bands = 64;
        for i in 0..bands {
            let t = i as f32 / bands as f32;
            let color = rgb((240.0 - 60.0 * t) as u8, (244.0 - 44.0 * t) as u8, (252.0 - 28.0 * t) as u8);
            let top = i * h / bands;
            let bottom = (i + 1) * h / bands;
            let brush = CreateSolidBrush(color);
            _ = FillRect(hdc, &RECT { left: 0, top, right: w, bottom }, brush);
            _ = DeleteObject(brush as HGDIOBJ);
        }

        // 2) Content: circles, diagonals, text — straight GDI as usual.
        let pen = CreatePen(PS_SOLID, 2, rgb(0x30, 0x60, 0xB0));
        let old_pen = SelectObject(hdc, pen as HGDIOBJ);
        let brush = CreateSolidBrush(rgb(0xE0, 0xB0, 0x40));
        let old_brush = SelectObject(hdc, brush as HGDIOBJ);
        for i in 0..6 {
            let d = 30 + i * 14;
            _ = Ellipse(hdc, 20 + i * 12, h - 30 - d, 20 + i * 12 + d, h - 30);
        }
        SelectObject(hdc, GetStockObject(NULL_BRUSH));
        for i in 0..12 {
            _ = Ellipse(hdc, w / 2 - i * 28, h / 2 - i * 16, w / 2 + i * 28, h / 2 + i * 16);
        }
        SelectObject(hdc, old_pen);
        SelectObject(hdc, old_brush);
        _ = DeleteObject(pen as HGDIOBJ);
        _ = DeleteObject(brush as HGDIOBJ);

        _ = SetBkMode(hdc, TRANSPARENT);
        _ = SetTextColor(hdc, rgb(0x40, 0x40, 0x40));
        _ = TextOutW(hdc, 12, 10, w!("drawn entirely offscreen, blitted once — no flicker"), -1);
    }
}

extern "system" fn wndproc(hwnd: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe {
        match message as i32 {
            // "Erase background? Already did." Returning 1 (nonzero)
            // claims the erase is handled, so DefWindowProcW never gets
            // to splash the class brush on screen between our paints.
            // This single line kills half the flicker; the offscreen
            // buffer kills the other half.
            WM_ERASEBKGND => 1,
            WM_PAINT => {
                let mut ps = PAINTSTRUCT::default();
                let hdc = BeginPaint(hwnd, &mut ps);

                let mut rc = RECT::default();
                _ = GetClientRect(hwnd, &mut rc);
                let (w, h) = (rc.right, rc.bottom);

                if w > 0 && h > 0 {
                    // A memory DC starts out 1×1 monochrome — it has no
                    // pixels of its own until we lend it a bitmap.
                    let memdc = CreateCompatibleDC(Some(hdc));
                    // THE trap: compatible with the *screen* (hdc), not
                    // with memdc — or you get a 1×1 monochrome surface.
                    let bitmap = CreateCompatibleBitmap(hdc, w, h);
                    let old_bitmap = SelectObject(memdc, bitmap as HGDIOBJ);

                    draw_scene(memdc, w, h);

                    // One visible operation for the whole frame. SRCCOPY =
                    // plain source-to-destination copy.
                    _ = BitBlt(hdc, 0, 0, w, h, Some(memdc), 0, 0, SRCCOPY);

                    // Take our bitmap out before deleting it (a selected
                    // object can't be deleted), then destroy the DC.
                    SelectObject(memdc, old_bitmap);
                    _ = DeleteObject(bitmap as HGDIOBJ);
                    _ = DeleteDC(memdc);
                }

                _ = EndPaint(hwnd, &ps);
                0
            }
            WM_DESTROY => {
                PostQuitMessage(0);
                0
            }
            _ => DefWindowProcW(hwnd, message, wparam, lparam),
        }
    }
}
