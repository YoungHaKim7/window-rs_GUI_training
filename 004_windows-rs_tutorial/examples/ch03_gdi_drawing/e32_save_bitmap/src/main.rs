//! e32 — Save the Client Area to a Real .bmp File
//!
//! What you learn:
//! * Capturing the screen: `GetDC` → `CreateCompatibleDC` +
//!   `CreateCompatibleBitmap` → `BitBlt(SRCCOPY)` — the classic
//!   screen-to-memory copy chain
//! * `GetDIBits` — asking GDI to hand over the *raw pixels* of an HBITMAP
//!   as a device-independent bitmap (DIB), in the format you specify
//!   (here: 24 bpp, uncompressed, bottom-up BGR rows)
//! * The .bmp file format by hand: `BITMAPFILEHEADER` ("BM" magic, file
//!   size, pixel offset) + `BITMAPINFOHEADER` (the DIB header) + the rows
//!   — and the two gotchas: rows are padded to a multiple of 4 bytes, and
//!   `BITMAPFILEHEADER` is *packed* to 14 bytes in the file while the Rust
//!   struct is 16 (alignment padding), so it is written field by field
//! * Writing the file next to the exe with `std::fs` — no external crates
//!
//! Watch the painted scene, then press S: a `client_capture.bmp` appears
//! next to the executable and the full path is printed to the console.
//!
//! New Win32 APIs: `CreateCompatibleDC`, `CreateCompatibleBitmap`,
//! `BitBlt`, `GetDIBits` (wingdi); `GetDC`/`ReleaseDC` (winuser).
//!
//! Run: `cargo run -p e32_save_bitmap`

use std::io::Write as _;
use windows::{Win32::*, core::*};

fn main() {
    unsafe {
        let instance = GetModuleHandleW(PCWSTR::null());
        debug_assert!(!instance.is_null());

        let wc = WNDCLASSW {
            lpfnWndProc: Some(wndproc),
            hInstance: instance,
            hCursor: LoadCursorW(None, PCWSTR(IDC_ARROW.0.cast())),
            hbrBackground: ((COLOR_WINDOW + 1) as isize) as HBRUSH,
            lpszClassName: w!("e32_class"),
            style: (CS_HREDRAW | CS_VREDRAW) as u32,
            ..Default::default()
        };
        debug_assert!(RegisterClassW(&wc) != 0);

        let hwnd = CreateWindowExW(
            0,
            w!("e32_class"),
            w!("e32 · Save Bitmap — press S to snapshot the client area"),
            (WS_OVERLAPPEDWINDOW | WS_VISIBLE) as u32,
            CW_USEDEFAULT, CW_USEDEFAULT, 600, 420,
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

fn to_wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Capture `hwnd`'s client area and write it as a 24-bpp .bmp next to the
/// executable. Returns the file path written.
unsafe fn save_client_bitmap(hwnd: HWND) -> std::io::Result<std::path::PathBuf> {
    unsafe {
        let mut rc = RECT::default();
        _ = GetClientRect(hwnd, &mut rc);
        let (w, h) = (rc.right, rc.bottom);
        debug_assert!(w > 0 && h > 0);

        // ---- 1. Blit the client pixels into a memory bitmap -------------
        let screen = GetDC(Some(hwnd));
        let mem = CreateCompatibleDC(Some(screen));
        // Compatible with the SCREEN dc (not the memory dc!) or you get a
        // 1×1 monochrome bitmap — the classic CreateCompatibleBitmap bug.
        let hbmp = CreateCompatibleBitmap(screen, w, h);
        let old = SelectObject(mem, hbmp as HGDIOBJ);
        _ = BitBlt(mem, 0, 0, w, h, Some(screen), 0, 0, SRCCOPY);

        // ---- 2. Ask GetDIBits for the pixels as 24-bpp bottom-up BGR ----
        let mut bmi = BITMAPINFO::default();
        bmi.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
        bmi.bmiHeader.biWidth = w;
        bmi.bmiHeader.biHeight = h; // POSITIVE height = bottom-up rows
        bmi.bmiHeader.biPlanes = 1; // must be 1
        bmi.bmiHeader.biBitCount = 24; // 3 bytes per pixel, BGR order
        bmi.bmiHeader.biCompression = BI_RGB as u32; // uncompressed

        // Each row is w*3 bytes, padded UP to a multiple of 4 (the DIB
        // spec is 32-bit-word aligned). 24 doesn't divide 4, so odd
        // widths really do carry padding bytes.
        let stride = (w as usize * 3 + 3) & !3;
        let mut pixels = vec![0u8; stride * h as usize];
        let copied = GetDIBits(
            mem,
            hbmp,
            0, // first scanline to retrieve
            h as u32, // number of scanlines (all of them)
            Some(pixels.as_mut_ptr() as LPVOID),
            &mut bmi, // in/out: describes the REQUESTED format
            DIB_RGB_COLORS as u32, // "no palette, literal RGB"
        );
        debug_assert!(copied == h);

        // Teardown while the DC/bitmap still exist.
        SelectObject(mem, old);
        _ = DeleteObject(hbmp as HGDIOBJ);
        _ = DeleteDC(mem);
        _ = ReleaseDC(Some(hwnd), screen);

        // ---- 3. Assemble the .bmp file ----------------------------------
        // Layout: FILEHEADER(14) + INFOHEADER(40) + pixel rows. 24-bpp
        // BI_RGB needs no palette, so pixels start at offset 54.
        let pixel_bytes = pixels.len() as u32;
        let mut file: Vec<u8> = Vec::with_capacity(54 + pixel_bytes as usize);

        // BITMAPFILEHEADER is 14 bytes PACKED in the file. The Rust struct
        // is repr(C) with DWORD alignment = 16 bytes (padding after the
        // WORD bfType), so we serialize field by field — never blit it.
        file.extend_from_slice(b"BM"); // bfType: the "BM" magic
        file.extend_from_slice(&(54 + pixel_bytes).to_le_bytes()); // bfSize
        file.extend_from_slice(&0u16.to_le_bytes()); // bfReserved1
        file.extend_from_slice(&0u16.to_le_bytes()); // bfReserved2
        file.extend_from_slice(&54u32.to_le_bytes()); // bfOffBits

        // BITMAPINFOHEADER: 11 fields, all naturally 4-byte-ish aligned —
        // exactly 40 bytes, no hidden padding (asserted, then blitted).
        debug_assert_eq!(std::mem::size_of::<BITMAPINFOHEADER>(), 40);
        let header = &bmi.bmiHeader as *const _ as *const u8;
        file.extend_from_slice(std::slice::from_raw_parts(header, 40));

        // GetDIBits already wrote bottom-up, padded BGR rows — exactly
        // what the file wants. Nothing to transform.
        file.extend_from_slice(&pixels);

        // ---- 4. Write it next to the exe --------------------------------
        let exe = std::env::current_exe()?;
        let path = exe.parent().map(std::path::Path::to_path_buf).unwrap_or_default();
        let path = path.join("client_capture.bmp");
        let mut f = std::fs::File::create(&path)?;
        f.write_all(&file)?;
        f.flush()?;
        Ok(path)
    }
}

extern "system" fn wndproc(hwnd: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe {
        match message as i32 {
            WM_KEYDOWN => {
                // Press S to save. There is no VK_S — letters use their
                // ASCII code as the virtual key.
                if wparam as i32 == 'S' as i32 {
                    match save_client_bitmap(hwnd) {
                        Ok(path) => println!("saved client area to {}", path.display()),
                        Err(e) => println!("save failed: {e}"),
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

                // Something worth snapshotting: colored bands + circles.
                let bands = [
                    rgb(0x2D, 0x6C, 0xAF), rgb(0x4F, 0x9D, 0x69),
                    rgb(0xE0, 0x9B, 0x2E), rgb(0xCB, 0x5A, 0x5A),
                ];
                let band_h = (rc.bottom - rc.top) / bands.len() as i32;
                for (i, color) in bands.iter().enumerate() {
                    let brush = CreateSolidBrush(*color);
                    _ = FillRect(
                        hdc,
                        &RECT { top: i as i32 * band_h, bottom: (i as i32 + 1) * band_h, ..rc },
                        brush,
                    );
                    _ = DeleteObject(brush as HGDIOBJ);
                }
                let brush = CreateSolidBrush(rgb(0xF4, 0xF4, 0xF4));
                let old_brush = SelectObject(hdc, brush as HGDIOBJ);
                let cx = (rc.right - rc.left) / 2;
                _ = Ellipse(hdc, cx - 70, band_h + 10, cx + 70, band_h + 150);
                _ = Ellipse(hdc, cx - 40, band_h + 160, cx + 40, band_h + 240);
                SelectObject(hdc, old_brush);
                _ = DeleteObject(brush as HGDIOBJ);

                _ = SetBkMode(hdc, TRANSPARENT);
                _ = SetTextColor(hdc, rgb(0xFF, 0xFF, 0xFF));
                let wide = to_wide("press S to save this as client_capture.bmp");
                _ = TextOutW(hdc, 12, 12, PCWSTR::from_raw(wide.as_ptr()), -1);

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
