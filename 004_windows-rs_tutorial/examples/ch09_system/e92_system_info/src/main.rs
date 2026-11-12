//! e92 — System Info: interrogating the machine you run on
//!
//! What you learn:
//! * `GetSystemInfo` + `SYSTEM_INFO` — CPU count, page size, allocation
//!   granularity, and the Rust shape of the C **anonymous union**: the
//!   bindings name it `Anonymous`, so the C `wProcessorArchitecture` becomes
//!   `info.Anonymous.Anonymous.wProcessorArchitecture` (one `Anonymous` for
//!   the union-in-struct, one for the union's inner struct) — and reading it
//!   is `unsafe` because unions are unions
//! * `GlobalMemoryStatusEx` + `MEMORYSTATUSEX` — RAM totals/free and memory
//!   load, with the eternal gotcha: set `dwLength = size_of::<MEMORYSTATUSEX>()`
//!   first, or the call fails — the struct is its own version header
//! * `GetSystemMetrics` — the grab-bag: SM_CXSCREEN/SM_CYSCREEN (primary
//!   monitor), SM_CXVIRTUALSCREEN/SM_CYVIRTUALSCREEN + SM_CMONITORS (the
//!   whole multi-monitor desktop), SM_REMOTESESSION (Remote Desktop)
//! * `GetTickCount64` — milliseconds since boot, 64-bit so it doesn't wrap
//!   after 49 days like the ancient 32-bit GetTickCount
//!
//! Everything is drawn as a plain text table in WM_PAINT.
//!
//! New Win32 APIs: `GetSystemInfo`, `GlobalMemoryStatusEx`, `GetTickCount64`
//! (sysinfoapi); `GetSystemMetrics` (winuser).
//!
//! Run: `cargo run -p e92_system_info`

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
            lpszClassName: w!("e92_class"),
            style: (CS_HREDRAW | CS_VREDRAW) as u32,
            ..Default::default()
        };
        debug_assert!(RegisterClassW(&wc) != 0);

        let hwnd = CreateWindowExW(
            0,
            w!("e92_class"),
            w!("e92 · System Info — CPU, memory, screens, uptime"),
            (WS_OVERLAPPEDWINDOW | WS_VISIBLE) as u32,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            640,
            560,
            None,
            None,
            Some(instance),
            None,
        );
        debug_assert!(!hwnd.is_null());

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

/// Collect every fact into label/value rows for the painted table.
fn gather_rows() -> Vec<(String, String)> {
    unsafe {
        let mut rows: Vec<(String, String)> = Vec::new();

        // ---- CPU / processor -------------------------------------------
        let mut info = SYSTEM_INFO::default();
        GetSystemInfo(&mut info);
        rows.push(("CPU count".into(), info.dwNumberOfProcessors.to_string()));

        // The C struct starts with an anonymous union { dwOemId, struct {
        // wProcessorArchitecture, wReserved } }. The bindings materialize
        // both anonymity levels as fields named "Anonymous", so the path
        // doubles up. Union reads are unsafe — you assert which variant is
        // live by knowing the API's contract (GetSystemInfo always fills
        // the struct variant on modern Windows).
        let arch: String = match info.Anonymous.Anonymous.wProcessorArchitecture as i32 {
            PROCESSOR_ARCHITECTURE_INTEL => "x86".into(),
            PROCESSOR_ARCHITECTURE_AMD64 => "x64 (Intel/AMD 64-bit)".into(),
            PROCESSOR_ARCHITECTURE_ARM64 => "ARM64".into(),
            PROCESSOR_ARCHITECTURE_ARM => "ARM32".into(),
            other => format!("arch code {other}"),
        };
        rows.push(("Architecture".into(), arch.into()));
        rows.push(("Page size".into(), format!("{} bytes", info.dwPageSize)));
        rows.push((
            "Allocation granularity".into(),
            format!("{} bytes", info.dwAllocationGranularity),
        ));

        // ---- Memory -----------------------------------------------------
        let mut mem = MEMORYSTATUSEX::default();
        // The dwLength contract (same family of gotcha as PROCESSENTRY32W's
        // dwSize): the struct is versioned by its own size — set it or the
        // call fails and all the fields stay garbage.
        mem.dwLength = std::mem::size_of::<MEMORYSTATUSEX>() as u32;
        debug_assert!(GlobalMemoryStatusEx(&mut mem).as_bool());

        let gib = |bytes: u64| format!("{:.1} GiB", bytes as f64 / (1024.0 * 1024.0 * 1024.0));
        rows.push(("Memory load".into(), format!("{}%", mem.dwMemoryLoad)));
        rows.push(("Physical memory".into(), gib(mem.ullTotalPhys)));
        rows.push(("Physical available".into(), gib(mem.ullAvailPhys)));

        // ---- Screens ----------------------------------------------------
        let cx = GetSystemMetrics(SM_CXSCREEN);
        let cy = GetSystemMetrics(SM_CYSCREEN);
        rows.push(("Primary screen".into(), format!("{cx} x {cy} px")));

        let vx = GetSystemMetrics(SM_CXVIRTUALSCREEN);
        let vy = GetSystemMetrics(SM_CYVIRTUALSCREEN);
        let monitors = GetSystemMetrics(SM_CMONITORS);
        rows.push((
            "Virtual desktop".into(),
            format!("{vx} x {vy} px across {monitors} monitor(s)"),
        ));

        let remote = GetSystemMetrics(SM_REMOTESESSION) != 0;
        rows.push((
            "Remote Desktop session".into(),
            if remote { "yes".into() } else { "no".into() },
        ));

        // ---- Uptime -----------------------------------------------------
        let ms = GetTickCount64();
        let secs = ms / 1000;
        rows.push((
            "Uptime".into(),
            format!("{}h {}m {}s", secs / 3600, (secs / 60) % 60, secs % 60),
        ));

        rows
    }
}

extern "system" fn wndproc(hwnd: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe {
        match message as i32 {
            WM_PAINT => {
                let mut ps = PAINTSTRUCT::default();
                let hdc = BeginPaint(hwnd, &mut ps);
                let mut rc = RECT::default();
                _ = GetClientRect(hwnd, &mut rc);

                let rows = gather_rows();

                // Fixed two-column layout: labels left, values at x = 220.
                // No GDI table API exists — DrawTextW in a rectangle, row
                // by row, is how everyone does it.
                _ = SetBkMode(hdc, TRANSPARENT);
                let mut y = 24;
                let draw = |text: &str, x: i32, y: &mut i32, bold: bool| {
                    if bold {
                        // Bold labels: a font made on the fly (CreateFontW
                        // takes DWORDs for everything but height and weight).
                        let font = CreateFontW(
                            20,
                            0,
                            0,
                            0,
                            FW_SEMIBOLD,
                            0,
                            0,
                            0,
                            DEFAULT_CHARSET as u32,
                            OUT_DEFAULT_PRECIS as u32,
                            CLIP_DEFAULT_PRECIS as u32,
                            DEFAULT_QUALITY as u32,
                            (DEFAULT_PITCH | FF_DONTCARE) as u32,
                            w!("Segoe UI"),
                        );
                        let old = SelectObject(hdc, font as HGDIOBJ);
                        let wide = to_wide(text);
                        _ = TextOutW(
                            hdc,
                            x,
                            *y,
                            PCWSTR::from_raw(wide.as_ptr()),
                            wide.len() as i32 - 1,
                        );
                        SelectObject(hdc, old);
                        _ = DeleteObject(font as HGDIOBJ);
                    } else {
                        let wide = to_wide(text);
                        _ = TextOutW(
                            hdc,
                            x,
                            *y,
                            PCWSTR::from_raw(wide.as_ptr()),
                            wide.len() as i32 - 1,
                        );
                    }
                    *y += 28;
                };

                draw("System", 24, &mut y, true);
                y += 6;
                for (label, value) in &rows {
                    draw(label, 40, &mut y, true);
                    draw(value, 240, &mut y, false);
                }

                _ = SetBkMode(hdc, OPAQUE);
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
