# windows-rs Tutorial — 100 Examples

A progressive, build-verified tutorial of **100 self-contained GUI examples**
written against the [`windows`](https://crates.io/crates/windows) crate —
Rust bindings for the raw Win32 API — starting from "Hello, World!" in a
message box and ending with complete applications (a notepad, a calculator,
todo apps, minesweeper, an MDI editor) and modern touches (Direct2D,
WebView2, dark title bars, tray icons).

Every example is a standalone crate: no framework, no wrapper library, no
`.rc` files — just you, Rust, and 30-odd years of Win32, with the
`RegisterClassW` boilerplate repeated on purpose until it becomes muscle
memory.

## Prerequisites

- Windows 10 or 11
- Rust **stable ≥ 1.95** (rustup: `rustup default stable`) — the vendored
  `windows` crate uses edition 2024
- No network needed: all dependencies are **path dependencies** into the
  vendored windows-rs clone in [`windows-rs/`](windows-rs/) (master branch,
  `windows` 0.100-dev)

## Running an example

From this directory (the cargo workspace root):

```
cargo run -p e03_hello_window
cargo run -p e77_notepad
cargo run -p e99_todo_final
```

Build everything at once (first build compiles the vendored `windows` crate
— expect a few minutes, cached afterwards):

```
cargo build --workspace
```

## The curriculum

| Chapter | Theme | Examples |
|---------|-------|----------|
| [`ch01_first_window`](examples/ch01_first_window/) | First steps: message box → a real window → painting text → styles | e01–e10 |
| [`ch02_messages_input`](examples/ch02_messages_input/) | The message loop; mouse, keyboard, timers, hotkeys; **the window-state pattern** | e11–e20 |
| [`ch03_gdi_drawing`](examples/ch03_gdi_drawing/) | GDI: pens, brushes, shapes, double buffering, scrolling, DPI, saving bitmaps | e21–e32 |
| [`ch04_menus`](examples/ch04_menus/) | Menu bars, context menus, accelerators, the system menu, runtime icons | e33–e40 |
| [`ch05_controls`](examples/ch05_controls/) | Buttons, edits, checkboxes, radios, combos, listboxes, progress bars, toolbars | e41–e54 |
| [`ch06_dialogs_files`](examples/ch06_dialogs_files/) | Modal & modeless dialogs (in-memory templates), task dialogs, open/save/color/font pickers | e55–e64 |
| [`ch07_window_patterns`](examples/ch07_window_patterns/) | State deep-dive, multiple windows, custom controls, subclassing, splitters, tray, layered/shaped/dark windows | e65–e76 |
| [`ch08_mini_apps`](examples/ch08_mini_apps/) | Notepad, calculators, todo apps, paint, snake, minesweeper, analog clock, text viewer, unit converter | e77–e88 |
| [`ch09_system`](examples/ch09_system/) | Clipboard, screen capture, processes, system info, registry, worker threads, EnumWindows | e89–e95 |
| [`ch10_modern_capstone`](examples/ch10_modern_capstone/) | Direct2D, WebView2, building your own window API, the capstone todo, an MDI editor | e96–e100 |

Each chapter directory has its own `README.md` with a per-example index.

## Full example index

| # | Example | What it teaches |
|---|---------|-----------------|
| e01 | `e01_message_box` | Smallest Windows program; `w!()` literals; why Win32 calls are `unsafe` |
| e02 | `e02_message_box_buttons` | Button/icon combos, default buttons, reading the answer |
| e03 | `e03_hello_window` | **The canonical window**: class → create → message loop → wndproc |
| e04 | `e04_hello_paint` | `WM_PAINT`, `BeginPaint`/`EndPaint`, `DrawTextW` |
| e05 | `e05_window_anatomy` | Every `WNDCLASSW`/`CreateWindowExW` field annotated; `ShowWindow` |
| e06 | `e06_unicode_strings` | `w!`, `Vec<u16>`, `HSTRING`; reading strings back |
| e07 | `e07_fonts_and_text` | `CreateFontW`, `SelectObject`, colors, rotation; GDI object lifetime |
| e08 | `e08_client_rect` | `AdjustWindowRect` — exact client size |
| e09 | `e09_center_window` | `GetSystemMetrics`, centering, `WM_MOVE` |
| e10 | `e10_window_styles` | Live WS_/WS_EX_ tour; `SetWindowLongPtrW` + `SWP_FRAMECHANGED` |
| e11 | `e11_message_loop` | Watching messages flow; the `PeekMessageW` alternative |
| e12 | `e12_mouse_clicks` | Mouse messages; packed coordinates; `GetDC` vs `WM_PAINT` |
| e13 | `e13_mouse_move` | **The window-state pattern**; mouse capture; rubber-band model |
| e14 | `e14_keyboard_input` | `WM_KEYDOWN` vs `WM_CHAR` vs `WM_SYSKEYDOWN` |
| e15 | `e15_keyboard_focus` | Focus, the caret, a text box by hand |
| e16 | `e16_timer_clock` | `SetTimer`/`WM_TIMER` heartbeat pattern |
| e17 | `e17_hit_testing` | `WM_NCHITTEST` — draggable frameless `WS_POPUP` window |
| e18 | `e18_shift_state` | `GetKeyState` vs `GetAsyncKeyState`; toggles; Shift+click |
| e19 | `e19_global_hotkey` | `RegisterHotKey` system-wide |
| e20 | `e20_cursors` | `WM_SETCURSOR`; per-zone cursors |
| e21 | `e21_pixels_lines` | `SetPixelV`, `MoveToEx`/`LineTo` |
| e22 | `e22_pens` | `CreatePen` style/width/color gallery |
| e23 | `e23_brushes` | Solid + hatch brushes; system vs owned brushes |
| e24 | `e24_shapes` | Rectangle/Ellipse/RoundRect/Pie/Chord/Arc/Polygon |
| e25 | `e25_drawing_modes` | `SetROP2` XOR self-erasing feedback drawing |
| e26 | `e26_double_buffering` | Compatible DC/bitmap + `BitBlt`; `WM_ERASEBKGND` |
| e27 | `e27_bouncing_ball` | Timer + double buffering animation |
| e28 | `e28_scrollbars` | Window scrollbars: `SetScrollInfo`, all `SB_*` codes |
| e29 | `e29_resize_aware` | `WM_SIZE`-driven re-layout |
| e30 | `e30_dpi_awareness` | Per-monitor DPI, `WM_DPICHANGED`, physical-size ruler |
| e31 | `e31_regions_clipping` | Regions + `CombineRgn` + `SelectClipRgn` |
| e32 | `e32_save_bitmap` | Client → real `.bmp` file (`GetDIBits`, packed headers) |
| e33 | `e33_menu_bar` | `CreateMenu`/`AppendMenuW`, `WM_COMMAND` routing |
| e34 | `e34_context_menu` | Right-click `TrackPopupMenu` |
| e35 | `e35_menu_state` | Checkmarks, radio groups, graying; painted state |
| e36 | `e36_dynamic_menu` | Runtime insert/remove; Recent-items submenu |
| e37 | `e37_accelerators` | `CreateAcceleratorTableW`; pump ordering |
| e38 | `e38_system_menu` | Extending the ⚙ system menu; `WM_SYSCOMMAND` masking |
| e39 | `e39_runtime_icon` | `CreateIconIndirect`; `WM_SETICON` |
| e40 | `e40_owner_draw_menu` | `MF_OWNERDRAW`, `WM_DRAWITEM`/`WM_MEASUREITEM` |
| e41 | `e41_button` | First child control; `BN_CLICKED`; control ids |
| e42 | `e42_static` | STATIC styles tour |
| e43 | `e43_edit_single` | EDIT in/out; runtime style flips (`ES_PASSWORD`) |
| e44 | `e44_edit_multiline` | Multiline EDIT with free scrolling |
| e45 | `e45_checkbox` | `BS_AUTOCHECKBOX`; `BM_GETCHECK` |
| e46 | `e46_radio_buttons` | Radio groups and the `WS_GROUP` rule |
| e47 | `e47_combobox` | Filling, `CBN_SELCHANGE`, reading the pick |
| e48 | `e48_listbox` | Selection, double-clicks, `LBS_NOTIFY` |
| e49 | `e49_scrollbar_controls` | Standalone scrollbar children vs window scrollbars |
| e50 | `e50_progressbar` | `InitCommonControlsEx`; stepping + marquee |
| e51 | `e51_trackbar` | The slider: `TBM_*`, `WM_HSCROLL` from controls |
| e52 | `e52_updown_control` | Spin buttons and buddy edits |
| e53 | `e53_statusbar` | Parts, live text, the `WM_SIZE` contract |
| e54 | `e54_toolbar` | Standard image lists, flat toolbar |
| e55 | `e55_modal_dialog` | **In-memory DLGTEMPLATE** (no .rc files); `DialogBoxIndirectParamW` |
| e56 | `e56_modeless_dialog` | `CreateDialogIndirectParamW` + `IsDialogMessage` |
| e57 | `e57_dialog_controls` | Settings dialog; data both ways via `dwInitParam` |
| e58 | `e58_task_dialog` | Modern `TaskDialogIndirect`: command links, verification |
| e59 | `e59_open_file_dialog` | `GetOpenFileNameW`; double-null filters |
| e60 | `e60_save_file_dialog` | `GetSaveFileNameW`; actually writing the file |
| e61 | `e61_color_picker` | `ChooseColorW` with persistent custom colors |
| e62 | `e62_font_picker` | `ChooseFontW` → `LOGFONTW` → live preview |
| e63 | `e63_folder_browse` | `SHBrowseForFolderW` |
| e64 | `e64_file_io_text` | Open → edit → save end-to-end |
| e65 | `e65_window_state` | **State pattern, deep**: `WM_NCCREATE` attach, `WM_NCDESTROY` teardown, re-entrancy |
| e66 | `e66_multiple_windows` | Two classes; **owned vs child** windows; sticky notes |
| e67 | `e67_custom_control` | Your own reusable child control class |
| e68 | `e68_subclassing` | `SetWindowSubclass` — hex-only EDIT |
| e69 | `e69_owner_draw_button` | `BS_OWNERDRAW` + `WM_DRAWITEM` |
| e70 | `e70_splitter_window` | Draggable divider between panes |
| e71 | `e71_tray_icon` | `Shell_NotifyIconW`: tray, balloons, cleanup |
| e72 | `e72_layered_window` | Alpha slider + color-key transparency |
| e73 | `e73_shaped_window` | `SetWindowRgn` round/star windows |
| e74 | `e74_dark_titlebar` | `DwmSetWindowAttribute` dark mode & caption colors |
| e75 | `e75_drag_drop_files` | `WM_DROPFILES` from Explorer |
| e76 | `e76_message_only_window` | `HWND_MESSAGE` invisible message sinks |
| e77 | `e77_notepad` | Real notepad: menus, open/save, dirty flag |
| e78 | `e78_calculator_basic` | Button-grid calculator, two-operand logic |
| e79 | `e79_calculator_full` | Chained ops, keyboard, history |
| e80 | `e80_todo_basic` | Todo: add, list, delete |
| e81 | `e81_todo_complete` | Done-state, status counts, filtered views |
| e82 | `e82_todo_persist` | File persistence + Ctrl+S + Recent menu |
| e83 | `e83_paint_app` | Doodle pad with menus and `.bmp` export |
| e84 | `e84_snake` | Classic snake, double-buffered |
| e85 | `e85_minesweeper` | Custom-painted board, flood-fill, first-click-safe |
| e86 | `e86_clock_analog` | Hand geometry + smooth face |
| e87 | `e87_text_viewer` | Hand-rolled text layout, scrolling, find dialog |
| e88 | `e88_unit_converter` | Live conversion; `EN_CHANGE`; data-driven units |
| e89 | `e89_clipboard` | CF_UNICODETEXT both directions; global memory |
| e90 | `e90_screen_capture` | Screen `BitBlt` + `.bmp` save |
| e91 | `e91_process_list` | Toolhelp snapshot → listbox |
| e92 | `e92_system_info` | Metrics, memory, uptime panel |
| e93 | `e93_registry_settings` | Remember window placement in HKCU |
| e94 | `e94_worker_thread` | Thread → `PostMessage` mailbox; cancel flag |
| e95 | `e95_enum_windows` | `EnumWindows` + bring-to-front |
| e96 | `e96_direct2d_hello` | Direct2D via a DC render target behind the same old HWND |
| e97 | `e97_webview` | WebView2 hello (adapted from the vendored sample; needs the WebView2 Runtime) |
| e98 | `e98_window_builder` | Build your own fluent window API — the tutorial recapitulated |
| e99 | `e99_todo_final` | **Capstone todo**: tray + dark mode + accelerators + persistence |
| e100 | `e100_mdi_editor` | Classic MDI: frame/client/child, `DefFrameProcW`/`DefMDIChildProcW` |

## How the code is written

- **Direct to the API.** `use windows::{Win32::*, core::*};` and the
  bindings, nothing in between. When you finish, you can read any Win32
  C/C++ sample and mentally translate it to Rust.
- **Self-contained.** Every example compiles alone, with only the narrow
  `windows` features it needs enabled (see each `Cargo.toml`). No shared
  helper crate — the boilerplate is the curriculum.
- **State done the Rust way.** Instead of C globals, stateful examples
  attach a `Box<State>` to the window via `GWLP_USERDATA` (introduced in
  e13, deepened in e65).
- **Comments that explain why.** Every non-obvious flag or parameter has a
  comment; every crate-version quirk (constant types, cast idioms) is
  called out where you meet it.

## Repository layout

```
004_windows-rs_tutorial/
├── Cargo.toml          # workspace manifest (examples/*/* are members)
├── examples/           # the 100 example crates, grouped by chapter
├── windows-rs/         # vendored clone of microsoft/windows-rs (path deps)
└── README.md           # this file
```

The vendored clone is untouched upstream code; it is both the dependency
source and the reference documentation — when a tutorial example uses an
API, you can read the actual bindings in
`windows-rs/crates/libs/windows/src/Windows/Win32/<module>/mod.rs`.
