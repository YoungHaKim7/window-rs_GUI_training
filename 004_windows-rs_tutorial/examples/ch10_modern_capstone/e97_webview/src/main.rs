//! e97 — WebView2: a real browser engine inside your window
//!
//! What you learn:
//! * How small "modern Windows" can be: `windows-webview` wraps WebView2
//!   (the Chromium-based engine inside Edge) behind one builder, and
//!   `windows-window` supplies the parent window + message loop — together
//!   they collapse the whole e03 boilerplate (RegisterClass, wndproc,
//!   GetMessage pump) into a handful of chained calls
//! * The builder chain, step by step: `WebViewWindow::new(title)` starts
//!   with a `windows-window::WindowBuilder`; `.size(w, h)` sets the OUTER
//!   size (borders included — use `.client_size` for the inner one);
//!   `.run(setup)` creates the window AND the WebView2 controller, runs
//!   the message loop until the window closes, then tears the browser down
//! * The `setup` callback: runs on the UI thread once the controller
//!   exists. You get a `&WebViewWindow`; `host.webview()` is the browser
//!   itself, and `navigate(url)` starts a load. Everything that can fail
//!   here returns `windows_core::Result`, and `?` propagates straight out
//!   through `main -> Result<()>`
//! * What's hidden below (worth knowing once): the crates handle COM STA
//!   init, the async controller creation (the callback pattern from e94,
//!   but with COM events instead of PostMessage), bounds-sync on resize,
//!   and clean shutdown ordering (browser before window)
//!
//! This is a faithful adaptation of the vendored sample at
//! `windows-rs/crates/samples/webview/minimal` — same structure, same
//! target URL, comments added. Needs the WebView2 Runtime installed
//! (standard on current Windows 10/11) and network access for the page.
//!
//! New Win32 APIs: none by hand — that's the point. (`WebViewWindow`,
//! `WebView`, `navigate` are the windows-webview/windows-window crates.)
//!
//! Run: `cargo run -p e97_webview`

use windows_webview::*;

fn main() -> Result<()> {
    // Step 1: start building the window + browser pair. The title sets the
    // Win32 caption exactly as in e03.
    WebViewWindow::new("e97 · WebView — windows-rs tutorial")
        // Step 2: outer window size in pixels (1024x768, matching the sample).
        .size(1024, 768)
        // Step 3: create everything and BLOCK on the message loop. The
        // closure runs once the WebView2 controller is ready — it is the
        // only place you're allowed to talk to the browser before the loop
        // starts. Returning Err from it aborts startup with that error.
        .run(|host| {
            // Step 4: grab the browser (the `WebView` — navigation, script,
            // settings; the `host` wrapper also owns the controller that
            // sizes it to the window).
            let webview = host.webview();

            // Step 5: load a page. This is an ordinary navigation — the
            // same engine serves file://, data: and custom app:// schemes
            // (see the webview crate's `protocol` module) once you go
            // further than this hello.
            webview.navigate("https://learn.microsoft.com/windows/dev-environment/")?;

            Ok(())
        })
}
