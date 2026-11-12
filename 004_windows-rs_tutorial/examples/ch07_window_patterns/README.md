# Chapter 7 — Window Patterns

Beyond one plain window: this chapter is about the structural techniques —
how state really lives on a window, how apps own many windows, how to build
your own controls, subclass other people's, and shape windows in ways the
defaults don't offer (transparent, non-rectangular, dark, tray-dwelling).

| # | Example | What you learn |
|---|---------|----------------|
| e65 | `e65_window_state` | The deep state pattern: attach in `WM_NCCREATE` via `lpparam`, free in `WM_NCDESTROY`, the re-entrancy rules |
| e66 | `e66_multiple_windows` | Two classes, two wndprocs; **owned vs. child windows** settled; a live sticky-notes app |
| e67 | `e67_custom_control` | Build a reusable child control (own class + wndproc) that talks to its parent like BUTTON does |
| e68 | `e68_subclassing` | `SetWindowSubclass`: an EDIT that only accepts hex digits; why `ES_NUMBER` isn't enough |
| e69 | `e69_owner_draw_button` | `BS_OWNERDRAW` + `WM_DRAWITEM`: painting the button yourself, pressed state and all |
| e70 | `e70_splitter_window` | A draggable divider between two panes — layout ownership, mouse capture on a child |
| e71 | `e71_tray_icon` | `Shell_NotifyIconW`: notification-area icon, callback messages, balloon tips, cleanup |
| e72 | `e72_layered_window` | `WS_EX_LAYERED`: per-pixel alpha with a slider, and the color-key trick |
| e73 | `e73_shaped_window` | `SetWindowRgn`: round and star-shaped windows (and why regions don't antialias) |
| e74 | `e74_dark_titlebar` | `DwmSetWindowAttribute`: dark mode, caption and text colors on the DWM-owned frame |
| e75 | `e75_drag_drop_files` | `DragAcceptFiles` + `WM_DROPFILES`: files dragged in from Explorer |
| e76 | `e76_message_only_window` | `HWND_MESSAGE`: an invisible window that exists purely to receive messages |

**Suggested order:** e65 first — the rest of the chapter assumes its
attach-in-`WM_NCCREATE` flavor of the state pattern.
