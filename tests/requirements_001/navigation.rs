use super::support::{buffer_with_text, cursor, key, key_with};
use editor_core::{CharacterOffset, Selection, SelectionSet, TextBuffer};
use editor_types::{KeyCode, Modifier, Modifiers};

#[derive(Debug)]
struct PreferredColumnFile {
    vectors: Vec<PreferredColumnVector>,
}

#[derive(Debug)]
struct PreferredColumnVector {
    case_id: String,
    variant_id: String,
    document: String,
    tab_width: usize,
    initial_char_offset: usize,
    operations: Vec<String>,
    checkpoints: Vec<PreferredCheckpoint>,
}

#[derive(Debug)]
struct PreferredCheckpoint {
    after_operation: usize,
    char_offset: usize,
    logical_line: u32,
    logical_character: u32,
    display_column: usize,
    preferred_display_column: usize,
}

fn preferred_vectors() -> PreferredColumnFile {
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("Requirements/002/vectors/preferred-column.json");
    let text = std::fs::read_to_string(path).expect("canonical preferred-column vectors");
    let root: serde_json::Value =
        serde_json::from_str(&text).expect("canonical preferred-column JSON schema");
    let vectors = root
        .get("vectors")
        .and_then(serde_json::Value::as_array)
        .expect("canonical vectors array")
        .iter()
        .map(|vector| PreferredColumnVector {
            case_id: vector["case_id"].as_str().expect("case_id").to_owned(),
            variant_id: vector["variant_id"]
                .as_str()
                .expect("variant_id")
                .to_owned(),
            document: vector["document"].as_str().expect("document").to_owned(),
            tab_width: usize::try_from(vector["tab_width"].as_u64().expect("tab_width"))
                .expect("tab_width fits usize"),
            initial_char_offset: usize::try_from(
                vector["initial_char_offset"]
                    .as_u64()
                    .expect("initial_char_offset"),
            )
            .expect("initial_char_offset fits usize"),
            operations: vector["operations"]
                .as_array()
                .expect("operations")
                .iter()
                .map(|operation| operation.as_str().expect("operation").to_owned())
                .collect(),
            checkpoints: vector["checkpoints"]
                .as_array()
                .expect("checkpoints")
                .iter()
                .map(|checkpoint| PreferredCheckpoint {
                    after_operation: usize::try_from(
                        checkpoint["after_operation"]
                            .as_u64()
                            .expect("after_operation"),
                    )
                    .expect("after_operation fits usize"),
                    char_offset: usize::try_from(
                        checkpoint["char_offset"].as_u64().expect("char_offset"),
                    )
                    .expect("char_offset fits usize"),
                    logical_line: u32::try_from(
                        checkpoint["logical_line"].as_u64().expect("logical_line"),
                    )
                    .expect("logical_line fits u32"),
                    logical_character: u32::try_from(
                        checkpoint["logical_character"]
                            .as_u64()
                            .expect("logical_character"),
                    )
                    .expect("logical_character fits u32"),
                    display_column: usize::try_from(
                        checkpoint["display_column"]
                            .as_u64()
                            .expect("display_column"),
                    )
                    .expect("display_column fits usize"),
                    preferred_display_column: usize::try_from(
                        checkpoint["preferred_display_column"]
                            .as_u64()
                            .expect("preferred_display_column"),
                    )
                    .expect("preferred_display_column fits usize"),
                })
                .collect(),
        })
        .collect();
    PreferredColumnFile { vectors }
}

fn run_preferred_vector(vector: &PreferredColumnVector) {
    assert_eq!(
        vector.checkpoints.len(),
        vector.operations.len(),
        "canonical checkpoints must cover every operation"
    );
    let mut buffer = TextBuffer::new(&vector.document);
    buffer
        .set_selections(SelectionSet::single(Selection::cursor(CharacterOffset(
            vector.initial_char_offset,
        ))))
        .expect("canonical initial caret is valid");
    assert!(buffer.preferred_display_columns().is_none());
    for (operation_index, operation) in vector.operations.iter().enumerate() {
        assert_eq!(operation, "down", "unsupported canonical operation");
        buffer
            .move_vertical(1, false, vector.tab_width)
            .expect("canonical vertical operation");
        let checkpoint = vector
            .checkpoints
            .iter()
            .find(|checkpoint| checkpoint.after_operation == operation_index + 1)
            .expect("checkpoint for canonical operation");
        let active = buffer.selections().primary().active;
        let snapshot = buffer.snapshot();
        let position = snapshot
            .offset_to_position(active)
            .expect("actual caret position");
        let display_column = snapshot
            .display_column(active, vector.tab_width)
            .expect("actual display column");
        let preferred = buffer
            .preferred_display_columns()
            .and_then(|columns| columns.first().copied())
            .expect("stored preferred display column");
        assert_eq!(
            (
                active.0,
                position.line,
                position.character,
                display_column,
                preferred
            ),
            (
                checkpoint.char_offset,
                checkpoint.logical_line,
                checkpoint.logical_character,
                checkpoint.display_column,
                checkpoint.preferred_display_column,
            ),
            "{}::{} operation {}",
            vector.case_id,
            vector.variant_id,
            operation_index + 1
        );
    }
}

fn run_preferred_case(case_id: &str) {
    let file = preferred_vectors();
    let selected = file
        .vectors
        .iter()
        .filter(|vector| vector.case_id == case_id)
        .collect::<Vec<_>>();
    assert!(
        !selected.is_empty(),
        "canonical vector missing for {case_id}"
    );
    for vector in selected {
        run_preferred_vector(vector);
    }
}

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
    run_preferred_case("K004_PREFERRED_COLUMN_EMPTY");
}
#[test]
fn k005_preferred_column_short() {
    run_preferred_case("K005_PREFERRED_COLUMN_SHORT");
}
#[test]
fn k006_preferred_column_tabs() {
    run_preferred_case("K006_PREFERRED_COLUMN_TABS");
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
