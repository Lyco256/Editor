use super::support::source;

#[test]
fn m001_click_text() {
    assert!(source("src/app/state.rs").contains("MouseAction::Down(MouseButton::Left)"));
}
#[test]
fn m002_click_right_to_eol() {
    assert!(source("src/app/state.rs").contains("pointer"));
}
#[test]
fn m003_click_empty_line() {
    assert!(source("crates/app-ui/src/editor/mod.rs").contains("cursor_cell"));
}
#[test]
fn m004_click_below_to_eof() {
    assert!(source("crates/editor-core/src/buffer.rs").contains("offset_for_display_column"));
}
#[test]
fn m005_drag_clamp_eol_eof() {
    assert!(source("src/app/state.rs").contains("MouseAction::Drag(MouseButton::Left)"));
}
#[test]
fn m006_shift_click() {
    assert!(source("src/app/state.rs").contains("extend_selection"));
}
#[test]
fn m007_alt_click_cursor() {
    assert!(source("src/app/state.rs").contains("Modifier::Alt"));
}
#[test]
fn m008_double_click_word() {
    assert!(source("src/app/state.rs").contains("click_count"));
}
#[test]
fn m009_triple_click_line() {
    assert!(source("src/app/state.rs").contains("select_line"));
}
#[test]
fn m010_wheel_target() {
    assert!(source("src/app/state.rs").contains("ScrollLines"));
}
#[test]
fn m011_primary_caret_preserves_glyph() {
    assert!(!source("crates/app-ui/src/editor/mod.rs").contains("▌"));
}
#[test]
fn m012_primary_caret_steady_bar() {
    assert!(source("src/app/runtime.rs").contains("present_cursor"));
}
#[test]
fn m013_selection_preserves_glyph() {
    assert!(source("crates/terminal-backend/src/framebuffer.rs").contains("continuation"));
}
#[test]
fn m014_secondary_cursor_preserves_glyph() {
    assert!(source("crates/app-ui/src/editor/mod.rs").contains("selections"));
}
#[test]
fn m015_cjk_combining_pointer() {
    assert!(source("crates/terminal-backend/src/framebuffer.rs").contains("UnicodeSegmentation"));
}
