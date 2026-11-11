# Chapter 5 — Controls

Win32 ships with a whole zoo of ready-made controls — buttons, edits,
checkboxes, radios, combos, listboxes, bars and sliders. They are just
child windows of built-in window classes (BUTTON, EDIT, ...), they talk to
you through `WM_COMMAND`, and you create them with the same
`CreateWindowExW` you already know. This chapter meets all of them.

| # | Example | What you learn |
|---|---------|----------------|
| e41 | `e41_button` | The "hello world" of controls: a BUTTON child, `BN_CLICKED`, control ids via the `hmenu` parameter |
| e42 | `e42_static` | STATIC labels and frames: `SS_*` styles; why statics take no input by design |
| e43 | `e43_edit_single` | Single-line EDIT: `GetWindowTextW`/`SetWindowTextW`, runtime style flips (`ES_PASSWORD`) |
| e44 | `e44_edit_multiline` | Multiline EDIT with free scrolling: `ES_MULTILINE`, `WS_VSCROLL`, `ES_WANTRETURN` |
| e45 | `e45_checkbox` | A checkbox is a BUTTON style: `BS_AUTOCHECKBOX`, `BM_GETCHECK`, timer-driven reaction |
| e46 | `e46_radio_buttons` | Radio groups: `BS_AUTORADIOBUTTON` and the `WS_GROUP` grouping rule |
| e47 | `e47_combobox` | `CBS_DROPDOWNLIST`: filling (`CB_ADDSTRING`), notifications (`CBN_SELCHANGE`), reading the pick |
| e48 | `e48_listbox` | `LBS_NOTIFY` listbox: selection, double-clicks, programmatic selection |
| e49 | `e49_scrollbar_controls` | A standalone SCROLLBAR control (vs. window scrollbars in e28): `WM_VSCROLL` with `lparam` = control |
| e50 | `e50_progressbar` | Common controls start here: `InitCommonControlsEx`, `PBM_*` stepping, and the marquee mode |
| e51 | `e51_trackbar` | The slider: `TBM_*` range/position, `WM_HSCROLL` from a control, tick marks |
| e52 | `e52_updown_control` | Spin buttons and their *buddy* edit (`UDS_SETBUDDYINT`, `UDM_SETRANGE32`) |
| e53 | `e53_statusbar` | Status bars: parts (`SB_SETPARTS`), live text (`SB_SETTEXTW`), the `WM_SIZE` resize contract |
| e54 | `e54_toolbar` | Toolbars: standard image lists (`TB_LOADIMAGES`), buttons, flat style & tooltips |

**Suggested order:** straight through — the controls get richer but the
`WM_COMMAND` plumbing never changes.
