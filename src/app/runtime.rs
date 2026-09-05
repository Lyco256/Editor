//! Event-driven runtime with one authoritative state mutation path.

use std::collections::VecDeque;

use app_ui::{
    editor::{EditorStatusData, EditorViewportState, SemanticMarkerSet},
    frame::empty_frame,
    shell::{
        BottomPanelState, ExplorerState, PaneNode, PanelEntry, ShellFocus, ShellState, TabEntry,
    },
};
use editor_core::TextBuffer;
use editor_types::{OutputLevel, StyleRole};
use terminal_backend::{Framebuffer, InputReader, TerminalAdapter};
use thiserror::Error;

use super::{action::Action, effect::Effect, state::AppState};

pub trait ActionSource {
    fn next_action(&mut self) -> Option<Action>;
}

pub trait EffectDispatcher {
    fn dispatch(&mut self, effect: Effect);

    fn poll_events(&mut self) -> Vec<super::event::Event> {
        Vec::new()
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum RuntimeError {
    #[error("could not enter terminal: {0}")]
    Enter(String),
    #[error("could not render terminal frame: {0}")]
    Render(String),
    #[error("could not restore terminal: {0}")]
    Restore(String),
    #[error("could not read terminal input: {0}")]
    Input(String),
    #[error("runtime failed ({runtime}); terminal restore also failed ({restore})")]
    RestoreAfterFailure { runtime: String, restore: String },
}

/// A terminal that owns both presentation and normalized input capabilities.
pub trait InteractiveTerminal: TerminalAdapter + InputReader {}

impl<T> InteractiveTerminal for T where T: TerminalAdapter + InputReader {}

pub struct AppRuntime<T, I, D> {
    terminal: T,
    input: I,
    dispatcher: D,
    state: AppState,
    size: (u16, u16),
    recovery: Option<config_core::RecoveryStore>,
}

impl<T, I, D> AppRuntime<T, I, D>
where
    T: TerminalAdapter,
    I: ActionSource,
    D: EffectDispatcher,
{
    #[must_use]
    pub fn new(terminal: T, input: I, dispatcher: D, size: (u16, u16)) -> Self {
        Self::with_state(terminal, input, dispatcher, size, AppState::default())
    }

    #[must_use]
    pub fn with_state(
        terminal: T,
        input: I,
        dispatcher: D,
        size: (u16, u16),
        state: AppState,
    ) -> Self {
        Self {
            terminal,
            input,
            dispatcher,
            state,
            size,
            recovery: None,
        }
    }

    #[must_use]
    pub fn with_recovery_store(
        terminal: T,
        input: I,
        dispatcher: D,
        size: (u16, u16),
        state: AppState,
        recovery: config_core::RecoveryStore,
    ) -> Self {
        Self {
            terminal,
            input,
            dispatcher,
            state,
            size,
            recovery: Some(recovery),
        }
    }

    /// Runs until input ends or the state accepts a quit action.
    ///
    /// # Errors
    ///
    /// Returns a typed lifecycle error when terminal entry, rendering, or restoration fails.
    pub fn run(mut self) -> Result<AppState, RuntimeError> {
        self.terminal
            .enter()
            .map_err(|error| RuntimeError::Enter(error.to_string()))?;

        let runtime_result = self.run_entered();
        let restore_result = self.terminal.restore().map_err(|error| error.to_string());

        match (runtime_result, restore_result) {
            (Ok(()), Ok(())) => Ok(self.state),
            (Ok(()), Err(restore)) => Err(RuntimeError::Restore(restore)),
            (Err(runtime), Ok(())) => Err(runtime),
            (Err(runtime), Err(restore)) => Err(RuntimeError::RestoreAfterFailure {
                runtime: runtime.to_string(),
                restore,
            }),
        }
    }

    fn run_entered(&mut self) -> Result<(), RuntimeError> {
        self.render()?;
        while self.state.running {
            let Some(action) = self.input.next_action() else {
                break;
            };
            let transition = self.state.apply_action(action);
            for event in transition.events {
                self.state.apply_event(event);
            }
            for effect in transition.effects {
                self.dispatcher.dispatch(effect);
            }
            self.persist_checkpoint();
            for event in self.dispatcher.poll_events() {
                self.state.apply_event(event);
            }
            if transition.render {
                self.render()?;
            }
        }
        Ok(())
    }

    fn render(&mut self) -> Result<(), RuntimeError> {
        let frame = frame_for_state(&self.state, self.size, self.terminal.capabilities());
        self.terminal
            .render(&frame)
            .map_err(|error| RuntimeError::Render(error.to_string()))?;
        self.state.frame_number += 1;
        Ok(())
    }

    fn persist_checkpoint(&mut self) {
        let Some(store) = &self.recovery else {
            return;
        };
        if let Err(error) = store.save(&self.state.session_state()) {
            self.state.output.push(editor_types::OutputMessage {
                subsystem: "recovery".to_owned(),
                operation: "checkpoint".to_owned(),
                level: editor_types::OutputLevel::Warning,
                message: format!("could not persist recovery checkpoint: {error}"),
            });
        }
    }
}

#[allow(clippy::too_many_lines)]
fn frame_for_state(
    state: &AppState,
    size: (u16, u16),
    capabilities: editor_types::TerminalCapabilities,
) -> Framebuffer {
    let mut frame = empty_frame(size.0, size.1);
    let buffer = TextBuffer::new(&state.active_text);
    let file_name = state
        .active_path
        .as_ref()
        .and_then(|path| path.file_name())
        .map_or_else(
            || "Welcome".to_owned(),
            |name| name.to_string_lossy().into_owned(),
        );
    let mut diagnostic_counts = app_ui::editor::DiagnosticCounts::default();
    let diagnostic_markers = state
        .diagnostics
        .iter()
        .map(|diagnostic| {
            let kind = match diagnostic.severity {
                editor_types::DiagnosticSeverity::Error => {
                    diagnostic_counts.error += 1;
                    app_ui::editor::MarkerKind::Error
                }
                editor_types::DiagnosticSeverity::Warning => {
                    diagnostic_counts.warning += 1;
                    app_ui::editor::MarkerKind::Warning
                }
                editor_types::DiagnosticSeverity::Information => {
                    diagnostic_counts.information += 1;
                    app_ui::editor::MarkerKind::Information
                }
                editor_types::DiagnosticSeverity::Hint => {
                    diagnostic_counts.hint += 1;
                    app_ui::editor::MarkerKind::Hint
                }
            };
            app_ui::editor::MarkerSpan {
                range: diagnostic.range,
                kind,
            }
        })
        .collect::<Vec<_>>();
    let status = EditorStatusData {
        file_name: file_name.clone(),
        dirty: state.active_dirty,
        encoding: state
            .tabs
            .get(state.active_tab)
            .map(|tab| tab.encoding.canonical_name().to_ascii_lowercase())
            .unwrap_or_else(|| "utf-8".to_owned()),
        line_ending: state
            .tabs
            .get(state.active_tab)
            .map(|tab| match tab.line_endings {
                workspace_core::LineEndings::None => "none",
                workspace_core::LineEndings::Lf => "lf",
                workspace_core::LineEndings::Crlf => "crlf",
                workspace_core::LineEndings::Mixed => "mixed",
            })
            .unwrap_or("lf")
            .to_owned(),
        branch: state
            .git_status
            .as_ref()
            .and_then(|status| status.branch.clone()),
        language_server: format!("{:?}", state.language_server),
        trust: if state.workspace_trusted {
            "trusted"
        } else {
            "untrusted"
        }
        .to_owned(),
        diagnostics: diagnostic_counts,
        ..EditorStatusData::default()
    };
    let viewport = EditorViewportState {
        title: file_name,
        snapshot: buffer.snapshot(),
        viewport: app_ui::editor::TextViewport::default(),
        selections: buffer.selections().clone(),
        folds: editor_core::FoldSet::default(),
        markers: SemanticMarkerSet {
            language: diagnostic_markers.clone(),
            ..SemanticMarkerSet::default()
        },
        search_matches: Vec::new(),
        bracket_matches: Vec::new(),
        status: status.clone(),
        show_line_numbers: true,
        tab_width: 4,
    };
    let roots = state
        .workspace_roots
        .iter()
        .map(|path| path.display().to_string())
        .collect();
    let entries = state
        .explorer_entries
        .iter()
        .map(|entry| app_ui::shell::ExplorerEntry {
            depth: entry.depth,
            label: entry.label.clone(),
            active: entry.active,
            expanded: entry.expanded,
        })
        .collect();
    let mut output_entries: Vec<PanelEntry> = state
        .output
        .iter()
        .map(|message| PanelEntry {
            label: message.message.clone(),
            detail: Some(message.operation.clone()),
            level: match message.level {
                OutputLevel::Error => StyleRole::Error,
                OutputLevel::Warning => StyleRole::Warning,
                OutputLevel::Information | OutputLevel::Trace => StyleRole::Information,
            },
        })
        .collect();
    output_entries.extend(state.diagnostics.iter().map(|diagnostic| PanelEntry {
        label: diagnostic.message.clone(),
        detail: Some(format!(
            "{}:{}",
            diagnostic.range.start.0,
            match diagnostic.severity {
                editor_types::DiagnosticSeverity::Error => "error",
                editor_types::DiagnosticSeverity::Warning => "warning",
                editor_types::DiagnosticSeverity::Information => "info",
                editor_types::DiagnosticSeverity::Hint => "hint",
            }
        )),
        level: match diagnostic.severity {
            editor_types::DiagnosticSeverity::Error => StyleRole::Error,
            editor_types::DiagnosticSeverity::Warning => StyleRole::Warning,
            editor_types::DiagnosticSeverity::Information => StyleRole::Information,
            editor_types::DiagnosticSeverity::Hint => StyleRole::Hint,
        },
    }));
    let second_viewport = state
        .split_secondary_tab
        .and_then(|index| state.tabs.get(index))
        .map(|tab| {
            let title = tab
                .path
                .as_ref()
                .and_then(|path| path.file_name())
                .map_or_else(
                    || "Welcome".to_owned(),
                    |name| name.to_string_lossy().into_owned(),
                );
            EditorViewportState {
                title,
                snapshot: tab.buffer.snapshot(),
                viewport: app_ui::editor::TextViewport::default(),
                selections: tab.buffer.selections().clone(),
                folds: editor_core::FoldSet::default(),
                markers: SemanticMarkerSet {
                    language: diagnostic_markers,
                    ..SemanticMarkerSet::default()
                },
                search_matches: Vec::new(),
                bracket_matches: Vec::new(),
                status: status.clone(),
                show_line_numbers: true,
                tab_width: 4,
            }
        });
    let root = match (state.split_axis, second_viewport) {
        (Some(axis), Some(second)) => PaneNode::split(
            axis,
            state.split_ratio_percent,
            PaneNode::leaf(viewport),
            PaneNode::leaf(second),
        ),
        _ => PaneNode::leaf(viewport),
    };
    let shell = ShellState {
        explorer: ExplorerState {
            roots,
            entries,
            visible: state.explorer_visible,
        },
        tabs: state
            .tabs
            .iter()
            .enumerate()
            .map(|(index, tab)| TabEntry {
                title: tab
                    .path
                    .as_ref()
                    .and_then(|path| path.file_name())
                    .map_or_else(
                        || "Welcome".to_owned(),
                        |name| name.to_string_lossy().into_owned(),
                    ),
                dirty: tab.buffer.is_dirty(),
                active: index == state.active_tab,
                closeable: true,
            })
            .collect(),
        root,
        bottom: BottomPanelState {
            title: if state.diagnostics.is_empty() {
                "Output".to_owned()
            } else {
                "Problems".to_owned()
            },
            entries: output_entries,
            visible: state.bottom_panel_visible || !state.output.is_empty(),
        },
        palette: state.palette.clone(),
        focus: if state.palette_visible {
            ShellFocus::CommandPalette
        } else {
            ShellFocus::Editor
        },
        status,
    };
    shell.render(
        &mut frame,
        app_ui::widgets::Rect::new(0, 0, size.0, size.1),
        capabilities,
    );
    frame
}

/// Runs the production event loop with one terminal adapter for rendering and input.
///
/// The loop blocks in the terminal adapter when idle; it does not poll at a fixed cadence.
///
/// # Errors
///
/// Returns a typed lifecycle error when entering, reading, rendering, or restoring the terminal
/// fails.
pub fn run_interactive<T, D>(
    terminal: T,
    dispatcher: D,
    size: (u16, u16),
) -> Result<AppState, RuntimeError>
where
    T: InteractiveTerminal,
    D: EffectDispatcher,
{
    run_interactive_with_state(terminal, dispatcher, size, AppState::default())
}

/// Interactive runtime variant used by startup paths that have already loaded a document or
/// workspace model.
///
/// # Errors
///
/// Returns a typed lifecycle error when entering, reading, rendering, or restoring the terminal
/// fails.
pub fn run_interactive_with_state<T, D>(
    terminal: T,
    dispatcher: D,
    size: (u16, u16),
    state: AppState,
) -> Result<AppState, RuntimeError>
where
    T: InteractiveTerminal,
    D: EffectDispatcher,
{
    run_interactive_with_state_and_recovery(terminal, dispatcher, size, state, None)
}

/// Interactive runtime variant with optional crash-recovery checkpoints after each action.
///
/// # Errors
///
/// Returns a typed lifecycle error when entering, reading, rendering, or restoring the terminal
/// fails.
pub fn run_interactive_with_state_and_recovery<T, D>(
    mut terminal: T,
    dispatcher: D,
    size: (u16, u16),
    state: AppState,
    recovery: Option<config_core::RecoveryStore>,
) -> Result<AppState, RuntimeError>
where
    T: InteractiveTerminal,
    D: EffectDispatcher,
{
    terminal
        .enter()
        .map_err(|error| RuntimeError::Enter(error.to_string()))?;
    let mut runtime = match recovery {
        Some(store) => AppRuntime::with_recovery_store(
            terminal,
            InteractiveActionSource,
            dispatcher,
            size,
            state,
            store,
        ),
        None => AppRuntime::with_state(terminal, InteractiveActionSource, dispatcher, size, state),
    };
    let runtime_result = runtime.run_entered_interactive();
    let restore_result = runtime
        .terminal
        .restore()
        .map_err(|error| error.to_string());
    match (runtime_result, restore_result) {
        (Ok(()), Ok(())) => Ok(runtime.state),
        (Ok(()), Err(restore)) => Err(RuntimeError::Restore(restore)),
        (Err(runtime), Ok(())) => Err(runtime),
        (Err(runtime), Err(restore)) => Err(RuntimeError::RestoreAfterFailure {
            runtime: runtime.to_string(),
            restore,
        }),
    }
}

struct InteractiveActionSource;

impl ActionSource for InteractiveActionSource {
    fn next_action(&mut self) -> Option<Action> {
        None
    }
}

impl<T, D> AppRuntime<T, InteractiveActionSource, D>
where
    T: InteractiveTerminal,
    D: EffectDispatcher,
{
    fn run_entered_interactive(&mut self) -> Result<(), RuntimeError> {
        self.render()?;
        while self.state.running {
            let input = self
                .terminal
                .read_input()
                .map_err(|error| RuntimeError::Input(error.to_string()))?;
            let transition = self.state.apply_action(Action::Input(input));
            for event in transition.events {
                self.state.apply_event(event);
            }
            for effect in transition.effects {
                self.dispatcher.dispatch(effect);
            }
            self.persist_checkpoint();
            for event in self.dispatcher.poll_events() {
                self.state.apply_event(event);
            }
            if transition.render {
                self.render()?;
            }
        }
        Ok(())
    }
}

#[derive(Debug, Default)]
pub struct QueueActionSource {
    actions: VecDeque<Action>,
}

impl QueueActionSource {
    #[must_use]
    pub fn new(actions: impl IntoIterator<Item = Action>) -> Self {
        Self {
            actions: actions.into_iter().collect(),
        }
    }
}

impl ActionSource for QueueActionSource {
    fn next_action(&mut self) -> Option<Action> {
        self.actions.pop_front()
    }
}

#[derive(Debug, Default)]
pub struct RecordingDispatcher {
    pub effects: Vec<Effect>,
}

impl EffectDispatcher for RecordingDispatcher {
    fn dispatch(&mut self, effect: Effect) {
        self.effects.push(effect);
    }
}
