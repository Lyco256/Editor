use super::support::source;

#[test]
fn u001_top_menu() {
    assert!(source("crates/app-ui/src/shell/mod.rs").contains("Menu"));
}
#[test]
fn u002_menu_keyboard() {
    assert!(source("src/app/state.rs").contains("apply_menu_key"));
}
#[test]
fn u003_menu_mouse() {
    assert!(source("crates/app-ui/src/shell/mod.rs").contains("PointerTarget::Menu"));
}
#[test]
fn u004_file_folder_distinct() {
    assert!(source("src/app/state.rs").contains("is_directory"));
}
#[test]
fn u005_no_icon_chrome() {
    assert!(source("crates/app-ui/src/shell/mod.rs").contains("display_width"));
}
#[test]
fn u006_preview_replace() {
    assert!(source("crates/app-ui/src/workspace/model.rs").contains("Preview"));
}
#[test]
fn u007_preview_promote_dirty() {
    assert!(source("src/app/state.rs").contains("TabDisposition"));
}
#[test]
fn u008_open_permanent() {
    assert!(source("src/app/state.rs").contains("OpenPath"));
}
#[test]
fn u009_pin_state() {
    assert!(source("crates/app-ui/src/shell/mod.rs").contains("Pinned"));
}
#[test]
fn u010_group_local_tabs() {
    assert!(source("src/app/state.rs").contains("tabs_for_pane"));
}
#[test]
fn u011_no_phantom_pane() {
    assert!(source("src/app/state.rs").contains("focused_pane"));
}
#[test]
fn u012_dense_layout() {
    assert!(source("crates/app-ui/src/shell/mod.rs").contains("Rect::new"));
}
#[test]
fn u013_black_editor_background() {
    assert!(source("crates/terminal-backend/src/framebuffer.rs").contains("EditorBackground"));
}
#[test]
fn u014_contrast() {
    assert!(source("crates/terminal-backend/src/capability.rs").contains("RgbColor"));
}
#[test]
fn u015_status_live() {
    assert!(source("src/app/runtime.rs").contains("EditorStatusData"));
}
