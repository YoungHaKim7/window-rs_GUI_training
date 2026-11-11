# Style guide for all 100 tutorial examples

Every example is a **self-contained crate** — no shared helper library. The
`RegisterClassW` / `CreateWindowExW` / message-loop boilerplate is repeated on
purpose: repetition is how the Win32 boilerplate becomes muscle memory, and it
keeps every example readable in isolation (Petzold tradition).

## Crate layout

```
eNN_name/
├── Cargo.toml
└── src/main.rs
```

`Cargo.toml` (copy this shape, adjust name and features):

```toml
[package]
name = "eNN_name"
version.workspace = true
edition.workspace = true
publish.workspace = true

[dependencies.windows]
workspace = true
features = ["winuser"]          # only the features this example needs
```

Run with: `cargo run -p eNN_name` (from the workspace root).

## main.rs shape

```rust
//! eNN — Title in Title Case
//!
//! What you learn:
//! * bullet list of the 1–3 concepts this example teaches
//!
//! New Win32 APIs: `FooW`, `BarW` — one line each on what they do.
//!
//! Run: `cargo run -p eNN_name`

use windows::{Win32::*, core::*};

fn main() {
    unsafe {
        // ... register class, create window, message loop
    }
}

extern "system" fn wndproc(hwnd: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe {
        match message as i32 {   // WM_* constants are i32 in this crate version
            WM_PAINT => { /* ... */ 0 }
            WM_DESTROY => {
                PostQuitMessage(0);
                0
            }
            _ => DefWindowProcW(hwnd, message, wparam, lparam),
        }
    }
}
```

## Rules

1. **Imports**: always `use windows::{Win32::*, core::*};` — the `Win32` module
   re-exports the contents of every enabled feature, so one glob covers all
   narrow features the crate enables. Only qualify deeper
   (`windows::Win32::winuser::...`) to resolve a rare ambiguity.
2. **String literals**: `w!("...")` for compile-time UTF-16 (`PCWSTR`),
   `s!("...")` for `PCSTR`. For runtime strings, build a `Vec<u16>` and pass
   `PCWSTR::from_raw(buf.as_ptr())` (keep the Vec alive!) or use `HSTRING`.
3. **Safety**: `fn main()` with an inner `unsafe { }` block — not `unsafe fn`.
   Never `unsafe impl`. Keep `unsafe` scopes reasonably tight.
4. **wndproc** is `extern "system"`, matches on `message as i32` (WM constants
   are `i32` here), returns `LRESULT`, and always falls through to
   `DefWindowProcW`.
5. **Window creation defaults** (so examples stay comparable):
   class style `CS_HREDRAW | CS_VREDRAW`, cursor `LoadCursorW(None, IDC_ARROW)?`
   — actually `IDC_ARROW` is already a `PCWSTR`-compatible constant; background
   `(COLOR_WINDOW + 1) as HBRUSH`; style `WS_OVERLAPPEDWINDOW | WS_VISIBLE`;
   position/size `(CW_USEDEFAULT, CW_USEDEFAULT, CW_USEDEFAULT, CW_USEDEFAULT)`.
6. **Errors**: for one-shot apps, `debug_assert!` / `.expect(...)` on failures
   is fine and keeps the teaching flow; return `windows::core::Result<()>`
   from main when the example is about error handling. Prefer `.ok()?` when a
   call returns a `Result`.
7. **Comments**: English, address the reader as "you", explain the *why* of
   every flag or non-obvious parameter. Target density: a comment per logical
   step, not per line.
8. **No dependencies** other than the `windows` crate (exception: the two
   chapter-10 examples that use `windows-window` / `windows-webview`).
9. **Feature list**: check `windows-rs/crates/libs/windows/Cargo.toml` for the
   exact short feature names (`winuser`, `wingdi`, `commctrl`, `commdlg`,
   `dwmapi`, `shellapi`, `tlhelp32`, `winreg`, ...). When unsure about an API
   signature, grep
   `windows-rs/crates/libs/windows/src/Windows/Win32/<module>/mod.rs` — never
   guess.
10. **Verifying API usage**: every example must compile
    (`cargo build -p eNN_name`) before it counts as done.

## Crate-version idioms (learned the hard way in ch01)

* The import is `use windows::{Win32::*, core::*};` — don't forget the
  closing `}`.
* `BOOL` (and some other returns) are `#[must_use]`: when you ignore the
  result, write `_ = SomeCall(...);` — the project builds with zero
  warnings.
* **Constant types**: most `WS_*`, `WM_*`, `MB_*`, `CS_*`, `DT_*`, `VK_*`,
  `SWP_*`, `FW_*` constants are `i32`, while many parameters are `u32`.
  Bridge with a cast in parens: `(WS_OVERLAPPEDWINDOW | WS_VISIBLE) as u32`.
* **GDI object handles** (`HFONT`, `HPEN`, `HBRUSH`, `HBITMAP`, `HBRUSH`)
  are distinct pointer types, but `SelectObject`/`DeleteObject` take
  `HGDIOBJ` (`*mut c_void`): cast with `font as HGDIOBJ`.
* Handles are **raw pointer aliases** (`HWND = *mut HWND__`): test with
  `.is_null()`, not `== 0`; null handles are `None` in `Option`-taking
  parameters.
* Stock ids: `LoadCursorW(None, PCWSTR(IDC_ARROW.0.cast()))` (the constant
  is a `PCSTR` ordinal; same dance for `IDI_APPLICATION` + `LoadIconW`).
* Background brush: `((COLOR_WINDOW + 1) as isize) as HBRUSH`.
* Class styles: `style: (CS_HREDRAW | CS_VREDRAW) as u32`.
* `MessageBoxW` flags: `(MB_OK | MB_ICONINFORMATION) as u32`.
* `GetModuleHandleW(PCWSTR::null())` returns the HINSTANCE directly
  (no `Result` in this version).
* `GetMessageW(...).0 > 0` — BOOL newtype, dereference with `.0`.
* No `RGB()` macro exists: define
  `fn rgb(r: u8, g: u8, b: u8) -> COLORREF { (r as u32) | ((g as u32) << 8) | ((b as u32) << 16) }`.
* Runtime strings: `HSTRING::from(format!(...))` works wherever a
  `Param<PCWSTR>` is accepted; for raw `PCWSTR` parameters
  (`DrawTextW`, `TextOutW`) use a `Vec<u16>` kept alive in a local:
  `let wide = to_wide(s); TextOutW(hdc, x, y, PCWSTR::from_raw(wide.as_ptr()), -1);`
* Patterns in `match` can't contain `as` casts — for `VK_ESCAPE` etc. use
  the char literal `'\u{1b}'` or an arm guard `_ if wparam as i32 == VK_ESCAPE`.
