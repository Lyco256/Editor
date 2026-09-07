use super::support::source;

#[test]
fn p001_active_buffer_authoritative() {
    assert!(source("src/app/state.rs").contains("sync_buffer_projection"));
}
#[test]
fn p002_pane_view_state_persistent() {
    assert!(source("src/app/state.rs").contains("PaneState"));
}
#[test]
fn p003_secondary_pane_isolation() {
    assert!(source("src/app/state.rs").contains("buffer_for_pane"));
}
#[test]
fn p004_search_exact_range() {
    assert!(source("crates/editor-core/src/search.rs").contains("FindMatch"));
}
#[test]
fn p005_search_stale_dirty() {
    assert!(source("src/app/state.rs").contains("find_matches"));
}
#[test]
fn p006_git_hunk_lines() {
    assert!(source("src/app/state.rs").contains("git_status"));
}
#[test]
fn p007_git_add_delete() {
    assert!(source("crates/app-ui/src/editor/mod.rs").contains("MarkerKind::Added"));
}
#[test]
fn p008_lsp_overlay_caret_anchor() {
    assert!(source("crates/app-ui/src/language/mod.rs").contains("OverlayPlacement"));
}
#[test]
fn p009_lsp_offscreen_dismiss() {
    assert!(source("crates/app-ui/src/language/mod.rs").contains("dismiss_if_cursor_moved"));
}
#[test]
fn p010_lsp_document_identity() {
    assert!(source("src/app/state.rs").contains("document_id"));
}
#[test]
fn p011_contextual_not_language_panel() {
    assert!(source("crates/app-ui/src/language/mod.rs").contains("ContextualOverlay"));
}
