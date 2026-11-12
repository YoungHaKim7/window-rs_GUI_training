//! e89 — The Clipboard: handing memory to the system and getting it back
//!
//! What you learn:
//! * The clipboard dance: `OpenClipboard` → `EmptyClipboard` →
//!   `SetClipboardData` → `CloseClipboard` — and the iron rule that after a
//!   successful `SetClipboardData` the *system owns* your memory; you never
//!   free it yourself
//! * `GMEM_MOVEABLE` and the 16-bit legacy: clipboard data must come from the
//!   ancient global heap (`GlobalAlloc`), allocated movable, locked just long
//!   enough to write (`GlobalLock`/`GlobalUnlock`), then handed over unlocked
//! * `CF_UNICODETEXT` — the standard format for text since Windows NT:
//!   UTF-16 code units plus a terminating NUL. (Docs sometimes say
//!   `CF_UNICODEFORMAT`; in these bindings the constant is `CF_UNICODETEXT`.)
//! * Pasting is the mirror image: `IsClipboardFormatAvailable` →
//!   `GetClipboardData` → `GlobalLock` → copy out → `GlobalUnlock` — the
//!   data still belongs to the clipboard, you only *borrow* the lock.
//!
//! Type into the edit control, press Copy, then Paste from anywhere — or
//! paste in text copied from another app. The clipboard is global to the
//! session: every program shares it.
//!
//! New Win32 APIs: `OpenClipboard`, `EmptyClipboard`, `SetClipboardData`,
//! `CloseClipboard`, `IsClipboardFormatAvailable`, `GetClipboardData`
//! (winuser); `GlobalAlloc`, `GlobalLock`, `GlobalUnlock` (winbase).
//!
//! Run: `cargo run -p e89_clipboard`

use windows::{Win32::*, core::*};

const IDC_EDIT: i32 = 1001;
const IDC_COPY: i32 = 1002;
const IDC_PASTE: i32 = 1003;

#[derive(Default)]
struct State {
    hwnd_edit: HWND,
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
            lpszClassName: w!("e89_class"),
            style: (CS_HREDRAW | CS_VREDRAW) as u32,
            ..Default::default()
        };
        debug_assert!(RegisterClassW(&wc) != 0);

        let hwnd = CreateWindowExW(
            0,
            w!("e89_class"),
            w!("e89 · Clipboard — Copy/Paste via CF_UNICODETEXT"),
            (WS_OVERLAPPEDWINDOW | WS_VISIBLE) as u32,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            560,
            300,
            None,
            None,
            Some(instance),
            None,
        );
        debug_assert!(!hwnd.is_null());

        let state = Box::into_raw(Box::<State>::default());
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, state as isize);

        // A multiline edit as the text playground, plus the two buttons.
        let edit = CreateWindowExW(
            0,
            w!("EDIT"),
            w!("Type something here, press Copy, then Paste it back."),
            (WS_CHILD | WS_VISIBLE | WS_BORDER | ES_MULTILINE | ES_WANTRETURN | WS_VSCROLL) as u32,
            20,
            20,
            500,
            160,
            Some(hwnd),
            Some(IDC_EDIT as usize as HMENU),
            Some(instance),
            None,
        );
        debug_assert!(!edit.is_null());

        let copy = CreateWindowExW(
            0,
            w!("BUTTON"),
            w!("Copy"),
            (WS_CHILD | WS_VISIBLE | BS_PUSHBUTTON) as u32,
            20,
            200,
            110,
            34,
            Some(hwnd),
            Some(IDC_COPY as usize as HMENU),
            Some(instance),
            None,
        );
        debug_assert!(!copy.is_null());

        let paste = CreateWindowExW(
            0,
            w!("BUTTON"),
            w!("Paste"),
            (WS_CHILD | WS_VISIBLE | BS_PUSHBUTTON) as u32,
            140,
            200,
            110,
            34,
            Some(hwnd),
            Some(IDC_PASTE as usize as HMENU),
            Some(instance),
            None,
        );
        debug_assert!(!paste.is_null());

        if let Some(s) = state_of(hwnd) {
            s.hwnd_edit = edit;
        }

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

/// Read the edit control's whole text as a Rust String (no NUL — that gets
/// appended when we serialize for the clipboard).
unsafe fn edit_text(hwnd_edit: HWND) -> String {
    unsafe {
        let len = GetWindowTextLengthW(hwnd_edit);
        let mut buf = vec![0u16; len as usize + 1];
        let copied = GetWindowTextW(hwnd_edit, PWSTR(buf.as_mut_ptr()), buf.len() as i32);
        String::from_utf16_lossy(&buf[..copied.max(0) as usize])
    }
}

/// Copy the edit control's text onto the clipboard as CF_UNICODETEXT.
// (std::result::Result spelled out: `core::*` re-exports windows' own
// one-generic Result alias, which would shadow the std type here.)
unsafe fn copy_to_clipboard(owner: HWND, text: &str) -> std::result::Result<(), String> {
    unsafe {
        // Serialize to UTF-16 with the NUL the format requires. The byte
        // count (not code-unit count!) is what GlobalAlloc wants.
        let wide: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
        let bytes = wide.len() * std::mem::size_of::<u16>();

        // GMEM_MOVEABLE is the 16-bit Windows relic: clipboard data must be
        // "movable" global memory, because the system may hand the block
        // between processes. Nobody writes new code with this allocator —
        // except every clipboard user, forever.
        if !OpenClipboard(Some(owner)).as_bool() {
            return Err("another window has the clipboard open".into());
        }

        // EmptyClipboard is required: it marks the clipboard as owned by
        // this window AND frees the previous contents.
        if !EmptyClipboard().as_bool() {
            _ = CloseClipboard();
            return Err("EmptyClipboard failed".into());
        }

        let hglobal = GlobalAlloc(GMEM_MOVEABLE as u32, bytes);
        if hglobal.is_null() {
            _ = CloseClipboard();
            return Err("GlobalAlloc failed".into());
        }

        // Lock just long enough to copy the payload in. GlobalLock on a
        // movable block returns a temporary pointer; it stops being valid at
        // GlobalUnlock.
        let dst = GlobalLock(hglobal);
        if dst.is_null() {
            _ = GlobalFree(hglobal);
            _ = CloseClipboard();
            return Err("GlobalLock failed".into());
        }
        std::ptr::copy_nonoverlapping(wide.as_ptr(), dst as *mut u16, wide.len());
        _ = GlobalUnlock(hglobal);

        // Hand it over. From here the SYSTEM owns the memory: do not free,
        // do not touch, do not pass Go. (If the call FAILS it returns null
        // and ownership stays with us — hence the cleanup branch.)
        if SetClipboardData(CF_UNICODETEXT as u32, Some(hglobal)).is_null() {
            _ = GlobalFree(hglobal);
            _ = CloseClipboard();
            return Err("SetClipboardData failed".into());
        }

        _ = CloseClipboard();
        Ok(())
    }
}

/// Read CF_UNICODETEXT off the clipboard and return it as a Rust String.
unsafe fn paste_from_clipboard(owner: HWND) -> std::result::Result<String, String> {
    unsafe {
        // Cheap pre-check that avoids opening the clipboard for formats we
        // don't support anyway.
        if !IsClipboardFormatAvailable(CF_UNICODETEXT as u32).as_bool() {
            return Err("no text on the clipboard".into());
        }

        if !OpenClipboard(Some(owner)).as_bool() {
            return Err("another window has the clipboard open".into());
        }

        // GetClipboardData returns a handle OWNED BY THE CLIPBOARD — we only
        // lock it, copy the payload out, unlock. Never free this handle.
        let handle = GetClipboardData(CF_UNICODETEXT as u32);
        if handle.is_null() {
            _ = CloseClipboard();
            return Err("GetClipboardData failed".into());
        }

        let src = GlobalLock(handle);
        if src.is_null() {
            _ = CloseClipboard();
            return Err("GlobalLock failed".into());
        }

        // Walk to the NUL — the clipboard gives us a length-less buffer.
        let mut len = 0usize;
        while *(src as *const u16).add(len) != 0 {
            len += 1;
        }
        let text = String::from_utf16_lossy(std::slice::from_raw_parts(src as *const u16, len));

        _ = GlobalUnlock(handle);
        _ = CloseClipboard();
        Ok(text)
    }
}

extern "system" fn wndproc(hwnd: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe {
        let Some(state) = state_of(hwnd) else {
            return DefWindowProcW(hwnd, message, wparam, lparam);
        };

        match message as i32 {
            WM_COMMAND => {
                let notification = ((wparam >> 16) & 0xffff) as u32;
                let control_id = (wparam & 0xffff) as u32;
                if notification == BN_CLICKED as u32 {
                    match control_id as i32 {
                        IDC_COPY => {
                            // Take the borrow OUT of state use first: the error
                            // path below runs SetWindowTextW, which can
                            // re-enter wndproc (e65's aliasing rule).
                            let text = edit_text(state.hwnd_edit);
                            match copy_to_clipboard(hwnd, &text) {
                                Ok(()) => {
                                    _ = SetWindowTextW(hwnd, w!("e89 · copied to the clipboard"))
                                }
                                Err(e) => {
                                    _ = SetWindowTextW(
                                        hwnd,
                                        &HSTRING::from(format!("e89 · copy failed: {e}")),
                                    )
                                }
                            }
                        }
                        IDC_PASTE => match paste_from_clipboard(hwnd) {
                            Ok(text) => {
                                _ = SetWindowTextW(state.hwnd_edit, &HSTRING::from(text));
                                _ = SetWindowTextW(hwnd, w!("e89 · pasted from the clipboard"))
                            }
                            Err(e) => {
                                _ = SetWindowTextW(
                                    hwnd,
                                    &HSTRING::from(format!("e89 · paste failed: {e}")),
                                )
                            }
                        },
                        _ => {}
                    }
                }
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
