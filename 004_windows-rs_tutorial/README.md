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

<!-- INDEX -->

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
