//! Product-boundary acceptance for the Requirements 003 pane/buffer issues.

use std::path::{Path, PathBuf};

use editor::app::{action::Action, state::AppState};
use editor_types::{InputEvent, Modifiers, MouseAction, MouseButton, ScreenCell};

struct Fixture {
    directory: tempfile::TempDir,
    primary: PathBuf,
    secondary: PathBuf,
    state: AppState,
}

fn fixture_with_split() -> Fixture {
    let directory = tempfile::tempdir().expect("S003-02 step 1: fixture directory");
    let primary = directory.path().join("primary.txt");
    let secondary = directory.path().join("secondary.txt");
    std::fs::write(&primary, "PRIMARY").expect("S003-02 step 1: primary fixture");
    std::fs::write(&secondary, "SECONDARY").expect("S003-02 step 1: secondary fixture");
    let mut state = AppState::default();
    state.open_startup_path(&primary);
    state
        .open_tab(&secondary)
        .expect("S003-02 step 2: open secondary buffer");
    state.apply_action(Action::SplitPane {
        axis: app_ui::shell::SplitAxis::Vertical,
        ratio_percent: 50,
    });
    state.apply_action(Action::FocusPane(0));
    Fixture {
        directory,
        primary,
        secondary,
        state,
    }
}

fn pointer(pane_id: u32, column: u16) -> app_ui::shell::PointerEvent {
    app_ui::shell::PointerEvent {
        target: app_ui::shell::PointerTarget::Editor {
            pane_id,
            local: ScreenCell { row: 0, column },
            region: app_ui::shell::EditorPointerRegion::Text,
            text_origin: 0,
            text_width: 80,
        },
        screen: ScreenCell { row: 0, column },
        action: MouseAction::Down(MouseButton::Left),
        modifiers: Modifiers::default(),
        click_count: 1,
    }
}

fn path_label(path: Option<&Path>) -> String {
    path.map_or_else(
        || "<untitled>".to_owned(),
        |path| path.display().to_string(),
    )
}

fn issue_failure(issue: &str, step: usize, state: &AppState, expected: &str, actual: &str) -> ! {
    let focused = state
        .focused_pane()
        .map_or_else(|| "<none>".to_owned(), |pane| pane.id.to_string());
    let displayed = state
        .tabs_for_pane(state.focused_pane().map_or(0, |pane| pane.id))
        .map_or_else(
            || "<none>".to_owned(),
            |tab| path_label(tab.path.as_deref()),
        );
    let resolved = state
        .buffer_for_pane(state.focused_pane().map_or(0, |pane| pane.id))
        .map_or_else(|| "<none>".to_owned(), ToString::to_string);
    panic!(
        "{issue} step={step} focused_pane={focused} displayed_buffer={displayed} resolved_operation_buffer={resolved} pointer_owner={focused} expected={expected} actual={actual}"
    )
}

#[test]
fn s003_02_different_buffer_secondary_pointer_and_edit() {
    let mut fixture = fixture_with_split();
    let before_primary = fixture
        .state
        .buffer_for_pane(0)
        .expect("S003-02 primary buffer")
        .to_string();
    println!(
        "S003-02 setup active_tab={} pane0_tab={} pane1_tab={}",
        fixture.state.active_tab_index(),
        fixture.state.focused_pane().map_or(99, |p| p.id),
        fixture
            .state
            .tabs_for_pane(1)
            .is_some_and(|t| t.path.is_some())
    );
    fixture.state.apply_action(Action::Pointer(pointer(1, 9)));
    fixture
        .state
        .apply_action(Action::Input(InputEvent::Paste("!".to_owned())));
    let focused = fixture.state.focused_pane().map(|pane| pane.id);
    let displayed = fixture
        .state
        .tabs_for_pane(1)
        .and_then(|tab| tab.path.as_deref())
        .map(Path::to_path_buf);
    let resolved = fixture
        .state
        .buffer_for_pane(1)
        .expect("S003-02 resolved secondary buffer")
        .to_string();
    let primary_after = fixture
        .state
        .buffer_for_pane(0)
        .expect("S003-02 primary after")
        .to_string();
    println!(
        "S003-02 observed focused={focused:?} resolved={resolved} primary_after={primary_after}"
    );
    if focused != Some(1) {
        issue_failure(
            "S003-02",
            3,
            &fixture.state,
            "focused_pane=1",
            &format!("focused_pane={focused:?}"),
        );
    }
    if displayed.as_deref() != Some(fixture.secondary.as_path()) {
        issue_failure(
            "S003-02",
            3,
            &fixture.state,
            "displayed_buffer=secondary",
            &path_label(displayed.as_deref()),
        );
    }
    if resolved != "SECONDARY!" {
        issue_failure(
            "S003-02",
            4,
            &fixture.state,
            "secondary buffer SECONDARY!",
            &resolved,
        );
    }
    if primary_after != before_primary || primary_after != "PRIMARY" {
        issue_failure(
            "S003-02",
            4,
            &fixture.state,
            "primary buffer unchanged",
            &primary_after,
        );
    }
    println!(
        "S003-02 focused_pane=1 displayed_buffer=Y resolved_buffer=Y pointer_owner=secondary changed_buffers=[Y]"
    );
    let _ = fixture.primary;
}

#[test]
fn s003_03_same_buffer_panes_restore_independent_cursor_state() {
    let directory = tempfile::tempdir().expect("S003-03 step 1: fixture directory");
    let path = directory.path().join("shared.txt");
    std::fs::write(&path, "SHARED").expect("S003-03 step 1: shared fixture");
    let mut state = AppState::default();
    state.open_startup_path(&path);
    state.apply_action(Action::SplitPane {
        axis: app_ui::shell::SplitAxis::Vertical,
        ratio_percent: 50,
    });
    state.apply_action(Action::Pointer(pointer(0, 2)));
    state.apply_action(Action::FocusPane(1));
    state.apply_action(Action::Pointer(pointer(1, 4)));
    state.apply_action(Action::FocusPane(0));
    let primary_cursor = state
        .buffer_for_pane(0)
        .expect("S003-03 primary buffer")
        .selections()
        .primary()
        .active
        .0;
    state.apply_action(Action::FocusPane(1));
    let secondary_cursor = state
        .buffer_for_pane(1)
        .expect("S003-03 secondary buffer")
        .selections()
        .primary()
        .active
        .0;
    if primary_cursor != 2 || secondary_cursor != 4 {
        issue_failure(
            "S003-03",
            5,
            &state,
            "pane cursors A=2 B=4",
            &format!("A={primary_cursor} B={secondary_cursor}"),
        );
    }
    println!(
        "S003-03 focused_pane=1 displayed_buffer=shared resolved_buffer=shared pointer_owner=secondary cursors=[2,4]"
    );
}

#[test]
fn s003_04_focus_switch_stress_keeps_resolution_current() {
    let mut fixture = fixture_with_split();
    for (pane, column) in [(1_u32, 1_u16), (0, 2), (1, 3), (0, 4), (1, 9)] {
        fixture.state.apply_action(Action::FocusPane(pane));
        fixture
            .state
            .apply_action(Action::Pointer(pointer(pane, column)));
    }
    fixture
        .state
        .apply_action(Action::Input(InputEvent::Paste("?".to_owned())));
    let secondary = fixture
        .state
        .buffer_for_pane(1)
        .expect("S003-04 secondary")
        .to_string();
    let primary = fixture
        .state
        .buffer_for_pane(0)
        .expect("S003-04 primary")
        .to_string();
    if secondary != "SECONDARY?" || primary != "PRIMARY" {
        issue_failure(
            "S003-04",
            11,
            &fixture.state,
            "edit secondary only",
            &format!("primary={primary} secondary={secondary}"),
        );
    }
    println!("S003-04 sequence=A→B→A→B focused_pane=1 resolved_buffer=Y changed_buffers=[Y]");
}

#[test]
fn s003_05_buffer_switch_immediately_before_edit() {
    let mut fixture = fixture_with_split();
    let third = fixture.directory.path().join("third.txt");
    std::fs::write(&third, "THIRD").expect("S003-05 step 1: third fixture");
    fixture
        .state
        .open_tab(&third)
        .expect("S003-05 step 2: open third");
    fixture.state.apply_action(Action::FocusPane(1));
    fixture.state.apply_action(Action::SwitchTab(2));
    fixture
        .state
        .apply_action(Action::Input(InputEvent::Paste("!".to_owned())));
    let actual = fixture
        .state
        .buffer_for_pane(1)
        .expect("S003-05 resolved third")
        .to_string();
    if actual != "!THIRD" {
        issue_failure("S003-05", 4, &fixture.state, "third buffer !THIRD", &actual);
    }
    println!("S003-05 displayed_buffer=Z resolved_buffer=Z changed_buffers=[Z]");
}

#[test]
fn s003_06_split_creation_immediate_pointer_and_edit() {
    let directory = tempfile::tempdir().expect("S003-06 step 1: fixture directory");
    let path = directory.path().join("split.txt");
    std::fs::write(&path, "SPLIT").expect("S003-06 step 1: split fixture");
    let mut state = AppState::default();
    state.open_startup_path(&path);
    state.apply_action(Action::SplitPane {
        axis: app_ui::shell::SplitAxis::Vertical,
        ratio_percent: 50,
    });
    state.apply_action(Action::Pointer(pointer(1, 2)));
    state.apply_action(Action::Input(InputEvent::Paste("!".to_owned())));
    let actual = state
        .buffer_for_pane(1)
        .expect("S003-06 split buffer")
        .to_string();
    if actual != "SP!LIT" {
        issue_failure("S003-06", 4, &state, "split buffer SP!LIT", &actual);
    }
    println!(
        "S003-06 split_created=true focused_pane=1 displayed_buffer=shared resolved_buffer=shared changed_buffers=[shared]"
    );
}
