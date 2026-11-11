//! e63 — Folder Browse: SHBrowseForFolderW and the pidl you must free
//!
//! What you learn:
//! * `SHBrowseForFolderW` — the tree-style "Browse for Folder" dialog
//!   (the old-school one; see the note at the bottom for the modern
//!   replacement). You describe it with a `BROWSEINFOW`: owner, an
//!   instruction line, and `ulFlags` — `BIF_RETURNONLYFSDIRS` keeps the
//!   tree to real filesystem directories
//! * **The PIDL dance**: the dialog returns an ITEMIDLIST pointer — a
//!   Shell-namespace address, not a path. `SHGetPathFromIDListW` converts
//!   it to a drive-letter path you can actually open; then you free the
//!   pidl yourself with `CoTaskMemFree` (it came from the Shell's
//!   allocator, so THAT allocator must return it — not Rust's, not
//!   LocalFree)
//! * The conversion can fail — some shell locations (Control Panel, the
//!   desktop root...) have no filesystem path even with
//!   BIF_RETURNONLYFSDIRS; handle the empty string
//!
//! Note: modern apps prefer `IFileDialog` (COM, with FOS_PICKFOLDERS) —
//! same dialog the file pickers use. That needs COM interfaces, which is
//! beyond this chapter's scope; the API here still works everywhere.
//!
//! New Win32 APIs: `SHBrowseForFolderW`, `BROWSEINFOW`,
//! `SHGetPathFromIDListW`, `CoTaskMemFree`.
//!
//! Run: `cargo run -p e63_folder_browse`

use windows::{Win32::*, core::*};

/// Main-window button that opens the browse dialog.
const IDC_BROWSE: i32 = 1001;

#[derive(Default)]
struct State {
    folder: String, // last picked path ("" = none yet)
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
            lpszClassName: w!("e63_class"),
            style: (CS_HREDRAW | CS_VREDRAW) as u32,
            ..Default::default()
        };
        debug_assert!(RegisterClassW(&wc) != 0);

        let hwnd = CreateWindowExW(
            0,
            w!("e63_class"),
            w!("e63 · Folder Browse — pick a directory"),
            (WS_OVERLAPPEDWINDOW | WS_VISIBLE) as u32,
            CW_USEDEFAULT, CW_USEDEFAULT, 520, 280,
            None, None, Some(instance), None,
        );
        debug_assert!(!hwnd.is_null());

        let state = Box::into_raw(Box::<State>::default());
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, state as isize);

        let browse = CreateWindowExW(
            0,
            w!("BUTTON"),
            w!("Browse for folder..."),
            (WS_CHILD | WS_VISIBLE | BS_PUSHBUTTON) as u32,
            20, 20, 160, 32,
            Some(hwnd), Some(IDC_BROWSE as usize as HMENU), Some(instance), None,
        );
        debug_assert!(!browse.is_null());

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

fn to_wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

fn wide_to_string(buf: &[u16]) -> String {
    let end = buf.iter().position(|&u| u == 0).unwrap_or(buf.len());
    String::from_utf16_lossy(&buf[..end])
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
                if notification == BN_CLICKED as u32 && control_id == IDC_BROWSE as u32 {
                    // The dialog writes the chosen item's display name
                    // here (its label — not necessarily its path).
                    let mut name_buf = vec![0u16; MAX_PATH as usize];

                    let mut bi = BROWSEINFOW {
                        hwndOwner: hwnd,
                        pidlRoot: std::ptr::null(), // desktop = the tree root
                        pszDisplayName: PWSTR(name_buf.as_mut_ptr()),
                        lpszTitle: w!("Pick a folder — the title bar will show it:"),
                        ulFlags: BIF_RETURNONLYFSDIRS as u32,
                        ..Default::default() // no callback, no lParam
                    };

                    // Null return = user canceled. Otherwise: a PIDL into
                    // the Shell namespace — the Shell's own pointer,
                    // allocated with its allocator.
                    let pidl = SHBrowseForFolderW(&mut bi);
                    if !pidl.is_null() {
                        // PIDL → filesystem path (the conversion every
                        // caller performs immediately — that's why
                        // SHGetPathFromIDList exists).
                        let mut path_buf = vec![0u16; MAX_PATH as usize];
                        if SHGetPathFromIDListW(pidl, PWSTR(path_buf.as_mut_ptr())).0 != 0 {
                            state.folder = wide_to_string(&path_buf);
                            _ = SetWindowTextW(
                                hwnd,
                                &HSTRING::from(format!("e63 · {}", state.folder)),
                            );
                        } else {
                            // Chosen shell item has no path (a virtual
                            // folder sneaked in). Keep the old one.
                            _ = SetWindowTextW(
                                hwnd,
                                w!("e63 · that location has no filesystem path"),
                            );
                        }
                        // Free the pidl with the allocator that made it:
                        // CoTaskMemFree, always, for shell-allocated pidls.
                        CoTaskMemFree(pidl as LPVOID);
                    } else {
                        _ = SetWindowTextW(hwnd, w!("e63 · canceled"));
                    }
                }
                0
            }
            WM_PAINT => {
                let mut ps = PAINTSTRUCT::default();
                let hdc = BeginPaint(hwnd, &mut ps);
                let text = if state.folder.is_empty() {
                    to_wide("no folder picked yet")
                } else {
                    to_wide(&format!("folder: {}", state.folder))
                };
                _ = SetBkMode(hdc, TRANSPARENT);
                _ = TextOutW(
                    hdc,
                    20,
                    90,
                    PCWSTR::from_raw(text.as_ptr()),
                    text.len() as i32 - 1,
                );
                _ = SetBkMode(hdc, OPAQUE);
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
