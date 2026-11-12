//! e03 — Hello, Window!: your first real window
//!
//! What you learn:
//! * The four steps every classic Windows program takes:
//!   1. register a *window class* (the blueprint)
//!   2. create a window from that class
//!   3. run the *message loop*
//!   4. answer messages in the *window procedure*
//! * Why "Hello, World!" shows up in the title bar here (the client area
//!   needs painting — that's e04)
//! * How clicking the X button ends the program: `WM_DESTROY` →
//!   `PostQuitMessage` → `GetMessageW` returns 0 → loop exits
//!
//! New Win32 APIs:
//! * `GetModuleHandleW` — the HINSTANCE (module handle) of our own .exe
//! * `RegisterClassW` — file the blueprint under a name Windows remembers
//! * `CreateWindowExW` — build a window from the blueprint
//! * `GetMessageW` / `TranslateMessage` / `DispatchMessageW` — the pump
//! * `DefWindowProcW` — default behavior for everything we don't handle
//!
//! Run: `cargo run -p e03_hello_window`

use windows::{Win32::*, core::*};

fn main() {
    unsafe {
        // 1. The module handle identifies our .exe to Windows; nearly every
        //    windowing call wants it. Passing a null string means "this exe".
        let instance = GetModuleHandleW(PCWSTR::null());
        debug_assert!(!instance.is_null());

        // 2. The window class: a blueprint Windows keeps for the whole
        //    process. Every window we create says which class it belongs to.
        let wc = WNDCLASSW {
            // Our message handler (below). Windows calls this function for
            // every event aimed at windows of this class.
            lpfnWndProc: Some(wndproc),

            hInstance: instance,

            // The standard arrow cursor when the mouse is over the window.
            // IDC_ARROW is really the ordinal 32512 smuggled through a
            // pointer value, hence the cast dance.
            hCursor: LoadCursorW(None, PCWSTR(IDC_ARROW.0.cast())),

            // Class background brush. (COLOR_WINDOW + 1) is the classic
            // "system window background" pseudo-handle.
            hbrBackground: ((COLOR_WINDOW + 1) as isize) as HBRUSH,

            // The class NAME. CreateWindowExW will look the class up by it.
            lpszClassName: w!("e03_class"),

            // Redraw the whole window when the user resizes it horizontally
            // or vertically (otherwise stale pixels would stick around).
            style: (CS_HREDRAW | CS_VREDRAW) as u32,

            // hIcon, lpszMenuName, cbClsExtra, cbWndExtra: zero/none.
            ..Default::default()
        };

        let atom = RegisterClassW(&wc);
        debug_assert!(atom != 0, "RegisterClassW failed");

        // 3. Build an actual window from the class. Note the "W" string
        //    parameters: class name must match the registration above.
        let hwnd = CreateWindowExW(
            0,                                         // no extended styles
            w!("e03_class"),                           // the blueprint to use
            w!("Hello, World!"),                       // title-bar caption
            (WS_OVERLAPPEDWINDOW | WS_VISIBLE) as u32, // standard frame + show immediately
            CW_USEDEFAULT,                             // x: let Windows place it
            CW_USEDEFAULT,                             // y
            CW_USEDEFAULT,                             // width
            CW_USEDEFAULT,                             // height
            None,                                      // no parent → top-level window
            None,                                      // no menu
            Some(instance),                            // owning module
            None,                                      // no extra creation data
        );
        debug_assert!(!hwnd.is_null(), "CreateWindowExW failed");

        // 4. The message loop. Windows queues events (input, painting,
        //    timers, ...) on a per-thread queue; this loop drains it for the
        //    lifetime of the program:
        //
        //      GetMessageW      take one message off the queue (blocks when empty)
        //      TranslateMessage turn raw key presses into WM_CHAR messages
        //      DispatchMessageW deliver the message to our wndproc
        //
        //    GetMessageW returns 0 only when it fetches WM_QUIT — which
        //    PostQuitMessage (see wndproc) puts on the queue at shutdown.
        //    (.0 > 0 also treats the documented -1 error case as "stop".)
        let mut message = MSG::default();
        while GetMessageW(&mut message, None, 0, 0).0 > 0 {
            _ = TranslateMessage(&message);
            DispatchMessageW(&message);
        }
    }
}

/// The window procedure: Windows calls this for every message destined for
/// windows of our class. `extern "system"` selects the Win32 ABI.
extern "system" fn wndproc(hwnd: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe {
        // WM_* constants are i32 in the windows crate, hence the cast.
        match message as i32 {
            // The window is going away (the X button ran through
            // WM_CLOSE → DestroyWindow → WM_DESTROY). PostQuitMessage makes
            // GetMessageW return 0, ending the loop in main.
            WM_DESTROY => {
                PostQuitMessage(0);
                0
            }

            // Anything we don't handle gets default treatment: dragging,
            // resizing, minimizing, the close button, ... all for free.
            _ => DefWindowProcW(hwnd, message, wparam, lparam),
        }
    }
}
