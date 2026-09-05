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
        for effect in self.state.take_deferred_effects() {
            self.dispatcher.dispatch(effect);
        }
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
            for effect in self.state.take_deferred_effects() {
                self.dispatcher.dispatch(effect);
            }
            self.persist_checkpoint();
            for event in self.dispatcher.poll_events() {
                self.state.apply_event(event);
                for effect in self.state.take_deferred_effects() {
                    self.dispatcher.dispatch(effect);
                }
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
    let syntax_error_markers = state
        .syntax_snapshot
        .error_regions
        .iter()
        .copied()
        .map(|range| app_ui::editor::MarkerSpan {
            range,
            kind: app_ui::editor::MarkerKind::Error,
        })
        .collect::<Vec<_>>();
    let mut language_markers = diagnostic_markers.clone();
    language_markers.extend(syntax_error_markers);
    let mut folds = editor_core::FoldSet::default();
    let fold_regions = state
        .syntax_snapshot
        .folds
        .iter()
        .filter_map(|range| {
            let start = buffer.offset_to_position(range.start).ok()?.line;
            let end = buffer.offset_to_position(range.end).ok()?.line;
            editor_core::FoldRegion::new(start, end).ok()
        })
        .collect::<Vec<_>>();
    folds.set_regions(fold_regions);
    let mut syntax_spans = state
        .language_ui
        .semantic
        .current()
        .map(|span_set| {
            span_set
                .spans
                .iter()
                .map(|span| {
                    let role = match span.style {
                        app_ui::language::TokenStyle::SemanticType
                        | app_ui::language::TokenStyle::SemanticFunction
                        | app_ui::language::TokenStyle::SemanticVariable => StyleRole::SemanticType,
                        app_ui::language::TokenStyle::SyntaxKeyword => StyleRole::SyntaxKeyword,
                        app_ui::language::TokenStyle::SyntaxString => StyleRole::SyntaxString,
                        app_ui::language::TokenStyle::SyntaxComment => StyleRole::SyntaxComment,
                        app_ui::language::TokenStyle::Plain => StyleRole::EditorText,
                    };
                    (span.range, role)
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    syntax_spans.extend(
        state
            .syntax_snapshot
            .highlights
            .iter()
            .map(|span| (span.range, span.role))
            .collect::<Vec<_>>(),
    );
    let bracket_matches = state
        .syntax_snapshot
        .pairs
        .iter()
        .flat_map(|pair| [pair.open, pair.close])
        .collect::<Vec<_>>();
    let search_matches = state
        .workspace_ui
        .search
        .results
        .iter()
        .filter(|result| state.active_path.as_ref() == Some(&result.path))
        .filter_map(|result| {
            let character = result
                .line_text
                .find(&result.matched_text)
                .map(|byte| result.line_text[..byte].chars().count())?;
            let start = buffer
                .position_to_offset(editor_types::LogicalPosition {
                    line: u32::try_from(result.line_number.saturating_sub(1)).ok()?,
                    character: u32::try_from(character).ok()?,
                })
                .ok()?;
            let end = editor_core::CharacterOffset(
                start.0.saturating_add(result.matched_text.chars().count()),
            );
            Some(editor_core::TextRange { start, end })
        })
        .collect::<Vec<_>>();
    let status = EditorStatusData {
        file_name: file_name.clone(),
        dirty: state.active_dirty,
        encoding: state.tabs.get(state.active_tab).map_or_else(
            || "utf-8".to_owned(),
            |tab| tab.encoding.canonical_name().to_ascii_lowercase(),
        ),
        line_ending: state
            .tabs
            .get(state.active_tab)
            .map_or("lf", |tab| match tab.line_endings {
                workspace_core::LineEndings::None => "none",
                workspace_core::LineEndings::Lf => "lf",
                workspace_core::LineEndings::Crlf => "crlf",
                workspace_core::LineEndings::Mixed => "mixed",
            })
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
        indent_style: if state.insert_spaces {
            String::from("spaces")
        } else {
            String::from("tabs")
        },
        indent_size: u8::try_from(state.tab_width).unwrap_or(u8::MAX),
        ..EditorStatusData::default()
    };
    let viewport = EditorViewportState {
        title: file_name,
        snapshot: buffer.snapshot(),
        viewport: app_ui::editor::TextViewport::default(),
        selections: buffer.selections().clone(),
        folds: folds.clone(),
        markers: SemanticMarkerSet {
            language: language_markers.clone(),
            ..SemanticMarkerSet::default()
        },
        syntax_spans,
        search_matches,
        bracket_matches,
        status: status.clone(),
        show_line_numbers: state.show_line_numbers,
        tab_width: state.tab_width,
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
    if let Some(prompt) = &state.workspace_ui.prompt {
        let label = match prompt {
            app_ui::workspace::WorkspacePrompt::CreateFile { path } => {
                format!("Create file? {}", path.display())
            }
            app_ui::workspace::WorkspacePrompt::Rename { source, target } => {
                format!("Rename? {} -> {}", source.display(), target.display())
            }
            app_ui::workspace::WorkspacePrompt::Move { source, target } => {
                format!("Move? {} -> {}", source.display(), target.display())
            }
            app_ui::workspace::WorkspacePrompt::Delete {
                path,
                recursive,
                byte_len,
            } => format!(
                "Delete? {}{} ({} byte(s))",
                path.display(),
                if *recursive { " recursively" } else { "" },
                byte_len
            ),
        };
        output_entries.push(PanelEntry {
            label,
            detail: Some("Confirm or Cancel File Operation from Command Palette".to_owned()),
            level: StyleRole::Warning,
        });
    }
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
                folds: folds.clone(),
                markers: SemanticMarkerSet {
                    language: language_markers,
                    ..SemanticMarkerSet::default()
                },
                syntax_spans: Vec::new(),
                search_matches: Vec::new(),
                bracket_matches: Vec::new(),
                status: status.clone(),
                show_line_numbers: state.show_line_numbers,
                tab_width: state.tab_width,
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
            title: match state.bottom_panel_view {
                super::state::BottomPanelView::Output => "Output",
                super::state::BottomPanelView::Problems => "Problems",
                super::state::BottomPanelView::Search => "Search",
                super::state::BottomPanelView::Git => "Source Control",
                super::state::BottomPanelView::Language => "Language",
            }
            .to_owned(),
            entries: output_entries,
            visible: state.bottom_panel_visible
                || !state.output.is_empty()
                || !matches!(
                    state.bottom_panel_view,
                    super::state::BottomPanelView::Output
                ),
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
    let layout = shell.layout(app_ui::widgets::Rect::new(0, 0, size.0, size.1));
    if let Some(bottom) = layout.bottom {
        let panel = match state.bottom_panel_view {
            super::state::BottomPanelView::Output => None,
            super::state::BottomPanelView::Problems => {
                state.language_ui.diagnostics.current().map(|dashboard| {
                    app_ui::language::render_diagnostic_dashboard(
                        dashboard,
                        bottom.width,
                        bottom.height,
                    )
                })
            }
            super::state::BottomPanelView::Search => {
                let mut panel = Framebuffer::new(bottom.width, bottom.height);
                app_ui::workspace::draw_search(&mut panel, &state.workspace_ui.search);
                Some(panel)
            }
            super::state::BottomPanelView::Git => state.git_dashboard.as_ref().map(|dashboard| {
                let mut panel = Framebuffer::new(bottom.width, bottom.height);
                dashboard.render(
                    &mut panel,
                    app_ui::git::Rect::new(0, 0, bottom.width, bottom.height),
                    capabilities,
                );
                panel
            }),
            super::state::BottomPanelView::Language => language_panel_for_state(state, bottom),
        };
        if let Some(panel) = panel {
            blit_frame(&mut frame, &panel, bottom.x, bottom.y);
        }
    }
    if let Some(input) = input_overlay_for_state(state, size) {
        blit_frame(&mut frame, &input, 2, 1);
    }
    frame
}

fn input_overlay_for_state(state: &AppState, size: (u16, u16)) -> Option<Framebuffer> {
    let mode = state.input_mode.as_ref()?;
    let width = size.0.saturating_sub(4).clamp(1, 100);
    let height = match mode {
        super::state::InputMode::QuickOpen => size.1.saturating_sub(2).clamp(1, 16),
        _ => 3.min(size.1.saturating_sub(1).max(1)),
    };
    let mut overlay = Framebuffer::new(width, height);
    if matches!(mode, super::state::InputMode::QuickOpen) {
        app_ui::workspace::draw_quick_open(&mut overlay, &state.workspace_ui.quick_open);
        return Some(overlay);
    }
    let label = match mode {
        super::state::InputMode::ProjectSearch => "Search workspace",
        super::state::InputMode::Find => "Find",
        super::state::InputMode::ReplaceQuery => "Replace query",
        super::state::InputMode::ReplaceReplacement { .. } => "Replace with",
        super::state::InputMode::QuickOpen => "Quick Open",
    };
    write_overlay_line(
        &mut overlay,
        0,
        &format!("{label}: {}", state.input_buffer),
        StyleRole::StatusBar,
    );
    write_overlay_line(
        &mut overlay,
        1,
        "Enter=apply  Esc=cancel  Backspace=delete",
        StyleRole::Information,
    );
    Some(overlay)
}

fn write_overlay_line(frame: &mut Framebuffer, row: u16, text: &str, foreground: StyleRole) {
    let (columns, rows) = frame.size();
    if row >= rows {
        return;
    }
    for (column, character) in text.chars().enumerate() {
        let Ok(column) = u16::try_from(column) else {
            break;
        };
        if column >= columns {
            break;
        }
        let _ = frame.set(
            column,
            row,
            terminal_backend::Cell {
                symbol: character.to_string(),
                foreground,
                background: StyleRole::Panel,
                ..terminal_backend::Cell::default()
            },
        );
    }
}

fn language_panel_for_state(state: &AppState, area: app_ui::widgets::Rect) -> Option<Framebuffer> {
    let width = area.width;
    let height = area.height;
    match state.last_language_method.as_deref()? {
        "textDocument/completion" => state
            .language_ui
            .completion
            .current()
            .map(|view| app_ui::language::render_completion_view(view, width, height)),
        "textDocument/hover" => state
            .language_ui
            .hover
            .current()
            .map(|view| app_ui::language::render_hover_view(view, width, height)),
        "textDocument/signatureHelp" => state
            .language_ui
            .signature
            .current()
            .map(|view| app_ui::language::render_signature_view(view, width, height)),
        "textDocument/definition" | "textDocument/declaration" | "textDocument/implementation" => {
            state
                .language_ui
                .go_to
                .current()
                .map(|view| app_ui::language::render_location_chooser(view, width, height))
        }
        "textDocument/references" => state
            .language_ui
            .references
            .current()
            .map(|view| app_ui::language::render_location_chooser(view, width, height)),
        "textDocument/rename" => state
            .language_ui
            .rename
            .current()
            .map(|view| app_ui::language::render_rename_preview(view, width, height)),
        "textDocument/codeAction" => state
            .language_ui
            .code_actions
            .current()
            .map(|view| app_ui::language::render_code_actions(view, width, height)),
        "textDocument/inlayHint" => state
            .language_ui
            .inlay_hints
            .current()
            .map(|view| app_ui::language::render_inlay_hints(view, width, height)),
        "textDocument/documentSymbol" | "workspace/symbol" => state
            .language_ui
            .symbols
            .current()
            .map(|view| app_ui::language::render_symbols(view, width, height)),
        "textDocument/formatting" | "textDocument/rangeFormatting" => state
            .language_ui
            .formatting
            .current()
            .map(|view| app_ui::language::render_formatting_feedback(view, width, height)),
        _ => None,
    }
}

fn blit_frame(destination: &mut Framebuffer, source: &Framebuffer, x: u16, y: u16) {
    let (width, height) = source.size();
    let (destination_width, destination_height) = destination.size();
    for row in 0..height {
        for column in 0..width {
            let Some(cell) = source.get(column, row).ok().cloned() else {
                continue;
            };
            let target_x = x.saturating_add(column);
            let target_y = y.saturating_add(row);
            if target_x < destination_width && target_y < destination_height {
                let _ = destination.set(target_x, target_y, cell);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::frame_for_state;
    use crate::app::state::{AppState, InputMode};
    use editor_types::{DocumentId, StyleRole, TextRange};

    #[test]
    fn root_frame_projects_syntax_roles_and_folds() {
        let mut state = AppState::default();
        state.active_text = String::from("fn main() {\n  1\n}");
        state.syntax_snapshot = syntax_engine::SyntaxSnapshot {
            ticket: Some(syntax_engine::ParseTicket {
                document: DocumentId(1),
                version: 0,
            }),
            highlights: vec![syntax_engine::SyntaxSpan {
                range: TextRange {
                    start: editor_types::CharacterOffset(3),
                    end: editor_types::CharacterOffset(7),
                },
                role: StyleRole::SyntaxKeyword,
            }],
            ..syntax_engine::SyntaxSnapshot::default()
        };
        let frame = frame_for_state(
            &state,
            (80, 24),
            editor_types::TerminalCapabilities::default(),
        );
        let cell = frame.get(31, 1).expect("syntax editor cell");
        assert_eq!(cell.foreground, StyleRole::SyntaxKeyword);
    }

    #[test]
    fn root_frame_renders_interactive_language_result_panel() {
        let mut state = AppState::default();
        let version = state.buffer.snapshot().version();
        state.apply_event(crate::app::event::Event::LspResponse {
            request: editor_types::RequestId(12),
            version,
            method: String::from("textDocument/completion"),
            result: serde_json::json!({"items": [{"label": "println!"}]}),
        });
        let frame = frame_for_state(
            &state,
            (100, 30),
            editor_types::TerminalCapabilities::default(),
        );
        let snapshot = app_ui::language::dump_framebuffer(&frame);
        assert!(snapshot.contains("C|o|m|p|l|e|t|i|o|n"));
        assert!(snapshot.contains("p|r|i|n|t|l|n|!"));
    }

    #[test]
    fn root_frame_renders_keyboard_input_overlay() {
        let mut state = AppState::default();
        state.input_mode = Some(InputMode::Find);
        state.input_buffer = "needle".to_owned();
        let frame = frame_for_state(
            &state,
            (80, 24),
            editor_types::TerminalCapabilities::default(),
        );
        let snapshot = app_ui::language::dump_framebuffer(&frame);
        assert!(snapshot.contains("F|i|n|d|:| |n|e|e|d|l|e"));
        assert!(snapshot.contains("E|n|t|e|r|=|a|p|p|l|y"));
    }
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
                for effect in self.state.take_deferred_effects() {
                    self.dispatcher.dispatch(effect);
                }
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
