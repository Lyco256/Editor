use super::support::source;

#[test]
fn q001_unicode_chrome_width() {
    assert!(source("crates/terminal-backend/src/framebuffer.rs").contains("UnicodeWidthStr"));
}
#[test]
fn q002_resize_property() {
    assert!(source("src/app/runtime.rs").contains("Resize { columns, rows }"));
}
#[test]
fn q003_no_fixed_geometry_patterns() {
    assert!(!source("src/app/state.rs").contains("120x40"));
}
#[test]
fn q004_no_cursor_glyph_pattern() {
    let text = source("crates/app-ui/src/editor/mod.rs");
    let production_marker = format!("#{}{}(test)]", "[", "cfg");
    let production = text.split(&production_marker).next().unwrap_or(&text);
    assert!(!production.contains("▌"));
}
#[test]
fn q005_no_required_ignores() {
    let marker = format!("#{}ignore]", "[");
    assert!(!source("tests/requirements_001.rs").contains(&marker));
}
#[test]
fn q006_frozen_oracle_unchanged() {
    assert!(
        source("Requirements/001/005_FREEZE_ACCEPTANCE_BASELINE.md").contains("BASELINE_REF.txt")
    );
}
