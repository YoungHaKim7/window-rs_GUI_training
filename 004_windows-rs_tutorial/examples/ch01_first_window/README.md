# Chapter 1 — The First Window

The opening chapter takes you from a single dialog box to a fully understood
window: what a *window class* is, how the message pump works, how text gets
painted, and how to control a window's size, position, and style.

Every example is a standalone crate — run it from the workspace root with
`cargo run -p <name>`.

| # | Example | What you learn |
|---|---------|----------------|
| e01 | `e01_message_box` | The smallest Windows program; `w!()` string literals; why Win32 calls are `unsafe` |
| e02 | `e02_message_box_buttons` | Button layouts, icons, default buttons; reading the user's answer |
| e03 | `e03_hello_window` | **The canonical window**: register class → create window → message loop → wndproc |
| e04 | `e04_hello_paint` | `WM_PAINT`, `BeginPaint`/`EndPaint`, `DrawTextW` — "Hello, World!" in the client area |
| e05 | `e05_window_anatomy` | Every `WNDCLASSW` field and `CreateWindowExW` parameter, annotated |
| e06 | `e06_unicode_strings` | `w!`, `Vec<u16>`, `HSTRING` — handing Win32 UTF-16, and reading strings back |
| e07 | `e07_fonts_and_text` | `CreateFontW`, `SelectObject`, text colors, transparent backgrounds, rotated text |
| e08 | `e08_client_rect` | `AdjustWindowRect` — an *exact* 512×512 client area (checkerboard proof) |
| e09 | `e09_center_window` | `GetSystemMetrics`; centering; `WM_MOVE`; screen vs window vs client coordinates |
| e10 | `e10_window_styles` | A live tour of `WS_`/`WS_EX_` styles — toggle the title bar, borders, topmost with keys 1–5/T |

**Suggested order:** straight through — each example assumes the previous.

If `e03` feels like a lot of boilerplate: it is, and that's the point. The
same skeleton repeats (with fewer comments) in every later example until you
can write it from memory.
