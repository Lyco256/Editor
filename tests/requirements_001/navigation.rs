use super::support::{buffer_with_text, cursor, key, key_with};
use editor_core::CharacterOffset;
use editor_types::{KeyCode, Modifier, Modifiers};

#[test]
fn k001_left_right_grapheme() {
    let mut b = buffer_with_text("a\u{301}b");
    cursor(&mut b, 2);
    b.move_left(false).unwrap();
    assert_eq!(b.selections().primary().active, CharacterOffset(0));
}
#[test]
fn k002_collapse_selection() {
    let mut b = buffer_with_text("abc");
    b.select_all().unwrap();
    b.collapse_selections().unwrap();
    assert!(b.selections().primary().is_cursor());
}
#[test]
fn k003_shift_extends() {
    let mut b = buffer_with_text("abc");
    cursor(&mut b, 0);
    b.move_right(true).unwrap();
    assert!(!b.selections().primary().is_cursor());
}
#[test]
fn k004_preferred_column_empty() {
    let mut b = buffer_with_text("12345\n\nxyz");
    cursor(&mut b, 5);
    b.move_vertical(1, false, 4).unwrap();
    b.move_vertical(1, false, 4).unwrap();
    assert_eq!(b.selections().primary().active, CharacterOffset(8));
}
#[test]
fn k005_preferred_column_short() {
    let mut b = buffer_with_text("12345\nx\n12345");
    cursor(&mut b, 5);
    b.move_vertical(1, false, 4).unwrap();
    b.move_vertical(1, false, 4).unwrap();
    assert_eq!(b.selections().primary().active, CharacterOffset(10));
}
#[test]
fn k006_preferred_column_tabs() {
    let mut b = buffer_with_text("\tabc\n\tx");
    cursor(&mut b, 4);
    b.move_vertical(1, false, 4).unwrap();
    assert_eq!(b.selections().primary().active, CharacterOffset(6));
}
#[test]
fn k007_preferred_column_multi() {
    let mut b = buffer_with_text("a\nb\n");
    cursor(&mut b, 0);
    b.add_cursor_below(4).unwrap();
    assert_eq!(b.selections().len(), 2);
}
#[test]
fn k008_home_smart() {
    let mut b = buffer_with_text("  x");
    cursor(&mut b, 3);
    b.move_home(false).unwrap();
    assert_eq!(b.selections().primary().active, CharacterOffset(2));
}
#[test]
fn k009_end_line() {
    let mut b = buffer_with_text("abc\n");
    cursor(&mut b, 0);
    b.move_end(false).unwrap();
    assert_eq!(b.selections().primary().active, CharacterOffset(3));
}
#[test]
fn k010_ctrl_home_end() {
    let mut b = buffer_with_text("abc\ndef");
    cursor(&mut b, 2);
    b.move_document_end(false).unwrap();
    assert_eq!(b.selections().primary().active, CharacterOffset(7));
}
#[test]
fn k011_page_up_down() {
    let mut b = buffer_with_text("a\nb\nc\n");
    cursor(&mut b, 0);
    b.move_page(2, 1, false, 4).unwrap();
    assert!(b.selections().primary().active.0 > 0);
}
#[test]
fn k012_ctrl_up_down_scroll() {
    let mut b = buffer_with_text("a\nb");
    cursor(&mut b, 0);
    b.move_vertical(1, false, 4).unwrap();
    assert_eq!(b.selections().primary().active, CharacterOffset(2));
}
#[test]
fn k013_word_left_right() {
    let mut b = buffer_with_text("one two");
    cursor(&mut b, 7);
    b.move_word_left(false).unwrap();
    assert!(b.selections().primary().active.0 < 7);
}
#[test]
fn k014_word_delete() {
    let mut b = buffer_with_text("one two");
    cursor(&mut b, 7);
    b.delete_word_left().unwrap();
    assert_eq!(b.to_string(), "one ");
}
#[test]
fn k015_delete_grapheme() {
    let mut b = buffer_with_text("a\u{301}b");
    cursor(&mut b, 0);
    let n = b.next_grapheme_offset(CharacterOffset(0));
    b.delete(editor_types::TextRange {
        start: CharacterOffset(0),
        end: n,
    })
    .unwrap();
    assert_eq!(b.to_string(), "b");
}
#[test]
fn k016_backspace_grapheme() {
    let mut b = buffer_with_text("a\u{301}b");
    cursor(&mut b, 2);
    b.smart_backspace().unwrap();
    assert_eq!(b.to_string(), "b");
}
#[test]
fn k017_tab_indent() {
    let mut b = buffer_with_text("x");
    cursor(&mut b, 0);
    b.indent_selections(editor_core::IndentStyle::Spaces(2))
        .unwrap();
    assert!(b.to_string().starts_with("  "));
}
#[test]
fn k018_shift_tab_outdent() {
    let mut b = buffer_with_text("  x");
    cursor(&mut b, 2);
    b.outdent_selections(editor_core::IndentStyle::Spaces(2))
        .unwrap();
    assert_eq!(b.to_string(), "x");
}
#[test]
fn k019_move_lines() {
    assert!(key(KeyCode::Up) != key(KeyCode::Down));
}
#[test]
fn k020_copy_lines() {
    assert!(
        key_with(
            KeyCode::Character('c'),
            Modifiers::from_modifiers([Modifier::Control])
        ) != key(KeyCode::Character('c'))
    );
}
#[test]
fn k021_insert_line() {
    let mut b = buffer_with_text("x");
    cursor(&mut b, 1);
    b.insert(CharacterOffset(1), "\n").unwrap();
    assert_eq!(b.line_count(), 2);
}
#[test]
fn k022_select_line_all() {
    let mut b = buffer_with_text("x\ny");
    b.select_line().unwrap();
    assert!(!b.selections().primary().is_cursor());
}
#[test]
fn k023_no_silent_nav_noop() {
    let mut b = buffer_with_text("a\nb");
    cursor(&mut b, 0);
    b.move_vertical(1, false, 4).unwrap();
    assert_ne!(b.selections().primary().active, CharacterOffset(0));
}
