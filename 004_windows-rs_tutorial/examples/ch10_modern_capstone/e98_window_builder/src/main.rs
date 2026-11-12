//! e98 — Window Builder: wrapping Win32 boilerplate in a fluent Rust API
//!
//! What you learn:
//! * How every "Win32 wrapper library" is built: a builder collects the
//!   options (`.title()`, `.size()`, `.on_message(...)`), `run()` does the
//!   RegisterClass / CreateWindowExW / GetMessage dance from e03, and the
//!   application's logic shrinks to **one closure**
//! * The dispatch mechanism: your closure lives in a
//!   `Box<dyn FnMut(HWND, u32, WPARAM, LPARAM) -> Option<LRESULT>>`, the
//!   wndproc fetches it from `GWLP_USERDATA` and calls it — `Some(result)`
//!   means "handled", `None` means "fall through to `DefWindowProcW`".
//!   That Option-as-handled convention is exactly how the vendored
//!   `windows-window` crate (and most GUI frameworks) route messages
//! * One subtle necessity: the closure box is **double-boxed**
//!   (`Box<Box<dyn FnMut>>`) — `Box<dyn FnMut>` is a FAT pointer
//!   (data + vtable), and `SetWindowLongPtrW` stores one machine word.
//!   The outer box pins the fat pointer in heap memory so a thin `*mut`
//!   can ride the USERDATA slot
//! * The re-entrancy rule, still: while your closure runs, any call that
//!   dispatches messages synchronously (`SendMessageW` to your own window,
//!   `DestroyWindow`, most dialog APIs) re-enters the closure — keep
//!   handlers free of such calls or guard them (this is why the real
//!   windows-window crate is 500+ lines: close-request replay, DPI,
//!   resize plumbing)
//!
//! This example RE-DERIVES the vendored `windows-rs/crates/libs/window`
//! crate in ~90 teachable lines — compare after reading. Then see the
//! e04 hello-paint app reborn at the bottom of this file in ~20 lines.
//!
//! New Win32 APIs: none new — but now on the *supply* side: you wrote the
//! boilerplate once so you never have to write it per-app again.
//!
//! Run: `cargo run -p e98_window_builder`

use windows::{Win32::*, core::*};

/// The message handler: return `Some(result)` if you handled the message,
/// `None` to let DefWindowProcW answer it.
pub type Handler = Box<dyn FnMut(HWND, u32, WPARAM, LPARAM) -> Option<LRESULT>>;

/// A tiny fluent window builder. `WindowBuilder::new(title)` ->
/// chain options -> `run()` blocks in the message loop until close.
pub struct WindowBuilder {
    title: String,
    width: i32,
    height: i32,
    handler: Option<Handler>,
}

impl WindowBuilder {
    /// Start with a caption. Everything else has a default.
    pub fn new(title: &str) -> Self {
        Self {
            title: title.to_string(),
            width: 640,
            height: 360,
            handler: None,
        }
    }

    /// Override the caption after the fact (title as runtime String here —
    /// note how `run()` bridges it with HSTRING, vs the compile-time `w!`
    /// used for the class name).
    pub fn title(mut self, title: &str) -> Self {
        self.title = title.to_string();
        self
    }

    /// Outer window size in pixels.
    pub fn size(mut self, width: i32, height: i32) -> Self {
        self.width = width;
        self.height = height;
        self
    }

    /// Install the message handler. Required — `run()` panics without one.
    pub fn on_message(mut self, handler: Handler) -> Self {
        self.handler = Some(handler);
        self
    }

    /// Register the class, create the window, attach the handler, pump
    /// messages until WM_QUIT. This is the entire e03 skeleton, once.
    pub fn run(self) {
        unsafe {
            let handler = self
                .handler
                .expect("WindowBuilder needs .on_message(...) before .run()");

            let instance = GetModuleHandleW(PCWSTR::null());
            debug_assert!(!instance.is_null());

            let wc = WNDCLASSW {
                lpfnWndProc: Some(builder_wndproc),
                hInstance: instance,
                hCursor: LoadCursorW(None, PCWSTR(IDC_ARROW.0.cast())),
                hbrBackground: ((COLOR_WINDOW + 1) as isize) as HBRUSH,
                lpszClassName: w!("e98_builder_class"),
                style: (CS_HREDRAW | CS_VREDRAW) as u32,
                ..Default::default()
            };
            debug_assert!(RegisterClassW(&wc) != 0);

            let hwnd = CreateWindowExW(
                0,
                w!("e98_builder_class"),
                &HSTRING::from(self.title),
                (WS_OVERLAPPEDWINDOW | WS_VISIBLE) as u32,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                self.width,
                self.height,
                None,
                None,
                Some(instance),
                None,
            );
            debug_assert!(!hwnd.is_null());

            // The one trick that makes the whole design work: heap-allocate
            // the handler behind a THIN pointer. `Box<Handler>` is itself
            // fat (Handler is unsized), so box it again — the outer box is
            // a normal sized object at a stable address, and that address
            // is what USERDATA keeps.
            let boxed: Box<Handler> = Box::new(handler);
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, Box::into_raw(boxed) as isize);

            let mut message = MSG::default();
            while GetMessageW(&mut message, None, 0, 0).0 > 0 {
                _ = TranslateMessage(&message);
                DispatchMessageW(&message);
            }
        }
    }
}

/// The builder's single wndproc: dispatch to the attached closure, default
/// the rest. Note how short the "framework" is once the handler is a box.
extern "system" fn builder_wndproc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    unsafe {
        let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut Handler;

        if message as i32 == WM_DESTROY {
            // Let the app see the teardown, then undo the attach from
            // run() — null the slot BEFORE from_raw so a straggler message
            // takes the default path instead of fishing out a dangling
            // pointer (rule 1 of e65).
            if !ptr.is_null() {
                _ = (*ptr)(hwnd, message, wparam, lparam);
                SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
                drop(Box::from_raw(ptr));
            }
            PostQuitMessage(0);
            return 0;
        }

        // Messages before the attach (WM_NCCREATE..WM_CREATE arrive inside
        // CreateWindowExW) and after teardown find a null pointer: default.
        match ptr.is_null() {
            false => match (*ptr)(hwnd, message, wparam, lparam) {
                Some(result) => result,             // handled by the app
                None => DefWindowProcW(hwnd, message, wparam, lparam),
            },
            true => DefWindowProcW(hwnd, message, wparam, lparam),
        }
    }
}

// ---------------------------------------------------------------------------
// The payoff: e04's hello-paint app, rebuilt on the builder in ~20 lines.
// Compare with examples/ch01_first_window/e04_hello_paint — same behavior
// (paint on left-click), none of the scaffolding.
// ---------------------------------------------------------------------------
fn main() {
    WindowBuilder::new("e98 \u{b7} Window Builder \u{2014} e04 reborn")
        .size(560, 280)
        .on_message(Box::new(|hwnd, message, _wparam, _lparam| {
            // The whole "framework part" of a Win32 app is the match
            // below — everything above it used to be boilerplate.
            unsafe {
                match message as i32 {
                    WM_PAINT => {
                        let mut ps = PAINTSTRUCT::default();
                        let hdc = BeginPaint(hwnd, &mut ps);
                        let mut rect = RECT::default();
                        _ = GetClientRect(hwnd, &mut rect);
                        let text = to_wide("Hello, builder! \u{2014} click to repaint, close to quit.");
                        _ = SetBkMode(hdc, TRANSPARENT);
                        _ = DrawTextW(
                            hdc,
                            PCWSTR::from_raw(text.as_ptr()),
                            -1,
                            &mut rect,
                            (DT_CENTER | DT_VCENTER | DT_SINGLELINE) as u32,
                        );
                        _ = EndPaint(hwnd, &ps);
                        Some(0)
                    }
                    WM_LBUTTONDOWN => {
                        _ = InvalidateRect(Some(hwnd), None, true);
                        Some(0)
                    }
                    _ => None, // not ours: DefWindowProcW takes it
                }
            }
        }))
        .run();
}

fn to_wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}
