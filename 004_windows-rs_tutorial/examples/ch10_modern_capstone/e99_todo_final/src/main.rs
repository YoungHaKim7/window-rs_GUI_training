//! e99 — Todo, Final: the whole tutorial in one window
//!
//! What you learn:
//! * Everything at once, in boringly plain code: listbox + entry + default
//!   button from ch05, menu + accelerators + tray popup menu from ch04,
//!   status bar parts from ch05, dark titlebar via DWM from the modern
//!   side, TaskDialog for About, and file persistence next to the exe —
//!   no pattern you haven't already met alone somewhere in e01–e98
//! * The tray icon lifecycle: `Shell_NotifyIconW(NIM_ADD)` at startup,
//!   a private `WM_APP + 1` callback message whose `lparam` carries the
//!   mouse event, a `TrackPopupMenu` on right-click (with the two classic
//!   dances: `SetForegroundWindow` before and a `WM_NULL` PostMessage
//!   after, or the menu refuses to dismiss when you click away), and
//!   `NIM_DELETE` in WM_DESTROY — leaving an icon behind is the classic
//!   tray-litter bug
//! * `DwmSetWindowAttribute(DWMWA_USE_IMMERSIVE_DARK_MODE)` — one call,
//!   one BOOL, and the title bar follows the system's dark mode rules
//!   without a single WM_PAINT
//! * COM-free "modern" dialogs: `TaskDialog` (comctl32) gives icons and
//!   typography for About-style boxes at the cost of one function call
//!
//! The feature set: add todos (Enter — the Add button is BS_DEFPUSHBUTTON
//! and the loop runs IsDialogMessageW), double-click toggles done, Del
//! deletes the selection, Ctrl+S saves (also saved on exit), Ctrl+N jumps
//! to the entry, View > Dark titlebar toggles, the tray icon's right-click
//! menu restores/exits. Todos live in `todo.txt` next to the executable
//! (under `cargo run` that is target/debug/todo.txt).
//!
//! New Win32 APIs: `Shell_NotifyIconW` + `NOTIFYICONDATAW` (shellapi);
//! `DwmSetWindowAttribute` + `DWMWA_USE_IMMERSIVE_DARK_MODE` (dwmapi);
//! `TaskDialog` + `TDCBF_OK_BUTTON` (commctrl); recap of
//! `CreateAcceleratorTableW`, `SB_SETPARTS`/`SB_SETTEXTW`, `TrackPopupMenu`.
//!
//! Run: `cargo run -p e99_todo_final`

use windows::{Win32::*, core::*};

// ---- ids -------------------------------------------------------------------
// Controls 1001+, menu items 2001+, tray-menu items 3001+ — the numbering
// families just keep WM_COMMAND dispatch readable.
const IDC_ENTRY: i32 = 1001;
const IDC_ADD: i32 = 1002;
const IDC_LIST: i32 = 1003;

const IDM_EXIT: u32 = 2001;
const IDM_FOCUS_ENTRY: u32 = 2002;
const IDM_DELETE: u32 = 2003;
const IDM_SAVE: u32 = 2004;
const IDM_DARK: u32 = 2005;
const IDM_ABOUT: u32 = 2006;

const IDM_TRAY_RESTORE: u32 = 3001;
const IDM_TRAY_EXIT: u32 = 3002;

const TRAY_ID: u32 = 1;
/// Tray callback message: the shell posts it to our hwnd with the mouse
/// message in lparam (classic, non-VERSION_4 notify icons).
const WM_APP_TRAY: i32 = WM_APP + 1;

#[derive(Default)]
struct State {
    todos: Vec<(String, bool)>, // (text, done)
    hwnd_entry: HWND,
    hwnd_add: HWND,
    hwnd_list: HWND,
    hwnd_status: HWND,
    dark: bool,
}

fn main() {
    unsafe {
        // Status bar = common control: register the family first (e50/e51).
        let icc = INITCOMMONCONTROLSEX {
            dwSize: std::mem::size_of::<INITCOMMONCONTROLSEX>() as u32,
            dwICC: ICC_BAR_CLASSES as u32,
        };
        _ = InitCommonControlsEx(&icc);

        let instance = GetModuleHandleW(PCWSTR::null());
        debug_assert!(!instance.is_null());

        let wc = WNDCLASSW {
            lpfnWndProc: Some(wndproc),
            hInstance: instance,
            hCursor: LoadCursorW(None, PCWSTR(IDC_ARROW.0.cast())),
            hbrBackground: ((COLOR_WINDOW + 1) as isize) as HBRUSH,
            lpszClassName: w!("e99_class"),
            style: (CS_HREDRAW | CS_VREDRAW) as u32,
            ..Default::default()
        };
        debug_assert!(RegisterClassW(&wc) != 0);

        // ---- Menu bar (e33's recipe: bar + popup drop-downs + SetMenu) ---
        let bar = CreateMenu();

        let file_menu = CreatePopupMenu();
        _ = AppendMenuW(file_menu, MF_STRING as u32, IDM_EXIT as usize, w!("E&xit"));

        let todo_menu = CreatePopupMenu();
        _ = AppendMenuW(
            todo_menu,
            MF_STRING as u32,
            IDM_FOCUS_ENTRY as usize,
            w!("Focus &entry\tCtrl+N"),
        );
        _ = AppendMenuW(
            todo_menu,
            MF_STRING as u32,
            IDM_DELETE as usize,
            w!("&Delete selected\tDel"),
        );
        _ = AppendMenuW(todo_menu, MF_SEPARATOR as u32, 0, w!(""));
        _ = AppendMenuW(
            todo_menu,
            MF_STRING as u32,
            IDM_SAVE as usize,
            w!("&Save\tCtrl+S"),
        );

        let view_menu = CreatePopupMenu();
        // Checkable item: state lives in the menu itself (e35); MF_CHECKED
        // is toggled in WM_COMMAND when the user picks it.
        _ = AppendMenuW(
            view_menu,
            MF_STRING as u32,
            IDM_DARK as usize,
            w!("&Dark titlebar"),
        );

        let help_menu = CreatePopupMenu();
        _ = AppendMenuW(
            help_menu,
            MF_STRING as u32,
            IDM_ABOUT as usize,
            w!("&About..."),
        );

        _ = AppendMenuW(bar, MF_POPUP as u32, file_menu as usize, w!("&File"));
        _ = AppendMenuW(bar, MF_POPUP as u32, todo_menu as usize, w!("&Todo"));
        _ = AppendMenuW(bar, MF_POPUP as u32, view_menu as usize, w!("&View"));
        _ = AppendMenuW(bar, MF_POPUP as u32, help_menu as usize, w!("&Help"));

        let hwnd = CreateWindowExW(
            0,
            w!("e99_class"),
            w!("e99 \u{b7} Todo, Final \u{2014} the tutorial in one window"),
            (WS_OVERLAPPEDWINDOW | WS_VISIBLE) as u32,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            560,
            420,
            None,
            Some(bar),
            Some(instance),
            None,
        );
        debug_assert!(!hwnd.is_null());

        let state = Box::into_raw(Box::new(State {
            todos: load_todos(),
            ..State::default()
        }));
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, state as isize);

        // ---- Child controls ----------------------------------------------
        // Single-line EDIT. WS_TABSTOP matters: IsDialogMessageW (below in
        // the loop) gives the window dialog-style Tab navigation for free.
        let entry = CreateWindowExW(
            WS_EX_CLIENTEDGE as u32,
            w!("EDIT"),
            w!(""),
            (WS_CHILD | WS_VISIBLE | WS_TABSTOP | ES_AUTOHSCROLL) as u32,
            10,
            10,
            380,
            28,
            Some(hwnd),
            Some(IDC_ENTRY as HMENU),
            Some(instance),
            None,
        );
        debug_assert!(!entry.is_null());

        // BS_DEFPUSHBUTTON: with IsDialogMessageW in the loop, pressing
        // Enter anywhere in the window clicks THIS button.
        let add = CreateWindowExW(
            0,
            w!("BUTTON"),
            w!("&Add"),
            (WS_CHILD | WS_VISIBLE | WS_TABSTOP | BS_DEFPUSHBUTTON) as u32,
            400,
            10,
            130,
            28,
            Some(hwnd),
            Some(IDC_ADD as HMENU),
            Some(instance),
            None,
        );
        debug_assert!(!add.is_null());

        let list = CreateWindowExW(
            WS_EX_CLIENTEDGE as u32,
            w!("LISTBOX"),
            w!(""),
            (WS_CHILD | WS_VISIBLE | LBS_NOTIFY | LBS_NOINTEGRALHEIGHT | WS_VSCROLL) as u32,
            10,
            48,
            520,
            300,
            Some(hwnd),
            Some(IDC_LIST as HMENU),
            Some(instance),
            None,
        );
        debug_assert!(!list.is_null());

        let status = CreateWindowExW(
            0,
            w!("msctls_statusbar32"),
            w!(""),
            (WS_CHILD | WS_VISIBLE) as u32,
            0,
            0,
            0,
            0, // the control sizes itself
            Some(hwnd),
            None,
            Some(instance),
            None,
        );
        debug_assert!(!status.is_null());

        if let Some(s) = state_of(hwnd) {
            s.hwnd_entry = entry;
            s.hwnd_add = add;
            s.hwnd_list = list;
            s.hwnd_status = status;
            refresh_list(s);
            refresh_status(s);
        }
        layout(hwnd); // initial geometry, and again on every WM_SIZE

        // ---- Accelerators (e37) ------------------------------------------
        let mut accels = [
            ACCEL {
                fVirt: (FVIRTKEY | FCONTROL) as u8,
                key: 'N' as u16,
                cmd: IDM_FOCUS_ENTRY as u16,
            },
            ACCEL {
                fVirt: FVIRTKEY as u8,
                key: VK_DELETE as u16,
                cmd: IDM_DELETE as u16,
            },
            ACCEL {
                fVirt: (FVIRTKEY | FCONTROL) as u8,
                key: 'S' as u16,
                cmd: IDM_SAVE as u16,
            },
        ];
        let haccel = CreateAcceleratorTableW(accels.as_mut_ptr(), accels.len() as i32);
        debug_assert!(!haccel.is_null());

        // ---- Tray icon ----------------------------------------------------
        tray_add(hwnd);

        // ---- Message loop -------------------------------------------------
        let mut message = MSG::default();
        while GetMessageW(&mut message, None, 0, 0).0 > 0 {
            // Order is fixed (e37): accelerators first, then dialog keys,
            // then the normal path. TranslateAcceleratorW re-posts matched
            // keys as WM_COMMAND; IsDialogMessageW handles Tab/Enter.
            if TranslateAcceleratorW(hwnd, haccel, &mut message) != 0 {
                continue;
            }
            if IsDialogMessageW(hwnd, &mut message).as_bool() {
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

// ---- persistence -----------------------------------------------------------
// Format: one todo per line, "0\ttext" / "1\ttext". A tab separator because
// todo text may contain spaces, commas, anything — a tab can't be typed
// into a single-line edit control.

fn todos_path() -> std::path::PathBuf {
    // Next to the exe — under `cargo run` that's target/debug/todo.txt.
    std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(std::path::Path::to_path_buf))
        .unwrap_or_default()
        .join("todo.txt")
}

fn load_todos() -> Vec<(String, bool)> {
    std::fs::read_to_string(todos_path())
        .unwrap_or_default()
        .lines()
        .filter_map(|line| {
            let (done, text) = line.split_once('\t')?;
            Some((text.to_string(), done == "1"))
        })
        .collect()
}

fn save_todos(todos: &[(String, bool)]) {
    let body: String = todos
        .iter()
        .map(|(text, done)| format!("{}\t{}\n", if *done { 1 } else { 0 }, text))
        .collect();
    _ = std::fs::write(todos_path(), body);
}

// ---- view refresh ----------------------------------------------------------

/// Rebuild the listbox from state. "[x]" beats a checkbox glyph here: the
/// default listbox font renders it identically everywhere.
unsafe fn refresh_list(state: &mut State) {
    unsafe {
        SendMessageW(state.hwnd_list, LB_RESETCONTENT as u32, 0, 0);
        for (text, done) in &state.todos {
            let row = format!("{} {}", if *done { "[x]" } else { "[ ]" }, text);
            let wide: Vec<u16> = row.encode_utf16().chain(std::iter::once(0)).collect();
            SendMessageW(
                state.hwnd_list,
                LB_ADDSTRING as u32,
                0,
                wide.as_ptr() as isize,
            );
        }
    }
}

/// Update the two status bar parts. SB_SETPARTS takes right-edge pixels
/// per part with -1 meaning "extend to the end"; text then goes to part i
/// by ORing the index into SB_SETTEXTW.
unsafe fn refresh_status(state: &mut State) {
    unsafe {
        let mut rect = RECT::default();
        _ = GetClientRect(state.hwnd_status, &mut rect);
        let widths = [rect.right - 240i32, -1];
        SendMessageW(
            state.hwnd_status,
            SB_SETPARTS as u32,
            2,
            widths.as_ptr() as isize,
        );

        let done = state.todos.iter().filter(|(_, d)| *d).count();
        let open = state.todos.len() - done;
        let part0 = format!(
            "{} total \u{b7} {} done \u{b7} {} open",
            state.todos.len(),
            done,
            open
        );
        let wide: Vec<u16> = part0.encode_utf16().chain(std::iter::once(0)).collect();
        SendMessageW(
            state.hwnd_status,
            (SB_SETTEXTW | 0) as u32,
            0,
            wide.as_ptr() as isize,
        );

        let part1 = "saved to todo.txt next to the exe (Ctrl+S)";
        let wide: Vec<u16> = part1.encode_utf16().chain(std::iter::once(0)).collect();
        SendMessageW(
            state.hwnd_status,
            (SB_SETTEXTW | 1) as u32,
            0,
            wide.as_ptr() as isize,
        );
    }
}

/// Reposition children on resize. The status bar resizes itself when it
/// receives WM_SIZE — one SendMessageW and no geometry of our own.
unsafe fn layout(hwnd: HWND) {
    unsafe {
        let Some(state) = state_of(hwnd) else {
            return;
        };
        let mut rect = RECT::default();
        _ = GetClientRect(hwnd, &mut rect);
        let w = rect.right - rect.left;

        // How tall is the status bar? Ask the control itself.
        let mut sb = RECT::default();
        _ = GetWindowRect(state.hwnd_status, &mut sb);
        let sb_h = sb.bottom - sb.top;

        _ = MoveWindow(state.hwnd_entry, 10, 10, w - 160, 28, true);
        _ = MoveWindow(state.hwnd_add, w - 142, 10, 130, 28, true);
        _ = MoveWindow(
            state.hwnd_list,
            10,
            48,
            w - 20,
            rect.bottom - sb_h - 58,
            true,
        );
        SendMessageW(state.hwnd_status, WM_SIZE as u32, 0, 0);
        refresh_status(state);
    }
}

// ---- dark titlebar ---------------------------------------------------------

/// DWMWA_USE_IMMERSIVE_DARK_MODE is attribute 20 (19 on pre-2004 builds —
/// Windows accepts either). One BOOL in, paint-free theming out; only the
/// non-client area changes, the client stays COLOR_WINDOW.
unsafe fn apply_dark_titlebar(hwnd: HWND, dark: bool) {
    unsafe {
        let value: i32 = if dark { 1 } else { 0 };
        _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_USE_IMMERSIVE_DARK_MODE as u32,
            &value as *const i32 as LPCVOID,
            std::mem::size_of::<i32>() as u32,
        );
    }
}

// ---- tray ------------------------------------------------------------------

unsafe fn tray_add(hwnd: HWND) {
    unsafe {
        let mut nid = NOTIFYICONDATAW::default();
        nid.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
        nid.hWnd = hwnd;
        nid.uID = TRAY_ID;
        nid.uFlags = (NIF_MESSAGE | NIF_ICON | NIF_TIP) as u32;
        nid.uCallbackMessage = WM_APP_TRAY as u32; // posted on mouse activity
        nid.hIcon = LoadIconW(None, PCWSTR(IDI_APPLICATION.0.cast()));
        // Fixed [WCHAR;128] buffer (e91's PROCESSENTRY32W rule): write the
        // UTF-16 ourselves, keep the terminating NUL, never overrun.
        for (i, unit) in "e99 todo".encode_utf16().take(127).enumerate() {
            nid.szTip[i] = unit;
        }
        _ = Shell_NotifyIconW(NIM_ADD as u32, &mut nid);
    }
}

unsafe fn tray_delete(hwnd: HWND) {
    unsafe {
        let mut nid = NOTIFYICONDATAW::default();
        nid.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
        nid.hWnd = hwnd;
        nid.uID = TRAY_ID;
        _ = Shell_NotifyIconW(NIM_DELETE as u32, &mut nid);
    }
}

unsafe fn show_tray_menu(hwnd: HWND) {
    unsafe {
        let menu = CreatePopupMenu();
        _ = AppendMenuW(
            menu,
            MF_STRING as u32,
            IDM_TRAY_RESTORE as usize,
            w!("&Restore"),
        );
        _ = AppendMenuW(menu, MF_STRING as u32, IDM_TRAY_EXIT as usize, w!("E&xit"));

        // The two-step dismissal dance (documented since Windows 95):
        // SetForegroundWindow BEFORE TrackPopupMenu, else the menu won't
        // close when the user clicks elsewhere...
        _ = SetForegroundWindow(hwnd);

        let mut pt = POINT::default();
        _ = GetCursorPos(&mut pt);
        _ = TrackPopupMenu(menu, TPM_RIGHTBUTTON as u32, pt.x, pt.y, None, hwnd, None);
        // ...and a WM_NULL right after, to force the taskbar thread to
        // notice the foreground change (KB Q135788, still true today).
        _ = PostMessageW(Some(hwnd), WM_NULL as u32, 0, 0);
        _ = DestroyMenu(menu);
    }
}

// ---- actions ---------------------------------------------------------------

unsafe fn add_todo(state: &mut State) {
    unsafe {
        let len = GetWindowTextLengthW(state.hwnd_entry);
        if len <= 0 {
            return; // nothing typed: ignore quietly
        }
        let mut buf = vec![0u16; len as usize + 1];
        let copied = GetWindowTextW(
            state.hwnd_entry,
            PWSTR::from_raw(buf.as_mut_ptr()),
            buf.len() as i32,
        );
        buf.truncate(copied as usize);
        state.todos.push((String::from_utf16_lossy(&buf), false));

        _ = SetWindowTextW(state.hwnd_entry, w!(""));
        refresh_list(state);
        refresh_status(state);
        _ = SetFocus(Some(state.hwnd_entry)); // ready for the next one
    }
}

unsafe fn toggle_selected(state: &mut State) {
    unsafe {
        let sel = SendMessageW(state.hwnd_list, LB_GETCURSEL as u32, 0, 0) as i32;
        if sel != LB_ERR && (sel as usize) < state.todos.len() {
            state.todos[sel as usize].1 = !state.todos[sel as usize].1;
            refresh_list(state);
            // Keep the selection on the toggled row (it just moved content).
            SendMessageW(state.hwnd_list, LB_SETCURSEL as u32, sel as usize, 0);
            refresh_status(state);
        }
    }
}

unsafe fn delete_selected(state: &mut State) {
    unsafe {
        let sel = SendMessageW(state.hwnd_list, LB_GETCURSEL as u32, 0, 0) as i32;
        if sel != LB_ERR && (sel as usize) < state.todos.len() {
            state.todos.remove(sel as usize);
            refresh_list(state);
            refresh_status(state);
        }
    }
}

// ---- wndproc ---------------------------------------------------------------

extern "system" fn wndproc(hwnd: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe {
        let Some(state) = state_of(hwnd) else {
            return DefWindowProcW(hwnd, message, wparam, lparam);
        };

        match message as i32 {
            WM_COMMAND => {
                let id = (wparam & 0xffff) as u16 as i32;
                let code = ((wparam >> 16) & 0xffff) as u16 as i32;

                // Controls first (they carry a notification code)...
                match (id, code) {
                    (IDC_ADD, BN_CLICKED) => {
                        add_todo(state);
                        return 0;
                    }
                    (IDC_LIST, LBN_DBLCLK) => {
                        toggle_selected(state);
                        return 0;
                    }
                    _ => {}
                }

                // ...then menu items (code == 0) and accelerators (which
                // arrive here AS menu commands — that's the e37 trick).
                match id as u32 {
                    IDM_EXIT | IDM_TRAY_EXIT => {
                        _ = DestroyWindow(hwnd);
                        0
                    }
                    IDM_TRAY_RESTORE => {
                        // Restore from a tray click: un-minimize, come up.
                        if IsIconic(hwnd).as_bool() {
                            _ = ShowWindow(hwnd, SW_RESTORE);
                        }
                        _ = SetForegroundWindow(hwnd);
                        0
                    }
                    IDM_FOCUS_ENTRY => {
                        _ = SetFocus(Some(state.hwnd_entry));
                        0
                    }
                    IDM_DELETE => {
                        delete_selected(state);
                        0
                    }
                    IDM_SAVE => {
                        save_todos(&state.todos);
                        println!(
                            "saved {} todos to {}",
                            state.todos.len(),
                            todos_path().display()
                        );
                        0
                    }
                    IDM_DARK => {
                        state.dark = !state.dark;
                        apply_dark_titlebar(hwnd, state.dark);
                        // The menu is the state holder (e35): check/uncheck.
                        let menu = GetMenu(hwnd);
                        _ = CheckMenuItem(
                            menu,
                            IDM_DARK,
                            (MF_BYCOMMAND | if state.dark { MF_CHECKED } else { MF_UNCHECKED })
                                as u32,
                        );
                        0
                    }
                    IDM_ABOUT => {
                        // TaskDialog: the comctl32 heir of MessageBoxW.
                        // Fixed text + icon, no custom dialog resource —
                        // the pragmatic About box since Vista.
                        _ = TaskDialog(
                            Some(hwnd),
                            None,
                            w!("About e99"),
                            w!("e99 \u{b7} Todo, Final"),
                            w!("The capstone of the windows-rs tutorial: controls, menus, \
                                accelerators, status bar, tray icon, DWM dark titlebar, \
                                TaskDialog and file persistence in one window."),
                            TDCBF_OK_BUTTON,
                            TD_INFORMATION_ICON,
                            None,
                        )
                        .ok();
                        0
                    }
                    _ => DefWindowProcW(hwnd, message, wparam, lparam),
                }
            }

            // ---- Tray mailbox: the shell posts WM_APP_TRAY with the mouse
            // message in lparam. (lparam, not wparam — tray quirk.)
            WM_APP_TRAY => {
                match lparam as i32 {
                    WM_LBUTTONUP => {
                        if IsIconic(hwnd).as_bool() {
                            _ = ShowWindow(hwnd, SW_RESTORE);
                        }
                        _ = SetForegroundWindow(hwnd);
                    }
                    WM_RBUTTONUP => show_tray_menu(hwnd),
                    _ => {}
                }
                0
            }

            WM_SIZE => {
                layout(hwnd);
                0
            }

            WM_DESTROY => {
                save_todos(&state.todos); // exit = save too (belt and braces)
                tray_delete(hwnd); // never litter the notification area
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
