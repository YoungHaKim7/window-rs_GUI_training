//! e82 — Todo List, Persistent: files, Ctrl+S, and a Recent menu
//!
//! What you learn:
//! * **Persistence next to the exe**: `std::env::current_exe()` →
//!   `.parent()` → `todo.txt`. The on-disk format is one text line per
//!   task, `"1|text"` / `"0|text"` — the flag, a pipe, the text (the
//!   obvious limitation: a `|` in the text corrupts the split; real
//!   formats escape or use length prefixes).
//! * The accelerator loop (e37 recap): `CreateAcceleratorTableW` maps
//!   Ctrl+S to the Save command id, and `TranslateAcceleratorW` goes
//!   FIRST in the pump, before TranslateMessage — returning nonzero
//!   means "swallowed, already reposted as WM_COMMAND".
//! * A **dynamic submenu**: File → Recent holds the last three paths
//!   touched this session. It is rebuilt in place (remove items,
//!   re-append) whenever the list changes — e36's dynamic-menu idea
//!   applied to real app state (in-memory only: recents die with the
//!   process, which is fine for a tutorial).
//! * Save on EVERY exit path: [X], menu Exit, and WM_QUIT all funnel
//!   through WM_CLOSE, which saves before DestroyWindow. Forgetting
//!   one exit path is how apps lose user data.
//!
//! Run: `cargo run -p e82_todo_persist`
//! (add tasks, close, reopen — they're back; todo.txt sits next to the exe)

use std::path::PathBuf;
use windows::{Win32::*, core::*};

const IDC_ENTRY: i32 = 1001;
const IDC_ADD: i32 = 1002;
const IDC_LIST: i32 = 1003;

const IDM_NEW: usize = 4001;
const IDM_OPEN: usize = 4002;
const IDM_SAVE: usize = 4003;
const IDM_SAVE_AS: usize = 4004;
const IDM_EXIT: usize = 4005;
const IDM_RECENT_BASE: usize = 4100; // 4100..4102 = recent entries

struct State {
    items: Vec<(String, bool)>,
    /// The file we loaded from / save to. None = never touched a file.
    path: Option<PathBuf>,
    /// Up to three most-recent paths (front = newest).
    recent: Vec<PathBuf>,
    /// The Recent submenu handle, kept for in-place rebuilds.
    recent_menu: HMENU,
}

fn main() {
    unsafe {
        let instance = GetModuleHandleW(PCWSTR::null());
        debug_assert!(!instance.is_null());

        // Ctrl+S -> IDM_SAVE. Same id as the menu item, so both input
        // surfaces share one WM_COMMAND arm (and the menu shows the
        // shortcut hint for free — the \t in the label).
        let mut accels = [ACCEL {
            fVirt: (FVIRTKEY | FCONTROL) as u8,
            key: 'S' as u16,
            cmd: IDM_SAVE as u16,
        }];
        let haccel = CreateAcceleratorTableW(accels.as_mut_ptr(), accels.len() as i32);
        debug_assert!(!haccel.is_null());

        let wc = WNDCLASSW {
            lpfnWndProc: Some(wndproc),
            hInstance: instance,
            hCursor: LoadCursorW(None, PCWSTR(IDC_ARROW.0.cast())),
            hbrBackground: ((COLOR_WINDOW + 1) as isize) as HBRUSH,
            lpszClassName: w!("e82_class"),
            style: (CS_HREDRAW | CS_VREDRAW) as u32,
            ..Default::default()
        };
        debug_assert!(RegisterClassW(&wc) != 0);

        let state = Box::into_raw(Box::new(State {
            items: Vec::new(),
            path: None,
            recent: Vec::new(),
            recent_menu: std::ptr::null_mut(),
        }));

        let hwnd = CreateWindowExW(
            0,
            w!("e82_class"),
            w!("e82 · Todo (persistent) — Ctrl+S saves"),
            (WS_OVERLAPPEDWINDOW | WS_VISIBLE) as u32,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            500,
            440,
            None,
            None,
            Some(instance),
            Some(state as LPVOID),
        );
        debug_assert!(!hwnd.is_null());
        if hwnd.is_null() {
            drop(Box::from_raw(state));
        }

        // ---- The pump, accelerator-first (e37) --------------------------
        let mut message = MSG::default();
        while GetMessageW(&mut message, None, 0, 0).0 > 0 {
            // Nonzero = the key matched the table: a WM_COMMAND was
            // already posted to our hwnd. Drop the raw key message.
            if TranslateAcceleratorW(hwnd, haccel, &mut message) != 0 {
                continue;
            }
            _ = TranslateMessage(&message);
            DispatchMessageW(&message);
        }
        _ = DestroyAcceleratorTable(haccel);
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

/// Default location: todo.txt NEXT TO THE EXE — not the CWD (which for
/// "cargo run" is the workspace root, and changes per launch method).
fn default_todo_path() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|p| p.to_path_buf()))
        .unwrap_or_default()
        .join("todo.txt")
}

/// "1|text" per line. Malformed lines load as open tasks (defensive).
fn load_items(path: &std::path::Path) -> Vec<(String, bool)> {
    match std::fs::read_to_string(path) {
        Ok(text) => text
            .lines()
            .filter(|l| !l.trim().is_empty())
            .map(|l| match l.split_once('|') {
                Some(("1", rest)) => (rest.to_string(), true),
                Some(("0", rest)) => (rest.to_string(), false),
                _ => (l.to_string(), false),
            })
            .collect(),
        Err(_) => Vec::new(),
    }
}

fn save_items(path: &std::path::Path, items: &[(String, bool)]) -> std::io::Result<()> {
    let text: String = items
        .iter()
        .map(|(t, d)| format!("{}|{t}\n", if *d { 1 } else { 0 }))
        .collect();
    std::fs::write(path, text)
}

/// Rebuild the Recent submenu from state.recent — delete everything,
/// append fresh (a menu is data; e36 made that point, apps live it).
unsafe fn rebuild_recent_menu(state: &mut State) {
    unsafe {
        let menu = state.recent_menu;
        if menu.is_null() {
            return;
        }
        // Remove all current entries (id range = our 4100.. ids).
        for i in 0..3 {
            _ = RemoveMenu(menu, (IDM_RECENT_BASE + i) as u32, MF_BYCOMMAND as u32);
        }
        if state.recent.is_empty() {
            _ = AppendMenuW(
                menu,
                MF_STRING as u32 | MF_GRAYED as u32,
                0,
                w!("(none yet)"),
            );
        } else {
            for (i, path) in state.recent.iter().take(3).enumerate() {
                let name = path
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_else(|| path.to_string_lossy().into_owned());
                let label = format!("&{} {name}", i + 1);
                let wide = to_wide(&label);
                _ = AppendMenuW(
                    menu,
                    MF_STRING as u32,
                    IDM_RECENT_BASE + i,
                    PCWSTR::from_raw(wide.as_ptr()),
                );
            }
        }
    }
}

/// Newest-first, deduped, capped at three.
fn remember_recent(state: &mut State, path: PathBuf) {
    state.recent.retain(|p| *p != path);
    state.recent.insert(0, path);
    state.recent.truncate(3);
    unsafe { rebuild_recent_menu(state) };
}

/// The one listbox writer: model -> view, whole list every time.
unsafe fn refill_list(hwnd: HWND) {
    unsafe {
        let Some(state) = state_of(hwnd) else { return };
        let list = GetDlgItem(Some(hwnd), IDC_LIST);
        SendMessageW(list, LB_RESETCONTENT as u32, 0, 0);
        for (text, done) in state.items.iter() {
            let label = if *done {
                format!("[x] {text}")
            } else {
                format!("[ ] {text}")
            };
            let wide = to_wide(&label);
            SendMessageW(list, LB_ADDSTRING as u32, 0, wide.as_ptr() as LPARAM);
        }
        let done = state.items.iter().filter(|(_, d)| *d).count();
        let name = state
            .path
            .as_ref()
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| String::from("Untitled"));
        _ = SetWindowTextW(
            hwnd,
            &HSTRING::from(format!(
                "{name} — {}/{} done — e82",
                done,
                state.items.len()
            )),
        );
    }
}

/// Save to the current path if any; returns true on success/no-op.
unsafe fn save_current(hwnd: HWND) -> bool {
    unsafe {
        let Some(state) = state_of(hwnd) else {
            return false;
        };
        // Borrow discipline (e65): MessageBoxW below re-enters wndproc,
        // so clone what we need and drop the &mut first.
        let path = state.path.clone();
        let items = state.items.clone();
        match path {
            None => true, // nothing to save to yet — Save As handles that
            Some(path) => {
                if let Err(err) = save_items(&path, &items) {
                    _ = MessageBoxW(
                        Some(hwnd),
                        &HSTRING::from(format!("Save failed:\n{err}")),
                        w!("e82"),
                        (MB_OK | MB_ICONERROR) as u32,
                    );
                    false
                } else {
                    true
                }
            }
        }
    }
}

/// The e59 picker (single file, text filter).
unsafe fn pick_file(owner: HWND, save: bool) -> Option<String> {
    unsafe {
        let mut blob = Vec::new();
        for part in ["Todo files (*.txt)", "*.txt", "All files (*.*)", "*.*"] {
            blob.extend(part.encode_utf16());
            blob.push(0);
        }
        blob.push(0);
        let mut file_buf = vec![0u16; 260];

        let mut ofn = OPENFILENAMEW {
            lStructSize: std::mem::size_of::<OPENFILENAMEW>() as u32,
            hwndOwner: owner,
            lpstrFilter: PCWSTR::from_raw(blob.as_ptr()),
            lpstrFile: PWSTR(file_buf.as_mut_ptr()),
            nMaxFile: file_buf.len() as u32,
            nFilterIndex: 1,
            Flags: if save {
                OFN_OVERWRITEPROMPT as u32
            } else {
                (OFN_PATHMUSTEXIST | OFN_FILEMUSTEXIST) as u32
            },
            ..Default::default()
        };
        let ok = if save {
            GetSaveFileNameW(&mut ofn).0 != 0
        } else {
            GetOpenFileNameW(&mut ofn).0 != 0
        };
        if ok {
            Some(wide_to_string(&file_buf))
        } else {
            None
        }
    }
}

/// Load a file into state + UI. Also used by Open and Recent.
unsafe fn open_into(hwnd: HWND, path: PathBuf) {
    unsafe {
        let items = load_items(&path);
        if let Some(state) = state_of(hwnd) {
            state.items = items;
            state.path = Some(path.clone());
            // remember_recent mutates through state again — end this
            // borrow by structuring: fetch once, act on it.
        }
        if let Some(state) = state_of(hwnd) {
            remember_recent(state, path);
        }
        refill_list(hwnd);
    }
}

extern "system" fn wndproc(hwnd: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe {
        match message as i32 {
            WM_NCCREATE => {
                let cs = lparam as *const CREATESTRUCTW;
                SetWindowLongPtrW(hwnd, GWLP_USERDATA, (*cs).lpCreateParams as isize);
                DefWindowProcW(hwnd, message, wparam, lparam)
            }

            WM_CREATE => {
                let cs = lparam as *const CREATESTRUCTW;
                let instance = (*cs).hInstance;

                let bar = CreateMenu();
                let file_menu = CreatePopupMenu();
                _ = AppendMenuW(file_menu, MF_STRING as u32, IDM_NEW, w!("&New"));
                _ = AppendMenuW(file_menu, MF_STRING as u32, IDM_OPEN, w!("&Open..."));
                _ = AppendMenuW(file_menu, MF_STRING as u32, IDM_SAVE, w!("&Save\tCtrl+S"));
                _ = AppendMenuW(file_menu, MF_STRING as u32, IDM_SAVE_AS, w!("Save &As..."));
                _ = AppendMenuW(file_menu, MF_SEPARATOR as u32, 0, w!(""));

                let recent_menu = CreatePopupMenu();
                _ = AppendMenuW(
                    file_menu,
                    MF_POPUP as u32,
                    recent_menu as usize,
                    w!("Recent"),
                );
                _ = AppendMenuW(file_menu, MF_SEPARATOR as u32, 0, w!(""));
                _ = AppendMenuW(file_menu, MF_STRING as u32, IDM_EXIT, w!("E&xit"));
                _ = AppendMenuW(bar, MF_POPUP as u32, file_menu as usize, w!("&File"));
                _ = SetMenu(hwnd, Some(bar));

                if let Some(state) = state_of(hwnd) {
                    state.recent_menu = recent_menu;
                    rebuild_recent_menu(state);
                }

                let entry = CreateWindowExW(
                    WS_EX_CLIENTEDGE as u32,
                    w!("EDIT"),
                    w!(""),
                    (WS_CHILD | WS_VISIBLE | ES_AUTOHSCROLL) as u32,
                    12,
                    14,
                    360,
                    26,
                    Some(hwnd),
                    Some(IDC_ENTRY as usize as HMENU),
                    Some(instance),
                    None,
                );
                debug_assert!(!entry.is_null());
                let add = CreateWindowExW(
                    0,
                    w!("BUTTON"),
                    w!("Add"),
                    (WS_CHILD | WS_VISIBLE | BS_DEFPUSHBUTTON) as u32,
                    382,
                    14,
                    80,
                    26,
                    Some(hwnd),
                    Some(IDC_ADD as usize as HMENU),
                    Some(instance),
                    None,
                );
                debug_assert!(!add.is_null());
                let list = CreateWindowExW(
                    WS_EX_CLIENTEDGE as u32,
                    w!("LISTBOX"),
                    w!(""),
                    (WS_CHILD | WS_VISIBLE | WS_VSCROLL | LBS_NOTIFY) as u32,
                    12,
                    52,
                    450,
                    320,
                    Some(hwnd),
                    Some(IDC_LIST as usize as HMENU),
                    Some(instance),
                    None,
                );
                debug_assert!(!list.is_null());

                // Startup load: the default todo.txt next to the exe
                // (missing file = empty list, no drama).
                open_into(hwnd, default_todo_path());
                _ = SetFocus(Some(entry));
                0
            }

            WM_COMMAND => {
                let notification = ((wparam >> 16) & 0xffff) as i32;
                let id = wparam & 0xffff;

                match id {
                    IDM_NEW => {
                        if let Some(state) = state_of(hwnd) {
                            state.items.clear();
                            state.path = None;
                        }
                        refill_list(hwnd);
                        0
                    }

                    IDM_OPEN => {
                        if let Some(pick) = pick_file(hwnd, false) {
                            open_into(hwnd, PathBuf::from(pick));
                        }
                        0
                    }

                    // Ctrl+S and the menu item land here identically.
                    IDM_SAVE => {
                        let has_path = state_of(hwnd).map(|s| s.path.is_some()).unwrap_or(false);
                        if !has_path {
                            // No file yet — degrade into Save As.
                            if let Some(pick) = pick_file(hwnd, true) {
                                if let Some(state) = state_of(hwnd) {
                                    state.path = Some(PathBuf::from(&pick));
                                }
                                if let Some(state) = state_of(hwnd) {
                                    remember_recent(state, PathBuf::from(pick));
                                }
                            }
                        }
                        save_current(hwnd);
                        refill_list(hwnd);
                        0
                    }

                    IDM_SAVE_AS => {
                        if let Some(pick) = pick_file(hwnd, true) {
                            if let Some(state) = state_of(hwnd) {
                                state.path = Some(PathBuf::from(&pick));
                            }
                            if let Some(state) = state_of(hwnd) {
                                remember_recent(state, PathBuf::from(pick));
                            }
                            save_current(hwnd);
                            refill_list(hwnd);
                        }
                        0
                    }

                    IDM_EXIT => {
                        // Same road as [X]: through WM_CLOSE, so the save
                        // below cannot be bypassed.
                        _ = PostMessageW(Some(hwnd), WM_CLOSE as u32, 0, 0);
                        0
                    }

                    // Pattern ranges can't hold expressions (STYLE.md's
                    // pattern rule) — a guard covers the recent-id span.
                    id2 if (IDM_RECENT_BASE..=IDM_RECENT_BASE + 2).contains(&id2) => {
                        // The shown index equals the state index (Recent
                        // entries and menu items were appended together).
                        let idx = id2 - IDM_RECENT_BASE;
                        let path = state_of(hwnd).and_then(|s| s.recent.get(idx).cloned());
                        if let Some(path) = path {
                            open_into(hwnd, path);
                        }
                        0
                    }

                    _ if id == IDC_ADD as usize && notification == BN_CLICKED => {
                        let entry = GetDlgItem(Some(hwnd), IDC_ENTRY);
                        let len = SendMessageW(entry, WM_GETTEXTLENGTH as u32, 0, 0) as usize;
                        let mut buf = vec![0u16; len + 1];
                        SendMessageW(
                            entry,
                            WM_GETTEXT as u32,
                            buf.len() as WPARAM,
                            buf.as_mut_ptr() as LPARAM,
                        );
                        _ = SetWindowTextW(entry, w!(""));
                        let text = wide_to_string(&buf).trim().to_string();
                        if !text.is_empty() {
                            if let Some(state) = state_of(hwnd) {
                                state.items.push((text, false));
                            }
                            refill_list(hwnd);
                        }
                        _ = SetFocus(Some(GetDlgItem(Some(hwnd), IDC_ENTRY)));
                        0
                    }

                    // Double-click toggles done (e81).
                    _ if id == IDC_LIST as usize && notification == LBN_DBLCLK => {
                        let list = GetDlgItem(Some(hwnd), IDC_LIST);
                        let sel = SendMessageW(list, LB_GETCURSEL as u32, 0, 0) as i32;
                        if sel >= 0 {
                            if let Some(state) = state_of(hwnd)
                                && (sel as usize) < state.items.len()
                            {
                                state.items[sel as usize].1 = !state.items[sel as usize].1;
                            }
                            refill_list(hwnd);
                        }
                        0
                    }

                    _ => DefWindowProcW(hwnd, message, wparam, lparam),
                }
            }

            WM_CLOSE => {
                // EVERY exit path funnels here (menu Exit posts WM_CLOSE,
                // the [X] box sends it) — so the save is unavoidable.
                save_current(hwnd);
                _ = DestroyWindow(hwnd);
                0
            }

            WM_NCDESTROY => {
                let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut State;
                if !ptr.is_null() {
                    SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
                    drop(Box::from_raw(ptr));
                }
                DefWindowProcW(hwnd, message, wparam, lparam)
            }

            WM_DESTROY => {
                PostQuitMessage(0);
                0
            }

            _ => DefWindowProcW(hwnd, message, wparam, lparam),
        }
    }
}
