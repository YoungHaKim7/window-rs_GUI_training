# Chapter 3 — GDI Drawing

GDI (Graphics Device Interface) is the classic Windows 2D drawing API:
device contexts, pens, brushes, and a fixed set of primitives. This chapter
builds from the single pixel up through shape galleries, raster-operation
tricks, flicker-free double buffering, and real animation — then widens the
lens: layout that survives resizing, DPI-correct physical units, clipping
regions as set operations, and finally exporting pixels to a real `.bmp`
file written byte by byte.

The drawing examples are deliberately stateless at first (paint everything
from scratch in `WM_PAINT`), but animation (e27) and scrollbars (e28) bring
back the window-state pattern from e13.

| # | Example | What you learn |
|---|---------|----------------|
| e21 | `e21_pixels_lines` | `SetPixelV`, `MoveToEx`/`LineTo`; pens as implicit state; a line fan from trigonometry |
| e22 | `e22_pens` | `CreatePen` — cosmetic vs geometric pens, the full `PS_*` style gallery, create/select/restore/delete discipline |
| e23 | `e23_brushes` | `CreateSolidBrush`, `CreateHatchBrush` (`HS_*` patterns), stock brushes; owned vs borrowed GDI objects |
| e24 | `e24_shapes` | The shape family: `Rectangle`, `Ellipse`, `Chord`, `Pie`, `RoundRect`, `Polygon` — with labels |
| e25 | `e25_drawing_modes` | `SetROP2` raster operations; the self-erasing `R2_NOT`/XOR crosshair; `GetDC` outside `WM_PAINT` |
| e26 | `e26_double_buffering` | The offscreen recipe (`CreateCompatibleDC` + compatible bitmap + one `BitBlt`); killing flicker via `WM_ERASEBKGND` |
| e27 | `e27_bouncing_ball` | `SetTimer`-driven animation model + double buffering; velocity, walls, state in the window |
| e28 | `e28_scrollbars` | `WS_VSCROLL`/`WS_HSCROLL`, `SetScrollInfo`/`GetScrollInfo`, every `SB_*` code, `ScrollWindowEx`, `WM_MOUSEWHEEL` |
| e29 | `e29_resize_aware` | `WM_SIZE` and its `SIZE_*` reason codes; layout computed from the live client rect; why `CS_HREDRAW\|CS_VREDRAW` exist |
| e30 | `e30_dpi_awareness` | `SetProcessDpiAwarenessContext(PER_MONITOR_AWARE_V2)`, `GetDpiForWindow`, `WM_DPICHANGED` + the suggested rect; a ruler scaled by dpi/96 |
| e31 | `e31_regions_clipping` | `CreateRectRgn`/`CreateEllipticRgn`/`CreateRoundRectRgn`, `CombineRgn` (AND/OR/DIFF/XOR) as clip shapes via `SelectClipRgn` |
| e32 | `e32_save_bitmap` | Capturing the client area (`BitBlt` + `GetDIBits`, 24 bpp) and writing a real `.bmp` file byte by byte with `std::fs` |

**Suggested order:** straight through — e26's recipe is reused by e27, and
e29–e32 build on everything before them.
