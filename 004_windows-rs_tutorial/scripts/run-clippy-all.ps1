# ============================================
# Windows API Learning Projects
# Run all Cargo workspace packages
# ============================================

$ErrorActionPreference = "Stop"

$Packages = @(
    "e01_message_box"
    "e02_message_box_buttons"
    "e03_hello_window"
    "e04_hello_paint"
    "e05_window_anatomy"
    "e06_unicode_strings"
    "e07_fonts_and_text"
    "e08_client_rect"
    "e09_center_window"
    "e10_window_styles"
    "e11_message_loop"
    "e12_mouse_clicks"
    "e13_mouse_move"
    "e14_keyboard_input"
    "e15_keyboard_focus"
    "e16_timer_clock"
    "e17_hit_testing"
    "e18_shift_state"
    "e19_global_hotkey"
    "e20_cursors"
    "e21_pixels_lines"
    "e22_pens"
    "e23_brushes"
    "e24_shapes"
    "e25_drawing_modes"
    "e26_double_buffering"
    "e27_bouncing_ball"
    "e28_scrollbars"
    "e29_resize_aware"
    "e30_dpi_awareness"
    "e31_regions_clipping"
    "e32_save_bitmap"
    "e33_menu_bar"
    "e34_context_menu"
    "e35_menu_state"
    "e36_dynamic_menu"
    "e37_accelerators"
    "e38_system_menu"
    "e39_runtime_icon"
    "e40_owner_draw_menu"
    "e41_button"
    "e42_static"
    "e43_edit_single"
    "e44_edit_multiline"
    "e45_checkbox"
    "e46_radio_buttons"
    "e47_combobox"
    "e48_listbox"
    "e49_scrollbar_controls"
    "e50_progressbar"
    "e51_trackbar"
    "e52_updown_control"
    "e53_statusbar"
    "e54_toolbar"
    "e55_modal_dialog"
    "e56_modeless_dialog"
    "e57_dialog_controls"
    "e58_task_dialog"
    "e59_open_file_dialog"
    "e60_save_file_dialog"
    "e61_color_picker"
    "e62_font_picker"
    "e63_folder_browse"
    "e64_file_io_text"
    "e65_window_state"
    "e66_multiple_windows"
    "e67_custom_control"
    "e68_subclassing"
    "e69_owner_draw_button"
    "e70_splitter_window"
    "e71_tray_icon"
    "e72_layered_window"
    "e73_shaped_window"
    "e74_dark_titlebar"
    "e75_drag_drop_files"
    "e76_message_only_window"
    "e77_notepad"
    "e78_calculator_basic"
    "e79_calculator_full"
    "e80_todo_basic"
    "e81_todo_complete"
    "e82_todo_persist"
    "e83_paint_app"
    "e84_snake"
    "e85_minesweeper"
    "e86_clock_analog"
    "e87_text_viewer"
    "e88_unit_converter"
    "e89_clipboard"
    "e90_screen_capture"
    "e91_process_list"
    "e92_system_info"
)

$Total = $Packages.Count
$Index = 0

foreach ($Package in $Packages) {
    $Index++

    Write-Host ""
    Write-Host "============================================" -ForegroundColor Cyan
    Write-Host "[$Index/$Total] Running: $Package" -ForegroundColor Yellow
    Write-Host "============================================" -ForegroundColor Cyan

    & cargo clippy --package $Package

    if ($LASTEXITCODE -ne 0) {
        Write-Host ""
        Write-Host "FAILED: $Package" -ForegroundColor Red
        Write-Host "Exit code: $LASTEXITCODE" -ForegroundColor Red
        exit $LASTEXITCODE
    }

    Write-Host "SUCCESS: $Package" -ForegroundColor Green
}

Write-Host ""
Write-Host "============================================" -ForegroundColor Green
Write-Host "All $Total projects completed successfully!" -ForegroundColor Green
Write-Host "============================================" -ForegroundColor Green
