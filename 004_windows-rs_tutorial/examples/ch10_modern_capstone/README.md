# Chapter 10 — Modern Capstone

The final chapter jumps from "raw Win32 with your own hands" to where
Windows GUI development actually lives now, then lands the whole tutorial.
First contact comes through the modern stacks the vendored windows-rs
ecosystem itself ships: Direct2D (GPU rendering straight into a WM_PAINT
device context) and WebView2 (a full Chromium browser hosted in your
window). Then the perspective flips — e98 rebuilds e03's boilerplate into a
fluent builder and shows it's the same design as the vendored
`windows-window` crate, i.e. what a "framework" is. Finally, e99 and e100
are the two capstones: a todo app combining every technique from the
tutorial, and a classic MDI editor — the 1990s' answer to "many documents,
one frame" — as the traditional last exam. Note: e97 needs the WebView2
Runtime (standard on current Windows 10/11) and network access for its
page; everything else in the chapter runs fully offline.

- **e96** `e96_direct2d_hello` — minimal Direct2D via a DC render target:
  `CoInitializeEx`, `D2D1CreateFactory`, `BindDC` + `BeginDraw`/`EndDraw` in
  WM_PAINT, f32 brushes, two ellipses and a rectangle, `D2DERR_RECREATE_TARGET`
  handling (the vendored direct2d sample reduced to essentials)
- **e97** `e97_webview` — a faithful adaptation of the vendored
  `samples/webview/minimal`: `WebViewWindow::new(...).size(...).run(...)`,
  navigate to a real page — the whole e03 skeleton replaced by two crates
- **e98** `e98_window_builder` — build a mini fluent windowing helper *inside*
  the example: `.title()`/`.size()`/`.on_message(Box<dyn FnMut ...>)`, the
  closure stored via GWLP_USERDATA (double-boxed — fat pointers!),
  `Option<LRESULT>` as "handled", then e04's hello-paint app reborn in ~20
  lines; explicitly re-derives the vendored `windows-window` crate
- **e99** `e99_todo_final` — the tutorial-combining todo: listbox with done
  toggle, status-bar counts, tray icon with restore menu, dark-titlebar menu
  toggle (DWM), Ctrl+N/Del/Ctrl+S accelerators, todo.txt persistence next to
  the exe, About via TaskDialog
- **e100** `e100_mdi_editor` — the MDI finale: frame + menu, the MDICLIENT
  window created from a `CLIENTCREATESTRUCT` (your Window submenu becomes the
  system's document list), documents born via `WM_MDICREATE` hosting multiline
  EDITs, Tile/Cascade/Close All, and the `DefFrameProcW`/`DefMDIChildProcW`
  pair explained

**Suggested order:** e96 → e97 (modern stacks), then e98 (what a framework
is), then either capstone — e99 if you want the tutorial's techniques
rehearsed together, e100 if you want the history lesson. They are
independent.
