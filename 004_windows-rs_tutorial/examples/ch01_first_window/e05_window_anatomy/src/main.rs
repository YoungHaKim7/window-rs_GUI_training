//! e05 — Window Anatomy: every field and parameter, annotated
//!
//! What you learn:
//! * What each `WNDCLASSW` field controls (cursor, background, icon, ...)
//! * What each `CreateWindowExW` parameter means — including the extended
//!   style argument (here: `WS_EX_ACCEPTFILES`, previewing drag & drop)
//! * The two-step show: `ShowWindow` + `UpdateWindow`, and why WS_VISIBLE
//!   is just a shortcut for the first one
//!
//! This example is e03 with the magnifying glass on. Read top to bottom;
//! the code is the article.
//!
//! Run: `cargo run -p e05_window_anatomy`

use windows::{Win32::*, core::*};

/// Convert a Rust &str into a null-terminated UTF-16 buffer — the shape
/// Win32 "W" functions expect. Keep the returned Vec alive as long as the
/// PCWSTR derived from it. (e06 takes a deep dive.)
fn to_wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

fn main() {
    unsafe {
        // ============================================================
        // HINSTANCE — "which module is creating this window?"
        // ============================================================
        // Almost all window APIs take the exe's module handle. It anchors
        // things owned per-module: the class registration below, resources
        // (icons, strings), window menus...
        let instance = GetModuleHandleW(PCWSTR::null());
        debug_assert!(!instance.is_null());

        // ============================================================
        // WNDCLASSW — the blueprint shared by many windows
        // ============================================================
        let wc = WNDCLASSW {
            // lpfnWndProc: THE callback. Windows calls this for every
            // message for every window of this class. Returning
            // DefWindowProcW for unhandled messages is the contract that
            // keeps standard behavior (drag, resize, close, ...) alive.
            lpfnWndProc: Some(wndproc),

            // style: class-level behavior flags.
            //   CS_HREDRAW / CS_VREDRAW — redraw the *whole* window on
            //   horizontal / vertical resize (cheap & simple; causes a
            //   visible flash; we fix flicker with double buffering in e26).
            style: (CS_HREDRAW | CS_VREDRAW) as u32,

            // cbClsExtra / cbWndExtra: bytes of extra storage Windows
            // reserves per class / per window. We'll use the window-bytes
            // trick in older code bases; in Rust we prefer GWLP_USERDATA.
            cbClsExtra: 0,
            cbWndExtra: 0,

            hInstance: instance,

            // hIcon: the icon in the title bar / taskbar. None means
            // "infer from the class"; we load custom icons in e39.
            hIcon: LoadIconW(None, PCWSTR(IDI_APPLICATION.0.cast())),

            // hCursor: mouse pointer shape over the client area.
            hCursor: LoadCursorW(None, PCWSTR(IDC_ARROW.0.cast())),

            // hbrBackground: what the client area is cleared with before
            // WM_PAINT. (COLOR_WINDOW + 1) = "system window color" — the +1
            // is because color indices 0.. are smuggled in as fake handles.
            hbrBackground: ((COLOR_WINDOW + 1) as isize) as HBRUSH,

            // lpszMenuName: a resource menu to attach — we have none yet
            // (menus arrive in e33, built in code, not resources).
            lpszMenuName: PCWSTR::null(),

            // lpszClassName: the lookup key. Any window created with this
            // exact name uses this blueprint.
            lpszClassName: w!("e05_class"),
        };

        // RegisterClassW returns an ATOM (nonzero id) or 0 on failure.
        let atom = RegisterClassW(&wc);
        debug_assert!(atom != 0, "RegisterClassW failed");

        // ============================================================
        // CreateWindowExW — build one window from the blueprint
        // ============================================================
        let hwnd = CreateWindowExW(
            // --- extended styles (the exotic ones) --------------------
            // Accept files dragged from Explorer (see e75 for the full
            // treatment). Regular styles below cover the everyday look.
            WS_EX_ACCEPTFILES as usize as u32,
            // --- class name: which blueprint --------------------------
            w!("e05_class"),
            // --- window name: the title-bar caption --------------------
            w!("Window Anatomy — every part explained"),
            // --- style: the everyday look -----------------------------
            // WS_OVERLAPPEDWINDOW bundles: WS_OVERLAPPED | WS_CAPTION
            //   | WS_SYSMENU | WS_THICKFRAME | WS_MINIMIZEBOX
            //   | WS_MAXIMIZEBOX — "a normal top-level window".
            // NOTE: no WS_VISIBLE this time — we show it manually below.
            WS_OVERLAPPEDWINDOW as u32,
            // --- position and size ------------------------------------
            // x/y in *screen* coordinates; CW_USEDEFAULT lets the window
            // manager cascade new windows. Width/height in pixels
            // *including* the frame (e08 shows how to get an exact
            // client area instead).
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            720,
            480,
            // --- parent ------------------------------------------------
            // None → a top-level window. A child window would pass its
            // parent here and add WS_CHILD to the style (e41+).
            None,
            // --- menu --------------------------------------------------
            // For a top-level window: the class/menu bar handle. None = no
            // menu. (For child windows this same parameter identifies the
            // child's control ID — confusing but traditional.)
            None,
            // --- instance ----------------------------------------------
            // The module that owns the window; usually your exe.
            Some(instance),
            // --- lpparam ------------------------------------------------
            // Extra data delivered to WM_CREATE. We don't need any; the
            // GWLP_USERDATA pattern in e65 is the modern alternative.
            None,
        );
        debug_assert!(!hwnd.is_null(), "CreateWindowExW failed");

        // ============================================================
        // Showing the window — the WS_VISIBLE shortcut, spelled out
        // ============================================================
        // ShowWindow makes the window visible and triggers the first
        // WM_PAINT. SW_SHOWNORMAL = "normal size" (SW_MAXIMIZE etc. exist).
        _ = ShowWindow(hwnd, SW_SHOWNORMAL);
        // UpdateWindow says "paint NOW, don't wait for the queue" — it
        // delivers WM_PAINT synchronously, skipping the wait in GetMessage.
        _ = UpdateWindow(hwnd);

        // The message pump (identical to e03/e04 — it never changes much).
        let mut message = MSG::default();
        while GetMessageW(&mut message, None, 0, 0).0 > 0 {
            _ = TranslateMessage(&message);
            DispatchMessageW(&message);
        }
    }
}

extern "system" fn wndproc(hwnd: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe {
        match message as i32 {
            WM_PAINT => {
                let mut ps = PAINTSTRUCT::default();
                let hdc = BeginPaint(hwnd, &mut ps);
                let mut rect = RECT::default();
                _ = GetClientRect(hwnd, &mut rect);

                // A little cheat-sheet rendered in the window itself.
                // `w!` only handles *compile-time* literals, so runtime
                // strings get the to_wide() treatment — e06 covers exactly
                // why this works and how to read strings back.
                let lines = [
                    "WNDCLASSW        blueprint: proc, cursor, background, name",
                    "CreateWindowExW  instance: class, title, style, size, parent, menu",
                    "ShowWindow       make visible (WS_VISIBLE is the shortcut)",
                    "UpdateWindow     paint right now, bypassing the queue",
                    "GetMessageW      DispatchMessageW -> wndproc -> your code",
                ];
                for (i, line) in lines.iter().enumerate() {
                    // Rect shrinks per line: quick manual text layout.
                    let mut r = rect;
                    r.top += 20 + i as i32 * 28;
                    let wide = to_wide(line); // must outlive the DrawTextW call
                    DrawTextW(
                        hdc,
                        PCWSTR::from_raw(wide.as_ptr()),
                        -1,
                        &mut r,
                        (DT_LEFT | DT_SINGLELINE) as u32,
                    );
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
