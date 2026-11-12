//! e65 — Window State, the Deep Version: attaching state via WM_NCCREATE
//!
//! What you learn:
//! * The e13 pattern had a gap: the state pointer was attached *after*
//!   `CreateWindowExW` returned, so the handful of messages Windows sends
//!   *during* creation (WM_NCCREATE, WM_NCCALCSIZE, WM_CREATE, ...) fell
//!   through to `DefWindowProcW`. This example closes that gap: the pointer
//!   rides the `lpparam` parameter of `CreateWindowExW`, wndproc catches it
//!   in **WM_NCCREATE** — the *very first* message a window receives — via
//!   the `CREATESTRUCTW` that `lparam` points to, and calls
//!   `SetWindowLongPtrW` immediately. From that point on state exists for
//!   every message, including `WM_CREATE`.
//! * Cleanup moves to **WM_NCDESTROY** — the *last* message a window
//!   receives, the mirror image of WM_NCCREATE. Freeing in WM_DESTROY also
//!   works for a single window, but WM_NCDESTROY is the formally correct
//!   "nothing of this window remains" point.
//! * The two safety rules that keep this pattern sound:
//!     1. **Never take ownership of the pointer twice** — reclaim it in
//!        exactly one place, and null the slot *before* `Box::from_raw` so
//!        a re-entrant message can't fish out a dangling pointer.
//!     2. **Never hold the `&mut State` borrow across a call that can
//!        re-enter wndproc** (`MessageBoxW`, `SendMessageW`, ...) — the
//!        nested call would fetch a second `&mut` to the same data and you
//!        have aliasing UB. End the borrow first, re-fetch afterwards.
//!
//! New Win32 APIs: `WM_NCCREATE`/`CREATESTRUCTW` (creation data plumbing),
//! `WM_NCDESTROY` (the teardown mirror).
//!
//! Run: `cargo run -p e65_window_state`

use windows::{Win32::*, core::*};

#[derive(Default)]
struct State {
    /// How many messages this window has seen since WM_NCCREATE attached
    /// the state. If this pattern works, the count starts climbing during
    /// CreateWindowExW itself — proof that state exists "for all messages".
    messages: u32,
    clicks: u32,
    /// Set in WM_CREATE — impossible in the e13 flavor, which is exactly
    /// the point of this example.
    created_in_wm_create: bool,
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
            lpszClassName: w!("e65_class"),
            style: (CS_HREDRAW | CS_VREDRAW) as u32,
            ..Default::default()
        };
        debug_assert!(RegisterClassW(&wc) != 0);

        // Build the state BEFORE the window exists, and hand its raw
        // pointer to CreateWindowExW. The last parameter — which e04/e13
        // always passed as None — is delivered untouched to WM_NCCREATE
        // and WM_CREATE inside the lpCreateParams field of CREATESTRUCTW.
        let state = Box::into_raw(Box::<State>::default());

        let hwnd = CreateWindowExW(
            0,
            w!("e65_class"),
            w!("e65 · Window State, attached at WM_NCCREATE"),
            (WS_OVERLAPPEDWINDOW | WS_VISIBLE) as u32,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            640,
            320,
            None,
            None,
            Some(instance),
            Some(state as LPVOID), // <-- rides along into WM_NCCREATE
        );
        debug_assert!(!hwnd.is_null());

        // If WM_NCCREATE rejected creation (it can!), CreateWindowExW
        // returns null WITHOUT ever sending WM_NCDESTROY — the state would
        // leak. Reclaim it here in that failure case. On success the box
        // is now owned by the window and must NOT be touched again.
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

/// Same helper as e13 — but now it can only return None before WM_NCCREATE
/// (which never happens for messages you can act on) and after the free in
/// WM_NCDESTROY. In other words: effectively always Some after creation.
unsafe fn state_of(hwnd: HWND) -> Option<&'static mut State> {
    unsafe {
        let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut State;
        (!ptr.is_null()).then(|| &mut *ptr)
    }
}

fn to_wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

extern "system" fn wndproc(hwnd: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe {
        match message as i32 {
            // ---- The very first message --------------------------------
            // lparam points to a CREATESTRUCTW filled in from the
            // CreateWindowExW arguments; its lpCreateParams is our Box
            // pointer, passed through verbatim. Attach it NOW and the
            // state is available for everything that follows — including
            // WM_CREATE further down.
            WM_NCCREATE => {
                let cs = lparam as *const CREATESTRUCTW;
                let state = (*cs).lpCreateParams as *mut State;
                SetWindowLongPtrW(hwnd, GWLP_USERDATA, state as isize);

                // Let DefWindowProcW do the actual non-client creation.
                // (Returning 0 here would abort CreateWindowExW.)
                DefWindowProcW(hwnd, message, wparam, lparam)
            }

            // ---- Proof that state exists at creation time --------------
            // e13's flavor could never do this: WM_CREATE ran before the
            // pointer was attached. Here we can read AND write state.
            WM_CREATE => {
                let Some(state) = state_of(hwnd) else {
                    // Unreachable given the WM_NCCREATE arm above; kept
                    // for the same defensive habit as e13.
                    return DefWindowProcW(hwnd, message, wparam, lparam);
                };
                state.created_in_wm_create = true;
                DefWindowProcW(hwnd, message, wparam, lparam)
            }

            WM_LBUTTONDOWN => {
                let Some(state) = state_of(hwnd) else {
                    return DefWindowProcW(hwnd, message, wparam, lparam);
                };
                state.clicks += 1;
                state.messages += 1; // counted by hand; see the general note below

                // *** The re-entrancy lesson ***
                // MessageBoxW runs its own message loop: while it is on
                // screen, wndproc keeps firing (WM_PAINT, WM_NCCREATE of
                // the box itself, ...). If we still held `state` across
                // this call, one of those nested invocations would fetch
                // a SECOND &mut to the same State — instant aliasing UB.
                // The borrow ends here because we don't touch it again:
                let text = format!("Click #{} — borrow released before this box.", state.clicks);
                _ = InvalidateRect(Some(hwnd), None, true);

                MessageBoxW(
                    Some(hwnd),
                    &HSTRING::from(text),
                    w!("still alive?"),
                    (MB_OK | MB_ICONINFORMATION) as u32,
                );

                // ...and only now, after the re-entering call returned, is
                // it safe to fetch a fresh borrow:
                if let Some(state) = state_of(hwnd) {
                    state.messages += 1; // messages that arrived inside the box count too
                }
                _ = InvalidateRect(Some(hwnd), None, true);
                0
            }

            WM_PAINT => {
                let mut ps = PAINTSTRUCT::default();
                let hdc = BeginPaint(hwnd, &mut ps);
                let mut rect = RECT::default();
                _ = GetClientRect(hwnd, &mut rect);

                let Some(state) = state_of(hwnd) else {
                    _ = EndPaint(hwnd, &ps);
                    return 0;
                };

                let lines = [
                    format!("messages seen since WM_NCCREATE: {}", state.messages),
                    format!("clicks: {}", state.clicks),
                    format!(
                        "state was usable in WM_CREATE: {}",
                        state.created_in_wm_create
                    ),
                ];
                for (i, line) in lines.iter().enumerate() {
                    let mut r = rect;
                    r.top += 20 + i as i32 * 28;
                    let wide = to_wide(line);
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

            // ---- The very last message ---------------------------------
            // WM_DESTROY arrives while the window still exists (children
            // are alive); WM_NCDESTROY is sent once it is truly gone —
            // the symmetric place to undo what WM_NCCREATE did.
            WM_NCDESTROY => {
                // Rule 1: take the pointer out FIRST (so any message that
                // somehow still arrives sees null and takes the default
                // path), then reclaim the Box. Exactly one from_raw, ever.
                let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut State;
                if !ptr.is_null() {
                    SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
                    drop(Box::from_raw(ptr));
                }
                DefWindowProcW(hwnd, message, wparam, lparam)
            }

            WM_DESTROY => {
                // PostQuitMessage here as usual — WM_NCDESTROY follows.
                PostQuitMessage(0);
                0
            }

            _ => {
                // General-message counter: proves by the number alone that
                // the state slot is populated for ordinary traffic.
                if let Some(state) = state_of(hwnd) {
                    state.messages += 1;
                }
                DefWindowProcW(hwnd, message, wparam, lparam)
            }
        }
    }
}
