//! e44 — Multiline Edit: free scrolling, and how word-wrap changes the feel
//!
//! What you learn:
//! * `ES_MULTILINE` turns an EDIT into a (very) mini text editor — you
//!   rarely build it yourself because the control brings its own caret,
//!   selection, clipboard and **scrollbars** (`WS_VSCROLL` + `ES_AUTOVSCROLL`)
//! * `ES_WANTRETURN`: without it, pressing Enter *clicks the default
//!   button* (dialog behavior) instead of inserting a newline
//! * Why `ES_AUTOHSCROLL` (off here) is the word-wrap switch: with it,
//!   lines grow sideways forever and the control grows a horizontal
//!   scrollbar; without it, text wraps at the right edge like Notepad
//!
//! New Win32 APIs: `ES_MULTILINE`, `ES_AUTOVSCROLL`, `ES_WANTRETURN`,
//! `WS_VSCROLL`, `EM_SETRECT` (mentioned).
//!
//! Run: `cargo run -p e44_edit_multiline`
//!
//! Try it: type more lines than fit, scroll with the wheel or the scrollbar,
//! select with the mouse, paste with Ctrl+V — all of it is free.

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
            lpszClassName: w!("e44_class"),
            style: (CS_HREDRAW | CS_VREDRAW) as u32,
            ..Default::default()
        };
        debug_assert!(RegisterClassW(&wc) != 0);

        let hwnd = CreateWindowExW(
            0,
            w!("e44_class"),
            w!("e44 · Multiline Edit — scrolling is free"),
            (WS_OVERLAPPEDWINDOW | WS_VISIBLE) as u32,
            CW_USEDEFAULT, CW_USEDEFAULT, 560, 420,
            None, None, Some(instance), None,
        );
        debug_assert!(!hwnd.is_null());

        // ---- The multiline EDIT -------------------------------------------------
        // Style-by-style, because each one changes the feel:
        //   ES_MULTILINE     — the multi-line engine (mandatory for the rest)
        //   ES_AUTOVSCROLL   — caret scrolls into view as you type past the end
        //   WS_VSCROLL       — a *window* scrollbar on the right edge. The
        //                      control implements all scrolling itself; we
        //                      never see a WM_VSCROLL for it (contrast e49!).
        //   ES_WANTRETURN    — Enter inserts \r\n instead of "click default
        //                      button" (dialog convention leaking through)
        //   WS_EX_CLIENTEDGE — the sunken border users expect from a text area
        // NOTE: ES_AUTOHSCROLL is deliberately NOT set. That single flag is
        // the word-wrap switch:
        //   * WITH ES_AUTOHSCROLL: long lines extend horizontally, the control
        //     grows an H scrollbar, Enter alone breaks lines (Notepad's
        //     "word wrap off" mode). Add WS_HSCROLL to see it.
        //   * WITHOUT it (here): text wraps at the right edge — softer,
        //     reading-friendly, "word wrap on".
        let hwnd_edit = CreateWindowExW(
            WS_EX_CLIENTEDGE as usize as u32,
            w!("EDIT"),
            w!(""),
            (WS_CHILD | WS_VISIBLE | ES_MULTILINE | ES_AUTOVSCROLL | ES_WANTRETURN | WS_VSCROLL) as u32,
            20, 20, 500, 340,
            Some(hwnd), None, Some(instance), None,
        );
        debug_assert!(!hwnd_edit.is_null());

        // Load a long paragraph as the initial content. SetWindowTextW
        // replaces the whole text; the control reflows and re-scrolls for us.
        // In a real editor EM_SETRECT is the related call that shrinks the
        // *formatting rectangle* (the area text occupies inside the control)
        // — handy to reserve a margin, but the default is usually fine.
        let paragraph = "The multiline EDIT control is a small text editor \
that Windows ships in the box. It brings its own caret and selection, \
supports the clipboard, undo, and — most importantly for this example — \
scrolling: give it WS_VSCROLL and ES_AUTOVSCROLL and every line you type \
past the bottom simply scrolls into view. No message handling on our side.\n\n\
Scroll me with the wheel. Select me with the mouse. Type here — I am a \
real, focusable, editable control, not a painted rectangle.\n\n\
And because ES_AUTOHSCROLL is off, this long paragraph wraps at the right \
edge instead of running off sideways. One style bit, completely different \
feel — try adding ES_AUTOHSCROLL | WS_HSCROLL to the CreateWindowExW call \
and compare.";
        let wide: Vec<u16> = paragraph.encode_utf16().chain(std::iter::once(0)).collect();
        _ = SetWindowTextW(hwnd_edit, PCWSTR::from_raw(wide.as_ptr()));

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
            WM_DESTROY => {
                PostQuitMessage(0);
                0
            }
            _ => DefWindowProcW(hwnd, message, wparam, lparam),
        }
    }
}
