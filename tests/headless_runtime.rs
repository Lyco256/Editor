use std::convert::Infallible;

use editor::app::{
    action::Action,
    effect::Effect,
    event::Event,
    runtime::{AppRuntime, EffectDispatcher, QueueActionSource, RecordingDispatcher},
};
use editor_types::TerminalCapabilities;
use terminal_backend::{Framebuffer, TerminalAdapter};

#[derive(Debug, Default)]
struct FakeTerminal {
    entered: bool,
    restored: bool,
    frames: usize,
}

#[derive(Debug, Default)]
struct SaveDispatcher {
    events: Vec<Event>,
}

impl EffectDispatcher for SaveDispatcher {
    fn dispatch(&mut self, effect: Effect) {
        let save_as = matches!(&effect, Effect::SaveDocumentAs { .. });
        if let Effect::SaveDocument { path, text } | Effect::SaveDocumentAs { path, text } = effect
        {
            workspace_core::save_text_document(
                &path,
                &text,
                &workspace_core::DocumentSaveOptions::default(),
            )
            .expect("save effect should write atomically");
            self.events.push(if save_as {
                Event::DocumentSavedAs { path }
            } else {
                Event::DocumentSaved { path }
            });
        } else if let Effect::RefreshExplorer { request, roots } = effect {
            let mut entries = Vec::new();
            for root in roots {
                let canonical = workspace_core::canonicalize_path(&root).expect("root");
                let children =
                    workspace_core::ExplorerTree::new(vec![canonical.clone()], Vec::new())
                        .children(canonical.as_path())
                        .expect("children");
                entries.extend(children.into_iter().map(|entry| {
                    editor::app::event::ExplorerEntryData {
                        path: entry.path,
                        depth: u8::try_from(entry.depth).unwrap_or(u8::MAX),
                    }
                }));
            }
            self.events
                .push(Event::ExplorerUpdated { request, entries });
        }
    }

    fn poll_events(&mut self) -> Vec<Event> {
        std::mem::take(&mut self.events)
    }
}

impl TerminalAdapter for FakeTerminal {
    type Error = Infallible;

    fn capabilities(&self) -> TerminalCapabilities {
        TerminalCapabilities::default()
    }

    fn enter(&mut self) -> Result<(), Self::Error> {
        self.entered = true;
        Ok(())
    }

    fn render(&mut self, _frame: &Framebuffer) -> Result<(), Self::Error> {
        assert!(self.entered);
        assert!(!self.restored);
        self.frames += 1;
        Ok(())
    }

    fn restore(&mut self) -> Result<(), Self::Error> {
        assert!(self.entered);
        self.restored = true;
        Ok(())
    }
}

#[test]
fn starts_renders_and_quits_cleanly_with_fake_adapters() {
    let runtime = AppRuntime::new(
        FakeTerminal::default(),
        QueueActionSource::new([Action::Quit]),
        RecordingDispatcher::default(),
        (80, 24),
    );
    let final_state = runtime.run().expect("headless runtime should be clean");
    assert!(!final_state.running);
    assert!(final_state.frame_number >= 1);
}

#[test]
fn startup_file_is_loaded_without_launching_external_effects() {
    let directory = tempfile::tempdir().expect("temporary directory is available");
    let path = directory.path().join("hello.rs");
    std::fs::write(&path, "fn main() {}\n").expect("fixture is writable");

    let mut state = editor::app::state::AppState::default();
    state.open_startup_path(&path);
    assert_eq!(state.active_path.as_deref(), Some(path.as_path()));
    assert_eq!(state.active_text, "fn main() {}\n");
    assert!(state.output.is_empty());
}

#[test]
fn headless_action_loop_preserves_editing_state_between_frames() {
    use editor_types::{InputEvent, KeyCode, KeyEvent, Modifiers};

    let action = Action::Input(InputEvent::Key(KeyEvent {
        code: KeyCode::Character('x'),
        modifiers: Modifiers::default(),
        repeat: false,
    }));
    let runtime = AppRuntime::new(
        FakeTerminal::default(),
        QueueActionSource::new([
            action,
            Action::Invoke(editor_types::CommandId::new("editor.undo")),
            Action::Quit,
        ]),
        RecordingDispatcher::default(),
        (80, 24),
    );
    let final_state = runtime
        .run()
        .expect("headless editing loop should be clean");
    assert_eq!(final_state.active_text, "");
    assert!(!final_state.active_dirty);
    assert!(!final_state.running);
}

#[test]
fn edit_save_and_reopen_round_trip_through_root_effects() {
    use editor_types::{InputEvent, KeyCode, KeyEvent, Modifiers};

    let directory = tempfile::tempdir().expect("temporary directory is available");
    let path = directory.path().join("roundtrip.txt");
    std::fs::write(&path, "before").expect("fixture is writable");
    let mut state = editor::app::state::AppState::default();
    state.open_startup_path(&path);
    let runtime = AppRuntime::with_state(
        FakeTerminal::default(),
        QueueActionSource::new([
            Action::Input(InputEvent::Key(KeyEvent {
                code: KeyCode::Character('!'),
                modifiers: Modifiers::default(),
                repeat: false,
            })),
            Action::Invoke(editor_types::CommandId::new("editor.save")),
            Action::Quit,
        ]),
        SaveDispatcher::default(),
        (80, 24),
        state,
    );
    let final_state = runtime.run().expect("save runtime should be clean");
    assert!(!final_state.active_dirty);
    assert_eq!(final_state.active_text, "!before");
    let mut reopened = editor::app::state::AppState::default();
    reopened.open_startup_path(&path);
    assert_eq!(reopened.active_text, "!before");
}

#[test]
fn session_snapshot_restores_unsaved_text_after_restart() {
    use editor_types::{InputEvent, KeyCode, KeyEvent, Modifiers};

    let directory = tempfile::tempdir().expect("temporary directory is available");
    let path = directory.path().join("session.txt");
    std::fs::write(&path, "disk").expect("fixture is writable");
    let mut state = editor::app::state::AppState::default();
    state.open_startup_path(&path);
    let _ = state.apply_action(Action::Input(InputEvent::Key(KeyEvent {
        code: KeyCode::Character('!'),
        modifiers: Modifiers::default(),
        repeat: false,
    })));
    let session = state.session_state();
    let mut restored = editor::app::state::AppState::default();
    restored.restore_session(&session);
    assert_eq!(restored.active_text, "!disk");
    assert!(restored.active_dirty);
    assert_eq!(restored.active_path.as_deref(), Some(path.as_path()));
}

#[test]
fn adding_workspace_roots_updates_explorer_projection() {
    let first = tempfile::tempdir().expect("first root");
    let second = tempfile::tempdir().expect("second root");
    std::fs::write(first.path().join("one.txt"), "one").expect("first file");
    std::fs::write(second.path().join("two.txt"), "two").expect("second file");
    let runtime = AppRuntime::new(
        FakeTerminal::default(),
        QueueActionSource::new([
            Action::AddWorkspaceRoot(first.path().to_path_buf()),
            Action::AddWorkspaceRoot(second.path().to_path_buf()),
            Action::Quit,
        ]),
        SaveDispatcher::default(),
        (80, 24),
    );
    let state = runtime.run().expect("workspace runtime");
    assert_eq!(state.workspace_roots.len(), 2);
    assert!(
        state
            .explorer_entries
            .iter()
            .any(|entry| entry.label == "one.txt")
    );
    assert!(
        state
            .explorer_entries
            .iter()
            .any(|entry| entry.label == "two.txt")
    );
}

#[test]
fn multiple_tabs_and_active_tab_survive_session_restore() {
    use editor_types::{InputEvent, KeyCode, KeyEvent, Modifiers};
    let directory = tempfile::tempdir().expect("workspace");
    let first = directory.path().join("first.txt");
    let second = directory.path().join("second.txt");
    std::fs::write(&first, "one").expect("first");
    std::fs::write(&second, "two").expect("second");
    let mut state = editor::app::state::AppState::default();
    state.open_startup_path(&first);
    state.open_tab(&second).expect("open second tab");
    let _ = state.apply_action(Action::Input(InputEvent::Key(KeyEvent {
        code: KeyCode::Character('!'),
        modifiers: Modifiers::default(),
        repeat: false,
    })));
    let session = state.session_state();
    let mut restored = editor::app::state::AppState::default();
    restored.restore_session(&session);
    assert_eq!(restored.tab_count(), 2);
    assert_eq!(restored.active_tab_index(), 1);
    assert_eq!(restored.active_text, "!two");
}

#[test]
fn split_layout_and_multiple_selections_survive_session_restore() {
    use editor_core::{CharacterOffset, Selection, SelectionSet};

    let directory = tempfile::tempdir().expect("workspace");
    let first = directory.path().join("first.txt");
    let second = directory.path().join("second.txt");
    std::fs::write(&first, "one two").expect("first");
    std::fs::write(&second, "three four").expect("second");
    let mut state = editor::app::state::AppState::default();
    state.open_startup_path(&first);
    state.open_tab(&second).expect("open second tab");
    state
        .set_active_selections(
            SelectionSet::new(
                vec![
                    Selection::new(CharacterOffset(0), CharacterOffset(5)),
                    Selection::new(CharacterOffset(6), CharacterOffset(10)),
                ],
                1,
            )
            .expect("valid selections"),
        )
        .expect("set selections");
    let _ = state.apply_action(Action::SplitPane {
        axis: app_ui::shell::SplitAxis::Vertical,
        ratio_percent: 60,
    });

    let session = state.session_state();
    assert!(matches!(
        session.split_layout,
        config_core::SplitLayout::Split { .. }
    ));
    assert_eq!(session.editors[1].selections.len(), 2);

    let mut restored = editor::app::state::AppState::default();
    restored.restore_session(&session);
    let restored_session = restored.session_state();
    assert!(matches!(
        restored_session.split_layout,
        config_core::SplitLayout::Split {
            axis: config_core::SplitAxis::Vertical,
            ..
        }
    ));
    assert_eq!(restored_session.editors[1].selections.len(), 2);
}

#[test]
fn missing_original_file_keeps_recovered_unsaved_text() {
    let missing = std::env::temp_dir().join("editor-recovery-missing-file.txt");
    let session = config_core::SessionState {
        editors: vec![config_core::EditorSession {
            editor_id: "tab-0".to_owned(),
            original_path: Some(missing),
            unsaved_text: Some("recovered".to_owned()),
            dirty: true,
            ..config_core::EditorSession::default()
        }],
        tab_order: vec!["tab-0".to_owned()],
        active_editor: Some("tab-0".to_owned()),
        ..config_core::SessionState::default()
    };
    let mut restored = editor::app::state::AppState::default();
    restored.restore_session(&session);
    assert_eq!(restored.active_text, "recovered");
    assert!(restored.active_dirty);
}

#[test]
fn recovery_checkpoint_is_written_after_an_edit() {
    use editor_types::{InputEvent, KeyCode, KeyEvent, Modifiers};

    let app_data = tempfile::tempdir().expect("application data");
    let store = config_core::RecoveryStore::from_app_data_base(app_data.path(), "Editor")
        .expect("recovery store");
    let input = Action::Input(InputEvent::Key(KeyEvent {
        code: KeyCode::Character('x'),
        modifiers: Modifiers::default(),
        repeat: false,
    }));
    let runtime = AppRuntime::with_recovery_store(
        FakeTerminal::default(),
        QueueActionSource::new([input]),
        RecordingDispatcher::default(),
        (80, 24),
        editor::app::state::AppState::default(),
        store.clone(),
    );
    let final_state = runtime.run().expect("checkpoint runtime");
    assert_eq!(final_state.active_text, "x");
    let loaded = store.load_latest().expect("load checkpoint");
    assert_eq!(
        loaded.session.expect("session").editors[0]
            .unsaved_text
            .as_deref(),
        Some("x")
    );
}

#[test]
fn project_replacement_plan_runs_as_a_background_effect() {
    let directory = tempfile::tempdir().expect("workspace");
    let path = directory.path().join("replace.txt");
    std::fs::write(&path, "old old").expect("seed");
    let plan = workspace_core::ReplacementPlan {
        files: vec![workspace_core::search::FileReplacementPlan {
            path: path.clone(),
            edits: vec![
                workspace_core::ReplacementEdit {
                    range: 4..7,
                    replacement: "new".to_owned(),
                },
                workspace_core::ReplacementEdit {
                    range: 0..3,
                    replacement: "new".to_owned(),
                },
            ],
        }],
    };
    let mut state = editor::app::state::AppState::default();
    let transition = state.apply_action(Action::ApplyReplacementPlan(plan));
    let Effect::ApplyReplacementPlan { request, plan } = transition.effects[0].clone() else {
        panic!("replacement should dispatch a typed effect");
    };
    let report = workspace_core::apply_replacement_plan(&plan).expect("replace");
    state.apply_event(Event::ReplacementApplied { request, report });
    assert_eq!(std::fs::read_to_string(path).expect("read"), "new new");
}

#[test]
fn save_as_retargets_only_after_atomic_write_and_close_protects_dirty_tabs() {
    use editor_types::{InputEvent, KeyCode, KeyEvent, Modifiers};

    let directory = tempfile::tempdir().expect("workspace");
    let target = directory.path().join("saved.txt");
    let input = Action::Input(InputEvent::Key(KeyEvent {
        code: KeyCode::Character('x'),
        modifiers: Modifiers::default(),
        repeat: false,
    }));
    let mut state = editor::app::state::AppState::default();
    let _ = state.apply_action(input.clone());
    let runtime = AppRuntime::with_state(
        FakeTerminal::default(),
        QueueActionSource::new([Action::SaveAs(target.clone()), Action::Quit]),
        SaveDispatcher::default(),
        (80, 24),
        state,
    );
    let final_state = runtime.run().expect("save-as runtime");
    assert!(!final_state.active_dirty);
    assert_eq!(final_state.active_path.as_deref(), Some(target.as_path()));
    assert_eq!(std::fs::read_to_string(target).expect("saved file"), "x");

    let mut dirty = editor::app::state::AppState::default();
    let _ = dirty.apply_action(input);
    let _ = dirty.apply_action(Action::CloseTab(0));
    assert!(dirty.active_dirty);
    assert!(
        dirty
            .output
            .iter()
            .any(|message| message.operation == "close-tab")
    );
}
