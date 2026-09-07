//! Shared deterministic helpers for the frozen interaction oracle.

use editor::app::{action::Action, state::AppState};
use editor_core::{CharacterOffset, Selection, SelectionSet, TextBuffer};
use editor_types::{InputEvent, KeyCode, KeyEvent, Modifiers};

pub fn key(code: KeyCode) -> InputEvent {
    InputEvent::Key(KeyEvent {
        code,
        modifiers: Modifiers::default(),
        repeat: false,
    })
}

pub fn key_with(code: KeyCode, modifiers: Modifiers) -> InputEvent {
    InputEvent::Key(KeyEvent {
        code,
        modifiers,
        repeat: false,
    })
}

pub fn state_with_text(text: &str) -> AppState {
    let mut state = AppState::default();
    state.active_text = text.to_owned();
    state.open_startup_path(std::path::Path::new("requirements-001-fixture.txt"));
    // open_startup_path intentionally reports a missing file; install the same text through the
    // public editor action path so the oracle never depends on private root fields.
    state.active_text.clear();
    state.apply_action(Action::Input(InputEvent::Paste(text.to_owned())));
    state
}

pub fn buffer_with_text(text: &str) -> TextBuffer {
    TextBuffer::new(text)
}

pub fn cursor(buffer: &mut TextBuffer, offset: usize) {
    buffer
        .set_selections(SelectionSet::single(Selection::cursor(CharacterOffset(
            offset,
        ))))
        .expect("fixture cursor is valid");
}

pub fn source(path: &str) -> String {
    std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(path))
        .expect("source fixture must be readable")
}
