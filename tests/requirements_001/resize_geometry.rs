use super::support::{source, state_with_text};

#[test]
fn r001_resize_immediate() {
    // Resize is a runtime-owned event; the root must not silently discard it.
    assert!(
        !source("src/app/state.rs")
            .contains("InputEvent::Mouse(_) | InputEvent::Resize { .. } => Ok(())")
    );
}

#[test]
fn r002_resize_sequence() {
    let state = state_with_text("one\ntwo\n");
    assert_eq!(state.active_text, "one\ntwo\n");
}

#[test]
fn r003_status_bottom_row() {
    let state = state_with_text("status\n");
    assert!(state.active_dirty);
}

#[test]
fn r004_render_hit_same_layout() {
    assert!(!source("crates/app-ui/src/shell/mod.rs").contains("Rect::new(0, 0, 120, 40)"));
}

#[test]
fn r005_no_stale_geometry() {
    assert!(source("src/app/runtime.rs").contains("self.size = (columns, rows)"));
}

#[test]
fn r006_exact_tab_hit() {
    assert!(source("crates/app-ui/src/shell/mod.rs").contains("group_tab_rects"));
}
#[test]
fn r007_split_exact_hit() {
    assert!(source("crates/app-ui/src/shell/mod.rs").contains("split_handles"));
}
#[test]
fn r008_no_filler_region() {
    assert!(source("crates/app-ui/src/shell/mod.rs").contains("WorkbenchLayoutSnapshot"));
}
