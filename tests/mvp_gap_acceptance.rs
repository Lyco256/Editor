//! Dedicated acceptance coverage for the cross-feature MVP contracts.

use app_ui::widgets::{GenericPicker, PickerRow};
use editor::app::{action::Action, state::AppState};
use editor_types::CommandId;

#[test]
fn root_wires_persistent_panes_eol_and_command_registry() {
    let mut state = AppState::default();
    assert_eq!(state.focused_pane().map(|pane| pane.id), Some(0));
    state.apply_action(Action::SplitPane {
        axis: app_ui::shell::SplitAxis::Vertical,
        ratio_percent: 60,
    });
    assert_ne!(
        state.session_state().split_layout,
        config_core::SplitLayout::Empty
    );
    state.apply_action(Action::SetLineEndings(workspace_core::LineEndings::Crlf));
    assert!(state.active_dirty);
    assert!(
        state
            .command_registry()
            .contains(&CommandId::new("editor.setEolCrlf"))
    );
}

#[test]
fn generic_picker_has_typed_ids_and_bounds_safe_navigation() {
    let mut picker = GenericPicker::new(
        "Recent",
        vec![PickerRow::new("one", "One"), PickerRow::new("two", "Two")],
    );
    picker.move_selection(100);
    assert_eq!(picker.accept(), Some("one"));
    picker.set_query("two");
    assert_eq!(picker.accept(), Some("two"));
    picker.move_selection(-100);
    assert_eq!(picker.accept(), Some("two"));
}

#[test]
fn every_available_default_command_has_a_root_route() {
    let commands = AppState::default()
        .command_registry()
        .entries()
        .iter()
        .filter(|entry| entry.available)
        .map(|entry| entry.id.clone())
        .collect::<Vec<_>>();
    for command in commands {
        let mut state = AppState::default();
        state.running = true;
        let _ = state.apply_action(Action::Invoke(command.clone()));
        assert!(
            !state.output.iter().any(|message| {
                message.operation == "command"
                    && message.message.contains("is not available in this build")
            }),
            "unrouted command: {}",
            command.as_str()
        );
    }
}
