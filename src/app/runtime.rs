//! Event-driven runtime with one authoritative state mutation path.

use std::collections::VecDeque;

use app_ui::{
    editor::{EditorStatusData, EditorViewportState, SemanticMarkerSet},
    frame::empty_frame,
    shell::{
        BottomPanelState, ExplorerState, PaneNode, PanelEntry, ShellFocus, ShellState, TabEntry,
    },
};
use editor_types::{InputEvent, OutputLevel, StyleRole};
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

#[derive(Debug, Clone)]
struct RenderScene {
    frame: Framebuffer,
    layout: app_ui::shell::WorkbenchLayoutSnapshot,
    cursor: Option<(u16, u16)>,
}

pub struct AppRuntime<T, I, D> {
    terminal: T,
    input: I,
    dispatcher: D,
    state: AppState,
    size: (u16, u16),
    layout_snapshot: Option<app_ui::shell::WorkbenchLayoutSnapshot>,
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
            layout_snapshot: None,
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
            layout_snapshot: None,
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
            if let Action::Input(InputEvent::Resize { columns, rows }) = action {
                if columns > 0 && rows > 0 {
                    self.size = (columns, rows);
                    self.render()?;
                }
                continue;
            }
            let action = self.translate_pointer_action(action);
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
        if self.size.0 == 0 || self.size.1 == 0 {
            return Ok(());
        }
        let scene = scene_for_state(&self.state, self.size, self.terminal.capabilities());
        self.layout_snapshot = Some(scene.layout.clone());
        self.terminal
            .render(&scene.frame)
            .map_err(|error| RuntimeError::Render(error.to_string()))?;
        self.terminal
            .present_cursor(scene.cursor)
            .map_err(|error| RuntimeError::Render(error.to_string()))?;
        if let Some(group) = scene
            .layout
            .editor_groups
            .iter()
            .find(|group| group.group_id == self.state.focused_pane().map_or(0, |pane| pane.id))
        {
            self.state
                .set_page_lines(group.content.height.saturating_sub(1).into());
            self.state.set_text_width(group.text.width);
        }
        self.state.frame_number += 1;
        Ok(())
    }

    fn translate_pointer_action(&self, action: Action) -> Action {
        let Action::Input(InputEvent::Mouse(mouse)) = action else {
            return action;
        };
        let Some(layout) = self.layout_snapshot.as_ref() else {
            return Action::Input(InputEvent::Mouse(mouse));
        };
        if let Some(target) = layout.pointer_target(mouse.position.column, mouse.position.row) {
            return Action::Pointer(app_ui::shell::PointerEvent {
                target,
                screen: mouse.position,
                action: mouse.action,
                modifiers: mouse.modifiers,
                click_count: mouse.click_count,
            });
        }
        Action::Pointer(app_ui::shell::PointerEvent {
            target: app_ui::shell::PointerTarget::Outside,
            screen: mouse.position,
            action: mouse.action,
            modifiers: mouse.modifiers,
            click_count: mouse.click_count,
        })
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

#[cfg(test)]
#[allow(clippy::too_many_lines)]
fn frame_for_state(
    state: &AppState,
    size: (u16, u16),
    capabilities: editor_types::TerminalCapabilities,
) -> Framebuffer {
    scene_for_state(state, size, capabilities).frame
}

#[allow(clippy::too_many_lines)]
fn scene_for_state(
    state: &AppState,
    size: (u16, u16),
    capabilities: editor_types::TerminalCapabilities,
) -> RenderScene {
    let mut frame = empty_frame(size.0, size.1);
    // The active tab buffer is authoritative. `active_text` is a projection used by recovery and
    // serialization; reconstructing a TextBuffer here would reset selections and undo history on
    // every frame.
    let buffer = &state.buffer;
    let snapshot = buffer.snapshot();
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
    let git_markers = git_markers_for_buffer(state, buffer);
    let mut folds = state
        .panes
        .iter()
        .find(|pane| pane.id == state.focused_pane)
        .map_or_else(editor_core::FoldSet::default, |pane| pane.folds.clone());
    if folds.regions().is_empty() {
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
    }
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
            let line = u32::try_from(result.line_number.saturating_sub(1)).ok()?;
            let line_start = snapshot
                .position_to_offset(editor_types::LogicalPosition { line, character: 0 })
                .ok()?;
            let line_text = snapshot.line_text(line).ok()?;
            if buffer.is_dirty() && line_text != result.line_text {
                return None;
            }
            let start_byte = result.line_byte_range.start;
            let end_byte = result.line_byte_range.end;
            let matched = line_text.get(start_byte..end_byte)?;
            if matched != result.matched_text {
                return None;
            }
            let start_chars = line_text[..start_byte].chars().count();
            let end_chars = line_text[..end_byte].chars().count();
            let start = editor_core::CharacterOffset(line_start.0.saturating_add(start_chars));
            let end = editor_core::CharacterOffset(line_start.0.saturating_add(end_chars));
            Some(editor_core::TextRange { start, end })
        })
        .collect::<Vec<_>>();
    let primary = buffer.selections().primary();
    let cursor_position = buffer
        .offset_to_position(primary.active)
        .unwrap_or_default();
    let selection_summary = if buffer.selections().len() > 1 {
        format!("{} cursors", buffer.selections().len())
    } else if primary.is_cursor() {
        String::from("1 cursor")
    } else {
        format!(
            "{} chars selected",
            primary
                .range()
                .end
                .0
                .saturating_sub(primary.range().start.0)
        )
    };
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
        cursor_line: cursor_position.line.saturating_add(1),
        cursor_column: cursor_position.character.saturating_add(1),
        selection_summary,
        ..EditorStatusData::default()
    };
    let viewport = EditorViewportState {
        pane_id: state.focused_pane,
        title: file_name,
        snapshot,
        viewport: state
            .panes
            .iter()
            .find(|pane| pane.id == state.focused_pane)
            .map_or_else(app_ui::editor::TextViewport::default, |pane| pane.viewport),
        selections: state.focused_pane().map_or_else(
            || buffer.selections().clone(),
            |pane| pane.selections.clone(),
        ),
        folds: folds.clone(),
        markers: SemanticMarkerSet {
            language: language_markers.clone(),
            git: git_markers.clone(),
            diagnostics: diagnostic_markers,
            ..SemanticMarkerSet::default()
        },
        syntax_spans: syntax_spans.clone(),
        search_matches: search_matches.clone(),
        bracket_matches: bracket_matches.clone(),
        status: status.clone(),
        show_line_numbers: state.show_line_numbers,
        tab_width: state.tab_width,
        overview_whole_document: true,
        inlay_hints: state
            .language_ui
            .inlay_hints
            .current()
            .map_or_else(Vec::new, |view| view.positions.clone()),
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
            is_directory: entry.is_directory,
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
        .panes
        .iter()
        .find(|pane| pane.id != state.focused_pane)
        .and_then(|pane| state.tabs.get(pane.displayed_tab).map(|tab| (pane, tab)))
        .map(|(pane, tab)| {
            let shared = pane.displayed_tab == state.active_tab;
            let title = tab
                .path
                .as_ref()
                .and_then(|path| path.file_name())
                .map_or_else(
                    || "Welcome".to_owned(),
                    |name| name.to_string_lossy().into_owned(),
                );
            EditorViewportState {
                pane_id: pane.id,
                title: title.clone(),
                snapshot: if shared {
                    buffer.snapshot()
                } else {
                    tab.buffer.snapshot()
                },
                viewport: pane.viewport,
                selections: pane.selections.clone(),
                folds: pane.folds.clone(),
                // Diagnostics, syntax, search, Git and inlay data are document-scoped. Until
                // their respective stores are keyed by document id, never leak focused-pane
                // decorations into a different secondary document.
                markers: if shared {
                    SemanticMarkerSet {
                        language: language_markers.clone(),
                        git: git_markers.clone(),
                        ..SemanticMarkerSet::default()
                    }
                } else {
                    SemanticMarkerSet::default()
                },
                syntax_spans: if shared {
                    syntax_spans.clone()
                } else {
                    Vec::new()
                },
                search_matches: if shared {
                    search_matches.clone()
                } else {
                    Vec::new()
                },
                bracket_matches: if shared {
                    bracket_matches.clone()
                } else {
                    Vec::new()
                },
                status: EditorStatusData {
                    file_name: title.clone(),
                    dirty: tab.buffer.is_dirty(),
                    ..status.clone()
                },
                show_line_numbers: state.show_line_numbers,
                tab_width: state.tab_width,
                overview_whole_document: true,
                inlay_hints: Vec::new(),
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
            scroll: state.explorer_scroll,
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
                disposition: tab.disposition,
            })
            .collect(),
        root,
        bottom: BottomPanelState {
            title: match state.bottom_panel_view {
                super::state::BottomPanelView::Output => "Output",
                super::state::BottomPanelView::Problems => "Problems",
                super::state::BottomPanelView::Search => "Search",
                super::state::BottomPanelView::Git => "Source Control",
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
        } else if state.menu_open {
            ShellFocus::Menu
        } else if state.explorer_focus {
            ShellFocus::Explorer
        } else {
            ShellFocus::Editor
        },
        status,
        menu_open: state.menu_open,
        menu_category: state.menu_category,
        menu_item: state.menu_item,
    };
    let layout = shell.layout_snapshot(app_ui::widgets::Rect::new(0, 0, size.0, size.1));
    shell.render_snapshot(&mut frame, &layout, capabilities);
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
        };
        if let Some(panel) = panel {
            blit_frame(&mut frame, &panel, bottom.x, bottom.y);
        }
    }
    if let Some(input) = input_overlay_for_state(state, size) {
        blit_frame(&mut frame, &input, 2, 1);
    }
    if let Some(overlay) = state.language_ui.contextual_overlay.as_ref() {
        if !overlay.rows.is_empty() {
            let width = overlay.placement.width.min(size.0.saturating_sub(2)).max(8);
            let height = overlay
                .rows
                .len()
                .saturating_add(2)
                .try_into()
                .unwrap_or(u16::MAX)
                .min(size.1.saturating_sub(2))
                .max(3);
            let (anchor_x, anchor_y) = screen_anchor_for_overlay(
                &shell.root,
                layout.editor,
                state.focused_pane,
                overlay.placement.anchor,
            )
            .unwrap_or((layout.editor.x, layout.editor.y));
            let x = anchor_x.min(size.0.saturating_sub(width));
            let y = anchor_y
                .saturating_add(1)
                .min(size.1.saturating_sub(height));
            let mut popup = Framebuffer::new(width, height);
            let rect = app_ui::widgets::Rect::new(0, 0, width, height);
            app_ui::widgets::fill_rect(
                &mut popup,
                rect,
                " ",
                StyleRole::EditorText,
                StyleRole::Panel,
            );
            app_ui::widgets::draw_border(&mut popup, rect, StyleRole::Gutter, StyleRole::Panel);
            let _ = app_ui::widgets::write_text(
                &mut popup,
                1,
                0,
                &overlay.title,
                StyleRole::Gutter,
                StyleRole::Panel,
            );
            for (index, row) in overlay.rows.iter().enumerate() {
                let text = row
                    .cells
                    .iter()
                    .map(|cell| cell.symbol.as_str())
                    .collect::<String>();
                let row_y = u16::try_from(index.saturating_add(1)).unwrap_or(u16::MAX);
                let _ = app_ui::widgets::write_text(
                    &mut popup,
                    1,
                    row_y,
                    &text,
                    StyleRole::EditorText,
                    if overlay.selected == index {
                        StyleRole::SelectionBackground
                    } else {
                        StyleRole::Panel
                    },
                );
            }
            blit_frame(&mut frame, &popup, x, y);
        }
    }
    let cursor = if matches!(shell.focus, ShellFocus::Editor) {
        cursor_for_panes(&shell.root, layout.editor, state.focused_pane)
    } else {
        None
    };
    RenderScene {
        frame,
        layout,
        cursor,
    }
}

fn cursor_for_panes(
    node: &PaneNode,
    rect: app_ui::widgets::Rect,
    focused_pane: u32,
) -> Option<(u16, u16)> {
    match node {
        PaneNode::Leaf(viewport) if viewport.pane_id == focused_pane => {
            let content = if rect.height > 1 {
                app_ui::widgets::Rect::new(
                    rect.x,
                    rect.y.saturating_add(1),
                    rect.width,
                    rect.height.saturating_sub(1),
                )
            } else {
                rect
            };
            viewport.cursor_cell(content)
        }
        PaneNode::Leaf(_) => None,
        PaneNode::Split {
            axis,
            ratio_percent,
            first,
            second,
        } => {
            let inner = rect;
            match axis {
                app_ui::shell::SplitAxis::Vertical => {
                    let left_width = inner
                        .width
                        .saturating_mul(*ratio_percent)
                        .saturating_div(100)
                        .max(1);
                    let divider = inner.x.saturating_add(left_width);
                    cursor_for_panes(
                        first,
                        app_ui::widgets::Rect::new(inner.x, inner.y, left_width, inner.height),
                        focused_pane,
                    )
                    .or_else(|| {
                        cursor_for_panes(
                            second,
                            app_ui::widgets::Rect::new(
                                divider.saturating_add(1),
                                inner.y,
                                inner.width.saturating_sub(left_width).saturating_sub(1),
                                inner.height,
                            ),
                            focused_pane,
                        )
                    })
                }
                app_ui::shell::SplitAxis::Horizontal => {
                    let top_height = inner
                        .height
                        .saturating_mul(*ratio_percent)
                        .saturating_div(100)
                        .max(1);
                    let divider = inner.y.saturating_add(top_height);
                    cursor_for_panes(
                        first,
                        app_ui::widgets::Rect::new(inner.x, inner.y, inner.width, top_height),
                        focused_pane,
                    )
                    .or_else(|| {
                        cursor_for_panes(
                            second,
                            app_ui::widgets::Rect::new(
                                inner.x,
                                divider.saturating_add(1),
                                inner.width,
                                inner.height.saturating_sub(top_height).saturating_sub(1),
                            ),
                            focused_pane,
                        )
                    })
                }
            }
        }
    }
}

fn screen_anchor_for_overlay(
    node: &PaneNode,
    rect: app_ui::widgets::Rect,
    pane_id: u32,
    anchor: editor_types::LogicalPosition,
) -> Option<(u16, u16)> {
    match node {
        PaneNode::Leaf(viewport) if viewport.pane_id == pane_id => {
            let content = if rect.height > 1 {
                app_ui::widgets::Rect::new(
                    rect.x,
                    rect.y.saturating_add(1),
                    rect.width,
                    rect.height.saturating_sub(1),
                )
            } else {
                rect
            };
            let geometry = viewport.geometry(content);
            let text_left = geometry.text.x;
            let text_right = geometry.text.right();
            let row = anchor.line.checked_sub(viewport.viewport.top_line)?;
            if row >= u32::from(content.height) {
                return None;
            }
            let offset = viewport.snapshot.position_to_offset(anchor).ok()?;
            let display = viewport
                .snapshot
                .display_column(offset, viewport.tab_width)
                .ok()?;
            let column = display.saturating_sub(usize::from(viewport.viewport.left_column));
            let x = text_left.saturating_add(u16::try_from(column).ok()?);
            let y = content.y.saturating_add(u16::try_from(row).ok()?);
            (x < text_right && y < content.bottom()).then_some((x, y))
        }
        PaneNode::Leaf(_) => None,
        PaneNode::Split {
            axis,
            ratio_percent,
            first,
            second,
        } => {
            let inner = rect;
            match axis {
                app_ui::shell::SplitAxis::Vertical => {
                    let left = inner
                        .width
                        .saturating_mul(*ratio_percent)
                        .saturating_div(100)
                        .max(1);
                    let divider = inner.x.saturating_add(left);
                    screen_anchor_for_overlay(
                        first,
                        app_ui::widgets::Rect::new(inner.x, inner.y, left, inner.height),
                        pane_id,
                        anchor,
                    )
                    .or_else(|| {
                        screen_anchor_for_overlay(
                            second,
                            app_ui::widgets::Rect::new(
                                divider.saturating_add(1),
                                inner.y,
                                inner.width.saturating_sub(left).saturating_sub(1),
                                inner.height,
                            ),
                            pane_id,
                            anchor,
                        )
                    })
                }
                app_ui::shell::SplitAxis::Horizontal => {
                    let top = inner
                        .height
                        .saturating_mul(*ratio_percent)
                        .saturating_div(100)
                        .max(1);
                    let divider = inner.y.saturating_add(top);
                    screen_anchor_for_overlay(
                        first,
                        app_ui::widgets::Rect::new(inner.x, inner.y, inner.width, top),
                        pane_id,
                        anchor,
                    )
                    .or_else(|| {
                        screen_anchor_for_overlay(
                            second,
                            app_ui::widgets::Rect::new(
                                inner.x,
                                divider.saturating_add(1),
                                inner.width,
                                inner.height.saturating_sub(top).saturating_sub(1),
                            ),
                            pane_id,
                            anchor,
                        )
                    })
                }
            }
        }
    }
}

/// Projects Git diff hunks into precise document ranges for the editor gutter and overview.
///
/// A status entry describes a file, but painting the whole file as changed is misleading for
/// large documents. Hunk line numbers are one-based and refer to the working-tree document.
fn git_markers_for_buffer(
    state: &AppState,
    buffer: &editor_core::TextBuffer,
) -> Vec<app_ui::editor::MarkerSpan> {
    let Some(path) = state.active_path.as_ref() else {
        return Vec::new();
    };
    let Some(dashboard) = state.git_dashboard.as_ref() else {
        return Vec::new();
    };
    let matches_path = |candidate: &std::path::Path| {
        candidate == path
            || candidate.canonicalize().ok().as_ref() == path.canonicalize().ok().as_ref()
    };
    let mut markers = Vec::new();
    for file in &dashboard.diff_files {
        let candidate = state
            .git_root
            .as_ref()
            .map_or_else(|| file.path.clone(), |root| root.join(&file.path));
        if !matches_path(&candidate) {
            continue;
        }
        for hunk in &file.hunks {
            let mut line = hunk.new_range.0.max(1);
            let has_addition = hunk
                .lines
                .iter()
                .any(|diff| matches!(diff.kind, vcs_git::GitDiffLineKind::Addition));
            let kind = if has_addition {
                app_ui::editor::MarkerKind::Modified
            } else {
                app_ui::editor::MarkerKind::Deleted
            };
            for diff in &hunk.lines {
                match diff.kind {
                    vcs_git::GitDiffLineKind::Addition => {
                        if let Some(range) = line_range(buffer, line) {
                            markers.push(app_ui::editor::MarkerSpan { range, kind });
                        }
                        line = line.saturating_add(1);
                    }
                    vcs_git::GitDiffLineKind::Context => line = line.saturating_add(1),
                    vcs_git::GitDiffLineKind::Removal => {
                        let anchor = line.saturating_sub(1).max(1);
                        if let Some(range) = line_range(buffer, anchor) {
                            markers.push(app_ui::editor::MarkerSpan {
                                range,
                                kind: app_ui::editor::MarkerKind::Deleted,
                            });
                        }
                    }
                    vcs_git::GitDiffLineKind::Meta => {}
                }
            }
            if hunk.lines.is_empty() && hunk.new_range.1 > 0 {
                for offset in 0..hunk.new_range.1 {
                    if let Some(range) = line_range(buffer, hunk.new_range.0.saturating_add(offset))
                    {
                        markers.push(app_ui::editor::MarkerSpan { range, kind });
                    }
                }
            }
        }
    }
    if markers.is_empty() {
        for entry in &dashboard.changes {
            let candidate = state
                .git_root
                .as_ref()
                .map_or_else(|| entry.path.clone(), |root| root.join(&entry.path));
            if matches_path(&candidate) && entry.untracked {
                if let Some(range) = line_range(buffer, 1) {
                    markers.push(app_ui::editor::MarkerSpan {
                        range,
                        kind: app_ui::editor::MarkerKind::Added,
                    });
                }
            }
        }
    }
    markers
}

fn line_range(
    buffer: &editor_core::TextBuffer,
    one_based_line: usize,
) -> Option<editor_types::TextRange> {
    let snapshot = buffer.snapshot();
    let line = u32::try_from(one_based_line.saturating_sub(1)).ok()?;
    let start = snapshot
        .position_to_offset(editor_types::LogicalPosition { line, character: 0 })
        .ok()?;
    let end = snapshot
        .line_text(line)
        .ok()
        .map(|text| editor_core::CharacterOffset(start.0.saturating_add(text.chars().count())))?;
    Some(editor_types::TextRange { start, end })
}

#[allow(clippy::too_many_lines)]
fn input_overlay_for_state(state: &AppState, size: (u16, u16)) -> Option<Framebuffer> {
    let mode = state.input_mode.as_ref()?;
    let width = size.0.saturating_sub(4).clamp(1, 100);
    let height = match mode {
        super::state::InputMode::QuickOpen => size.1.saturating_sub(2).clamp(1, 16),
        super::state::InputMode::Encoding { .. }
        | super::state::InputMode::LineEndings
        | super::state::InputMode::RecentWorkspace => size.1.saturating_sub(2).clamp(5, 16),
        _ => 3.min(size.1.saturating_sub(1).max(1)),
    };
    let mut overlay = Framebuffer::new(width, height);
    if matches!(mode, super::state::InputMode::QuickOpen) {
        app_ui::workspace::draw_quick_open(&mut overlay, &state.workspace_ui.quick_open);
        return Some(overlay);
    }
    let label = match mode {
        super::state::InputMode::OpenFolder => "Open Folder",
        super::state::InputMode::ProjectSearch => "Search workspace",
        super::state::InputMode::ProjectSearchInclude => "Search include filter",
        super::state::InputMode::ProjectSearchExclude => "Search exclude filter",
        super::state::InputMode::ProjectReplaceQuery => "Replace in Files query",
        super::state::InputMode::ProjectReplaceReplacement { .. } => "Replace in Files with",
        super::state::InputMode::SaveAs => "Save As",
        super::state::InputMode::GoToLine => "Go to Line/Column",
        super::state::InputMode::Encoding { save: true } => "Save with Encoding",
        super::state::InputMode::Encoding { save: false } => "Reopen with Encoding",
        super::state::InputMode::LineEndings => "Change End of Line Sequence",
        super::state::InputMode::RecentWorkspace => "Open Recent Workspace",
        super::state::InputMode::GitCreateBranch => "Git: Create Branch",
        super::state::InputMode::GitSwitchBranch => "Git: Switch Branch",
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
    if matches!(mode, super::state::InputMode::ProjectSearch) {
        let options = &state.project_search_options;
        let include = options.include.as_deref().unwrap_or("*");
        let exclude = options.exclude.as_deref().unwrap_or("-");
        write_overlay_line(
            &mut overlay,
            2,
            &format!(
                "{} case={} whole-word={} include={} exclude={}",
                if options.literal { "literal" } else { "regex" },
                options.case_sensitive,
                options.whole_word,
                include,
                exclude
            ),
            StyleRole::Information,
        );
    }
    if matches!(mode, super::state::InputMode::Encoding { .. }) {
        write_overlay_line(
            &mut overlay,
            2,
            "Up/Down select  Enter apply  type to filter",
            StyleRole::Information,
        );
        if let Some(picker) = state.encoding_picker.as_ref() {
            let selected = picker.selected().map(|row| row.id.as_str());
            for (index, row) in picker
                .visible_rows()
                .into_iter()
                .take(usize::from(height.saturating_sub(3)))
                .enumerate()
            {
                let row_number = u16::try_from(index).unwrap_or(u16::MAX).saturating_add(3);
                let marker = if selected == Some(row.id.as_str()) {
                    "> "
                } else {
                    "  "
                };
                let role = if selected == Some(row.id.as_str()) {
                    StyleRole::Selection
                } else {
                    StyleRole::Information
                };
                write_overlay_line(
                    &mut overlay,
                    row_number,
                    &format!("{marker}{}", row.label),
                    role,
                );
            }
        }
    }
    if matches!(mode, super::state::InputMode::LineEndings) {
        write_overlay_line(
            &mut overlay,
            2,
            "Up/Down select  Enter apply  Esc cancel",
            StyleRole::Information,
        );
        if let Some(picker) = state.line_endings_picker.as_ref() {
            let selected = picker.selected().map(|row| row.id.as_str());
            for (index, row) in picker.visible_rows().into_iter().enumerate() {
                let row_number = u16::try_from(index).unwrap_or(u16::MAX).saturating_add(3);
                let marker = if selected == Some(row.id.as_str()) {
                    "> "
                } else {
                    "  "
                };
                let role = if selected == Some(row.id.as_str()) {
                    StyleRole::Selection
                } else {
                    StyleRole::Information
                };
                write_overlay_line(
                    &mut overlay,
                    row_number,
                    &format!("{marker}{}", row.label),
                    role,
                );
            }
        }
    }
    if matches!(mode, super::state::InputMode::RecentWorkspace) {
        write_overlay_line(
            &mut overlay,
            2,
            "Up/Down select  Enter open  Esc cancel",
            StyleRole::Information,
        );
        if let Some(picker) = state.recent_picker.as_ref() {
            let selected = picker.selected().map(|row| row.id.as_str());
            for (index, row) in picker
                .visible_rows()
                .into_iter()
                .take(usize::from(height.saturating_sub(3)))
                .enumerate()
            {
                let row_number = u16::try_from(index).unwrap_or(u16::MAX).saturating_add(3);
                let marker = if selected == Some(row.id.as_str()) {
                    "> "
                } else {
                    "  "
                };
                let role = if selected == Some(row.id.as_str()) {
                    StyleRole::Selection
                } else if row.disabled {
                    StyleRole::Warning
                } else {
                    StyleRole::Information
                };
                write_overlay_line(
                    &mut overlay,
                    row_number,
                    &format!(
                        "{marker}{}{}",
                        row.label,
                        if row.disabled { " (missing)" } else { "" }
                    ),
                    role,
                );
            }
        }
    }
    Some(overlay)
}

fn write_overlay_line(frame: &mut Framebuffer, row: u16, text: &str, foreground: StyleRole) {
    let _ = app_ui::widgets::write_text(frame, 0, row, text, foreground, StyleRole::Panel);
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
        state.buffer = editor_core::TextBuffer::new("fn main() {\n  1\n}");
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
        let cell = frame.get(31, 2).expect("syntax editor cell");
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
