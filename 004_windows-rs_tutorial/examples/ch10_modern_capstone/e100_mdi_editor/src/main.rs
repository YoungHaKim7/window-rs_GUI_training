//! e100 — MDI Editor: the multiple-document-interface finale
//!
//! What you learn:
//! * The MDI cast, three windows deep: a **frame** window (menu, owns
//!   everything), one **MDICLIENT** child — a magic window USER32 provides
//!   that manages the document area — and N **MDI child** windows (your
//!   document class), each hosting a multiline EDIT
//! * Why MDI has its OWN default processors: the frame's unhandled messages
//!   go to `DefFrameProcW` (NOT DefWindowProcW) and the children's to
//!   `DefMDIChildProcW` — those two implement the entire 1990s contract:
//!   Ctrl+Tab/F6 cycling, the Window menu's "1 doc one", cascade/tile,
//!   child maximize-into-the-frame's-title-bar, system-menu keystrokes...
//!   `DefFrameProcW` is also the one Win32 def-proc with an extra argument:
//!   it wants the **MDI client hwnd** as its second parameter, because
//!   that's how it reaches the children it manages
//! * Creating a document is a MESSAGE, not CreateWindowExW: you send
//!   `WM_MDICREATE` to the client with a `MDICREATESTRUCTW` in lparam, and
//!   the client creates the child for you (it has to — the client tracks
//!   every child). `WM_MDIDESTROY`, `WM_MDITILE`, `WM_MDICASCADE`,
//!   `WM_MDIGETACTIVE` are the rest of the mini-API
//! * The `CLIENTCREATESTRUCT` handed to the MDICLIENT via CreateWindowExW's
//!   lpparam: it binds OUR "Window" submenu into the system — USER32
//!   appends the document list ("&1 Untitled", "&2 Untitled", ...) to it
//!   automatically, with command ids starting at `idFirstChild`. This is
//!   the same "pass a struct through lpparam" move as e65's state pointer
//!
//! Click File > New a few times, then Window > Tile / Cascade / Close All.
//! The state pointer rides CreateWindowExW's lpparam and attaches at
//! WM_NCCREATE — the e65 deep-state pattern, because here WM_CREATE
//! genuinely needs it.
//!
//! New Win32 APIs: `MDICLIENT` + `CLIENTCREATESTRUCT`, `WM_MDICREATE` +
//! `MDICREATESTRUCTW`, `WM_MDIDESTROY`/`WM_MDITILE`/`WM_MDICASCADE`/
//! `WM_MDIGETACTIVE`, `DefFrameProcW`, `DefMDIChildProcW`, `GetSubMenu`
//! (winuser).
//!
//! Run: `cargo run -p e100_mdi_editor`

use windows::{Win32::*, core::*};

// ---- ids -------------------------------------------------------------------
const IDM_NEW: usize = 4001;
const IDM_EXIT: usize = 4002;
const IDM_TILE: usize = 4003;
const IDM_CASCADE: usize = 4004;
const IDM_CLOSEALL: usize = 4005;

/// Id base for the document entries USER32 appends to our Window submenu.
const IDM_FIRSTCHILD: u32 = 5000;

/// Child-control ids inside the two window kinds.
const IDC_MDICLIENT: i32 = 6001; // the client window (an HMENU id)
const IDC_DOC_EDIT: i32 = 6002; // the EDIT inside each document

#[derive(Default)]
struct State {
    hwnd_client: HWND, // the MDICLIENT — set in WM_CREATE, used everywhere
    docs: u32,         // just for "Untitled N" titles
}

fn main() {
    unsafe {
        let instance = GetModuleHandleW(PCWSTR::null());
        debug_assert!(!instance.is_null());

        // ---- Class 1: the frame ----------------------------------------
        // COLOR_APPWORKSPACE is the traditional MDI desktop color — what
        // shows between cascaded documents.
        let wc_frame = WNDCLASSW {
            lpfnWndProc: Some(frame_wndproc),
            hInstance: instance,
            hCursor: LoadCursorW(None, PCWSTR(IDC_ARROW.0.cast())),
            hbrBackground: ((COLOR_APPWORKSPACE + 1) as isize) as HBRUSH,
            lpszClassName: w!("e100_frame"),
            ..Default::default() // no CS_HREDRAW/CS_VREDRAW: the client
                                 // manages its own repaints; frame flicker stays away
        };
        debug_assert!(RegisterClassW(&wc_frame) != 0);

        // ---- Class 2: the document (MDI child) ---------------------------
        let wc_doc = WNDCLASSW {
            lpfnWndProc: Some(doc_wndproc),
            hInstance: instance,
            hCursor: LoadCursorW(None, PCWSTR(IDC_ARROW.0.cast())),
            hbrBackground: ((COLOR_WINDOW + 1) as isize) as HBRUSH,
            lpszClassName: w!("e100_doc"),
            ..Default::default()
        };
        debug_assert!(RegisterClassW(&wc_doc) != 0);

        // ---- Menu: File + Window ------------------------------------------
        let bar = CreateMenu();

        let file_menu = CreatePopupMenu();
        _ = AppendMenuW(file_menu, MF_STRING as u32, IDM_NEW, w!("&New"));
        _ = AppendMenuW(file_menu, MF_SEPARATOR as u32, 0, w!(""));
        _ = AppendMenuW(file_menu, MF_STRING as u32, IDM_EXIT, w!("E&xit"));

        // The Window menu is not ours alone: the CLIENTCREATESTRUCT below
        // hands this submenu to USER32, and IT appends the live document
        // list (ids IDM_FIRSTCHILD + n) — position matters, we fish it out
        // by index in WM_CREATE.
        let window_menu = CreatePopupMenu();
        _ = AppendMenuW(
            window_menu,
            MF_STRING as u32,
            IDM_TILE,
            w!("Tile &Vertical"),
        );
        _ = AppendMenuW(window_menu, MF_STRING as u32, IDM_CASCADE, w!("&Cascade"));
        _ = AppendMenuW(window_menu, MF_SEPARATOR as u32, 0, w!(""));
        _ = AppendMenuW(
            window_menu,
            MF_STRING as u32,
            IDM_CLOSEALL,
            w!("C&lose All"),
        );

        _ = AppendMenuW(bar, MF_POPUP as u32, file_menu as usize, w!("&File"));
        _ = AppendMenuW(bar, MF_POPUP as u32, window_menu as usize, w!("&Window"));

        // ---- Frame window, state via lpparam (the e65 pattern) ------------
        let state = Box::into_raw(Box::<State>::default());
        let hwnd = CreateWindowExW(
            0,
            w!("e100_frame"),
            w!("e100 \u{b7} MDI Editor \u{2014} File > New to open documents"),
            (WS_OVERLAPPEDWINDOW | WS_VISIBLE) as u32,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            800,
            560,
            None,
            Some(bar),
            Some(instance),
            Some(state as LPVOID), // rides into WM_NCCREATE
        );
        debug_assert!(!hwnd.is_null());
        if hwnd.is_null() {
            drop(Box::from_raw(state)); // creation failed: reclaim (e65)
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

// ---- the frame -------------------------------------------------------------

extern "system" fn frame_wndproc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    unsafe {
        match message as i32 {
            // e65's deep-state attach: WM_NCCREATE is the FIRST message,
            // so state exists by the time WM_CREATE runs below.
            WM_NCCREATE => {
                let cs = lparam as *const CREATESTRUCTW;
                SetWindowLongPtrW(hwnd, GWLP_USERDATA, (*cs).lpCreateParams as isize);
                DefWindowProcW(hwnd, message, wparam, lparam)
            }

            WM_CREATE => {
                let Some(state) = state_of(hwnd) else {
                    return -1; // refuse creation without state
                };
                let cs = lparam as *const CREATESTRUCTW;

                // Bind our Window submenu: index 1 in the bar (File is 0).
                // hWindowMenu is a plain HANDLE — a raw-pointer alias, cast.
                let window_menu = GetSubMenu(GetMenu(hwnd), 1);
                let ccs = CLIENTCREATESTRUCT {
                    hWindowMenu: window_menu as HANDLE,
                    idFirstChild: IDM_FIRSTCHILD, // ids for "1 doc", "2 doc"...
                };

                // THE MDI client window. Class "MDICLIENT" is implemented
                // inside USER32; WS_CLIPCHILDREN stops documents overpainting
                // each other; the CLIENTCREATESTRUCT rides lpparam verbatim.
                // Window text must be NULL for this class.
                let client = CreateWindowExW(
                    0,
                    w!("MDICLIENT"),
                    PCWSTR::null(),
                    (WS_CHILD | WS_VISIBLE | WS_CLIPCHILDREN | WS_HSCROLL | WS_VSCROLL) as u32,
                    0,
                    0,
                    0,
                    0,
                    Some(hwnd),
                    Some(IDC_MDICLIENT as HMENU),
                    Some((*cs).hInstance),
                    Some(&ccs as *const CLIENTCREATESTRUCT as LPVOID),
                );
                debug_assert!(!client.is_null());
                state.hwnd_client = client;

                DefWindowProcW(hwnd, message, wparam, lparam)
            }

            WM_COMMAND => {
                let Some(state) = state_of(hwnd) else {
                    return 0;
                };
                match wparam & 0xffff {
                    IDM_NEW => {
                        // A document is born by MESSAGE, not CreateWindowExW:
                        // WM_MDICREATE to the client, MDICREATESTRUCTW in
                        // lparam. The client creates the "e100_doc" child,
                        // tracks it, and lists it in the Window menu.
                        state.docs += 1;
                        let title = HSTRING::from(format!("Untitled {}", state.docs));
                        let mdics = MDICREATESTRUCTW {
                            szClass: w!("e100_doc"),
                            // PCWSTR borrowed from the HSTRING above (kept
                            // alive by the local until SendMessageW returns).
                            szTitle: PCWSTR::from_raw(title.as_ptr()),
                            hOwner: GetModuleHandleW(PCWSTR::null()) as HANDLE,
                            x: CW_USEDEFAULT,
                            y: CW_USEDEFAULT,
                            cx: CW_USEDEFAULT,
                            cy: CW_USEDEFAULT,
                            style: 0, // MDI defaults (WS_CHILD etc.) are added by the client
                            lParam: 0,
                        };
                        _ = SendMessageW(
                            state.hwnd_client,
                            WM_MDICREATE as u32,
                            0,
                            &mdics as *const MDICREATESTRUCTW as isize,
                        );
                        0
                    }
                    IDM_TILE => {
                        _ = SendMessageW(
                            state.hwnd_client,
                            WM_MDITILE as u32,
                            MDITILE_VERTICAL as usize,
                            0,
                        );
                        0
                    }
                    IDM_CASCADE => {
                        _ = SendMessageW(state.hwnd_client, WM_MDICASCADE as u32, 0, 0);
                        0
                    }
                    IDM_CLOSEALL => {
                        // Close All = keep asking "who is active?" and
                        // destroying that one until the answer is null.
                        // (EnumChildWindows would also work; every child of
                        // the client IS a document — but this loop is the
                        // classic Petzold idiom.)
                        loop {
                            let active =
                                SendMessageW(state.hwnd_client, WM_MDIGETACTIVE as u32, 0, 0)
                                    as HWND;
                            if active.is_null() {
                                break;
                            }
                            _ = SendMessageW(
                                state.hwnd_client,
                                WM_MDIDESTROY as u32,
                                active as usize,
                                0,
                            );
                        }
                        0
                    }
                    IDM_EXIT => {
                        _ = DestroyWindow(hwnd);
                        0
                    }
                    // IDM_FIRSTCHILD+n (the appended doc entries) need no
                    // code at all: DefFrameProcW implements them.
                    _ => DefFrameProcW(hwnd, Some(state.hwnd_client), message, wparam, lparam),
                }
            }

            WM_SIZE => {
                // Keep the client glued to the frame's client area (width
                // and height arrive packed in lparam's two words); the
                // client then arranges the documents itself. DefFrameProcW
                // still gets its turn afterwards — it needs WM_SIZE too.
                let client = state_of(hwnd).map(|s| {
                    _ = MoveWindow(
                        s.hwnd_client,
                        0,
                        0,
                        (lparam & 0xffff) as u16 as i32,
                        ((lparam >> 16) & 0xffff) as u16 as i32,
                        true,
                    );
                    s.hwnd_client
                });
                DefFrameProcW(hwnd, client, message, wparam, lparam)
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

            // *** The MDI rule: EVERY unhandled frame message goes to
            // DefFrameProcW, not DefWindowProcW — with the MDI client hwnd
            // as the second argument, so it can reach the documents. It
            // implements child cycling, the Window menu, maximize-in-place,
            // and more; calling DefWindowProcW instead quietly breaks MDI.
            _ => {
                let client = state_of(hwnd).map(|s| s.hwnd_client);
                DefFrameProcW(hwnd, client, message, wparam, lparam)
            }
        }
    }
}

// ---- one document ----------------------------------------------------------

extern "system" fn doc_wndproc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    unsafe {
        match message as i32 {
            WM_CREATE => {
                // Each document is a thin shell around one big EDIT.
                // lparam reuses the WM_MDICREATESTRUCTW (in its lParam
                // field) — we only need the HINSTANCE back out.
                let cs = lparam as *const CREATESTRUCTW;
                let edit = CreateWindowExW(
                    WS_EX_CLIENTEDGE as u32,
                    w!("EDIT"),
                    w!(""),
                    (WS_CHILD
                        | WS_VISIBLE
                        | WS_VSCROLL
                        | WS_HSCROLL
                        | ES_MULTILINE
                        | ES_AUTOVSCROLL
                        | ES_AUTOHSCROLL) as u32,
                    0,
                    0,
                    0,
                    0,
                    Some(hwnd),
                    Some(IDC_DOC_EDIT as HMENU),
                    Some((*cs).hInstance),
                    None,
                );
                debug_assert!(!edit.is_null());
                DefMDIChildProcW(hwnd, message, wparam, lparam)
            }

            WM_SIZE => {
                // Fit the EDIT to the document. GetDlgItem saves us a
                // state block for one pointer — fine for a tutorial.
                let edit = GetDlgItem(Some(hwnd), IDC_DOC_EDIT);
                if !edit.is_null() {
                    _ = MoveWindow(
                        edit,
                        0,
                        0,
                        (lparam & 0xffff) as u16 as i32,
                        ((lparam >> 16) & 0xffff) as u16 as i32,
                        true,
                    );
                }
                DefMDIChildProcW(hwnd, message, wparam, lparam)
            }

            // Children NEVER call PostQuitMessage (only the frame does) and
            // NEVER DefWindowProcW — DefMDIChildProcW keeps the client's
            // bookkeeping straight (activation, Window menu, maximized
            // geometry) while the document closes or maximizes.
            _ => DefMDIChildProcW(hwnd, message, wparam, lparam),
        }
    }
}
