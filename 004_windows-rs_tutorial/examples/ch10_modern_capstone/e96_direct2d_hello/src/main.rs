//! e96 — Direct2D Hello: the modern GPU renderer behind the same old HWND
//!
//! What you learn:
//! * Direct2D in miniature: COM apartment init
//!   (`CoInitializeEx(COINIT_APARTMENTTHREADED)`), `D2D1CreateFactory`
//!   with the `ID2D1Factory` IID, then a **DC render target**
//!   (`ID2D1DCRenderTarget`) — the flavor of D2D target that renders
//!   directly INTO a GDI device context, so it slots into the plain
//!   WM_PAINT/BeginPaint rhythm from e04 without swap chains or DXGI
//! * The per-paint ritual: `BindDC` (aim the target at this HDC + client
//!   rect) → `BeginDraw` → draw → `EndDraw`. EndDraw is also the
//!   error-check point: `D2DERR_RECREATE_TARGET` means the GPU device went
//!   away (driver reset, sleep/wake) and the target must be recreated —
//!   the Direct2D stand-in for GDI's "just try again"
//! * The drawing vocabulary swap vs. GDI: no SelectObject juggling — you
//!   create brushes once (`CreateSolidColorBrush`, RGBA as four f32s,
//!   alpha included, no COLORREF packing) and pass them per call
//!   (`FillEllipse`, `DrawEllipse`, `FillRectangle`); coordinates are
//!   f32 DIPs, not ints
//! * COM + Rust: interfaces are reference-counted smart pointers — clone
//!   bumps the count, drop Releases it; no manual AddRef/Release anywhere
//!
//! This is the vendored windows-rs `direct2d` sample reduced to its
//! minimum — that sample adds DXGI swap chains, D3D devices, effects and
//! animation; the DC-render-target path here trades a little performance
//! for a lot of simplicity. Resize the window: the target re-binds to the
//! new rect every paint.
//!
//! New Win32 APIs: `CoInitializeEx` (combaseapi/objbase), `D2D1CreateFactory`,
//! `ID2D1Factory::CreateDCRenderTarget`, `ID2D1DCRenderTarget::BindDC`,
//! `BeginDraw`/`EndDraw`/`Clear`/`CreateSolidColorBrush`/`FillEllipse`/
//! `DrawEllipse`/`FillRectangle` (d2d/dcommon).
//!
//! Run: `cargo run -p e96_direct2d_hello`

use windows::{Win32::*, core::*};

// NOTE on paths: in this crate version the feature modules are re-exported
// FLAT under Win32 — ID2D1Factory et al. arrive via the `Win32::*` glob
// above, not through a Graphics::D2D path.
struct State {
    factory: ID2D1Factory,
    /// Created lazily on first paint; dropped (and recreated) if EndDraw
    /// ever reports D2DERR_RECREATE_TARGET.
    target: Option<ID2D1DCRenderTarget>,
}

fn main() {
    unsafe {
        // ---- COM init: Direct2D is a COM API ---------------------------
        // APARTMENTTHREADED (STA) is what UI code wants: single-threaded,
        // re-entrant message pumping under the hood. MULTITHREADED is for
        // worker threads or headless rendering. Failures other than
        // S_FALSE ("already initialized on this thread") we treat as fatal.
        CoInitializeEx(None, COINIT_APARTMENTTHREADED as u32)
            .ok()
            .expect("CoInitializeEx");

        // ---- The D2D1 factory: the root object everything else comes from.
        // SINGLE_THREADED is cheaper and correct for one UI thread. Note the
        // raw ABI shape: out-param of *mut c_void, then transmuted into the
        // typed interface via Interface::from_raw.
        let mut raw: *mut core::ffi::c_void = std::ptr::null_mut();
        D2D1CreateFactory(
            D2D1_FACTORY_TYPE_SINGLE_THREADED,
            &ID2D1Factory::IID,
            Some(&D2D1_FACTORY_OPTIONS::default()),
            &mut raw,
        )
        .ok()
        .expect("D2D1CreateFactory");
        let factory: ID2D1Factory = Interface::from_raw(raw);

        let instance = GetModuleHandleW(PCWSTR::null());
        debug_assert!(!instance.is_null());

        let wc = WNDCLASSW {
            lpfnWndProc: Some(wndproc),
            hInstance: instance,
            hCursor: LoadCursorW(None, PCWSTR(IDC_ARROW.0.cast())),
            // No background brush on purpose: D2D clears the whole client
            // area itself, and letting GDI also erase would just flicker.
            hbrBackground: 0 as HBRUSH,
            lpszClassName: w!("e96_class"),
            style: (CS_HREDRAW | CS_VREDRAW) as u32,
            ..Default::default()
        };
        debug_assert!(RegisterClassW(&wc) != 0);

        let hwnd = CreateWindowExW(
            0,
            w!("e96_class"),
            w!("e96 \u{b7} Direct2D Hello \u{2014} DC render target"),
            (WS_OVERLAPPEDWINDOW | WS_VISIBLE) as u32,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            640,
            480,
            None,
            None,
            Some(instance),
            None,
        );
        debug_assert!(!hwnd.is_null());

        let state = Box::into_raw(Box::new(State {
            factory,
            target: None,
        }));
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, state as isize);

        let mut message = MSG::default();
        while GetMessageW(&mut message, None, 0, 0).0 > 0 {
            _ = TranslateMessage(&message);
            DispatchMessageW(&message);
        }

        // Interfaces Release themselves on drop; CoUninitialize pairs with
        // the CoInitializeEx at the top (once per thread, balance them).
        CoUninitialize();
    }
}

unsafe fn state_of(hwnd: HWND) -> Option<&'static mut State> {
    unsafe {
        let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut State;
        (!ptr.is_null()).then(|| &mut *ptr)
    }
}

/// Build the DC render target. This shape is where the bindings get picky:
/// `r#type` (escaped keyword), DXGI_FORMAT for the pixel format, and
/// alphaMode IGNORE because we draw onto an opaque window, not a layered one.
unsafe fn create_target(factory: &ID2D1Factory) -> Result<ID2D1DCRenderTarget> {
    unsafe {
        let props = D2D1_RENDER_TARGET_PROPERTIES {
            r#type: D2D1_RENDER_TARGET_TYPE_SOFTWARE, // rasterize anywhere, incl. RDP
            pixelFormat: D2D1_PIXEL_FORMAT {
                format: DXGI_FORMAT_B8G8R8A8_UNORM, // 32-bpp, the window-friendly format
                alphaMode: D2D1_ALPHA_MODE_IGNORE,  // destination has no alpha channel
            },
            dpiX: 0.0, // 0.0 = "use the desktop DPI"
            dpiY: 0.0,
            usage: D2D1_RENDER_TARGET_USAGE_NONE,
            minLevel: D2D1_FEATURE_LEVEL_DEFAULT, // no minimum D3D feature level
        };
        factory.CreateDCRenderTarget(&props)
    }
}

extern "system" fn wndproc(hwnd: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe {
        let Some(state) = state_of(hwnd) else {
            return DefWindowProcW(hwnd, message, wparam, lparam);
        };

        match message as i32 {
            WM_PAINT => {
                let mut ps = PAINTSTRUCT::default();
                let hdc = BeginPaint(hwnd, &mut ps);

                let mut rect = RECT::default();
                _ = GetClientRect(hwnd, &mut rect);

                // Lazy-create on first paint (creating in main would work
                // too; first-paint is the habit that survives resizing).
                if state.target.is_none() {
                    match create_target(&state.factory) {
                        Ok(target) => state.target = Some(target),
                        Err(e) => {
                            println!("CreateDCRenderTarget failed: {e}");
                            _ = EndPaint(hwnd, &ps);
                            return 0;
                        }
                    }
                }
                let target = state.target.as_ref().unwrap();

                // Re-bind EVERY paint: the target renders into (hdc, rect),
                // and both change when the window is resized.
                if target.BindDC(hdc, &rect).is_err() {
                    _ = EndPaint(hwnd, &ps);
                    return 0;
                }

                let w = (rect.right - rect.left) as f32;
                let h = (rect.bottom - rect.top) as f32;

                // ---- BeginDraw / draw / EndDraw --------------------------
                _ = target.BeginDraw();

                // Colors are four f32s (RGBA), premultiplied by convention.
                // Dark slate background, painted over the whole client area.
                target.Clear(Some(&D2D1_COLOR_F {
                    r: 0.09,
                    g: 0.13,
                    b: 0.17,
                    a: 1.0,
                }));

                // Brushes are device-adjacent resources: create once per
                // draw here for clarity (a real app caches them alongside
                // the target). Pass None for brush properties = defaults.
                let teal = target
                    .CreateSolidColorBrush(
                        &D2D1_COLOR_F {
                            r: 0.11,
                            g: 0.75,
                            b: 0.70,
                            a: 1.0,
                        },
                        None,
                    )
                    .expect("brush");
                let coral = target
                    .CreateSolidColorBrush(
                        &D2D1_COLOR_F {
                            r: 0.95,
                            g: 0.36,
                            b: 0.28,
                            a: 1.0,
                        },
                        None,
                    )
                    .expect("brush");
                let sand = target
                    .CreateSolidColorBrush(
                        &D2D1_COLOR_F {
                            r: 0.93,
                            g: 0.87,
                            b: 0.70,
                            a: 1.0,
                        },
                        None,
                    )
                    .expect("brush");

                // Ellipse 1 (filled): center-left, sized off the window.
                // The point type comes from windows-numerics (Vector2) —
                // Default + field writes sidestep naming that type.
                let mut e1 = D2D1_ELLIPSE::default();
                e1.point.x = w * 0.32;
                e1.point.y = h * 0.42;
                e1.radiusX = w.min(h) * 0.22;
                e1.radiusY = w.min(h) * 0.22;
                target.FillEllipse(&e1, &teal);

                // Ellipse 2 (stroked): a ring around it. Stroke width is a
                // plain f32 — DIPs, so it stays visually stable across DPI.
                let mut e2 = D2D1_ELLIPSE::default();
                e2.point.x = w * 0.68;
                e2.point.y = h * 0.42;
                e2.radiusX = w.min(h) * 0.20;
                e2.radiusY = w.min(h) * 0.20;
                target.DrawEllipse(&e2, &coral, 6.0, None);

                // A rectangle tying them together. D2D1_RECT_F is the f32
                // cousin of windef's RECT — l/t/r/b, not left/top/right/bottom.
                let band = D2D1_RECT_F {
                    left: w * 0.18,
                    top: h * 0.78,
                    right: w * 0.82,
                    bottom: h * 0.86,
                };
                target.FillRectangle(&band, &sand);

                // EndDraw returns everything that went wrong during the
                // batch. D2DERR_RECREATE_TARGET = device lost: throw the
                // target away and let the next WM_PAINT build a fresh one.
                if let Err(err) = target.EndDraw(None, None).ok() {
                    if err.code() == D2DERR_RECREATE_TARGET {
                        state.target = None;
                    }
                }

                _ = EndPaint(hwnd, &ps);
                0
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

            _ => DefWindowProcW(hwnd, message, wparam, lparam),
        }
    }
}
