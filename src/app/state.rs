//! Authoritative root state and pure transition logic.
#![allow(clippy::struct_excessive_bools)]

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use app_ui::widgets::{CommandEntry, CommandPaletteState};
use editor_core::{Edit, PairConfig, TextBuffer, Transaction};
use editor_types::{
    CharacterOffset, DocumentId, GitStatusSummary, InputEvent, KeyCode, LanguageServerStatus,
    LogicalPosition, Modifier, MouseAction, MouseButton, OutputLevel, OutputMessage, RequestId,
    TextRange,
};

use super::{
    action::Action,
    effect::{Effect, ProcessSpec},
    event::Event,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppState {
    pub running: bool,
    pub workspace_trusted: bool,
    pub(crate) trust_store: workspace_core::TrustStore,
    pub frame_number: u64,
    pub language_server: LanguageServerStatus,
    pub(crate) lsp_position_encoding: lsp_client::protocol::PositionEncoding,
    pub git_status: Option<GitStatusSummary>,
    pub(crate) git_dashboard: Option<app_ui::git::GitDashboardState>,
    pub(crate) git_root: Option<PathBuf>,
    pub diagnostics: Vec<editor_types::Diagnostic>,
    pub(crate) workspace_ui: app_ui::workspace::WorkspaceUiState,
    pub(crate) language_ui: app_ui::language::LanguageModel,
    pub(crate) lsp_results: HashMap<String, serde_json::Value>,
    pub(crate) last_language_method: Option<String>,
    pub(crate) syntax_snapshot: syntax_engine::SyntaxSnapshot,
    pub(crate) document_id: DocumentId,
    pub output: Vec<OutputMessage>,
    pub active_path: Option<PathBuf>,
    pub workspace_roots: Vec<PathBuf>,
    pub explorer_entries: Vec<ExplorerProjection>,
    pub active_text: String,
    pub active_dirty: bool,
    pub explorer_visible: bool,
    pub bottom_panel_visible: bool,
    pub(crate) bottom_panel_view: BottomPanelView,
    pub palette_visible: bool,
    pub(crate) palette: CommandPaletteState,
    pub(crate) tabs: Vec<TabState>,
    pub(crate) active_tab: usize,
    pub(crate) split_axis: Option<app_ui::shell::SplitAxis>,
    pub(crate) split_ratio_percent: u16,
    pub(crate) split_secondary_tab: Option<usize>,
    pending_processes: HashMap<RequestId, super::effect::ExternalProcessKind>,
    pending_clipboard: HashMap<RequestId, PendingClipboard>,
    pending_format: HashMap<RequestId, PendingFormat>,
    pending_lsp_format: HashMap<RequestId, PendingFormat>,
    pending_git_discard: Option<vcs_git::GitDiscardPlan>,
    deferred_effects: Vec<Effect>,
    pub(crate) format_on_save: bool,
    pub(crate) format_on_paste: bool,
    pub(crate) tab_width: usize,
    pub(crate) insert_spaces: bool,
    pub(crate) show_line_numbers: bool,
    pub(crate) external_formatter: Option<ProcessSpec>,
    latest_explorer_request: Option<RequestId>,
    mouse_anchor: Option<CharacterOffset>,
    /// Persistent editor buffer backing the active document. The public text fields remain a
    /// lightweight projection for views and recovery serialization.
    pub(crate) buffer: TextBuffer,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BottomPanelView {
    Output,
    Problems,
    Search,
    Git,
    Language,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExplorerProjection {
    pub depth: u8,
    pub label: String,
    pub active: bool,
    pub expanded: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TabState {
    pub path: Option<PathBuf>,
    pub buffer: TextBuffer,
    pub encoding: workspace_core::EncodingKind,
    pub with_bom: bool,
    pub line_endings: workspace_core::LineEndings,
}

impl TabState {
    fn untitled() -> Self {
        Self {
            path: None,
            buffer: TextBuffer::default(),
            encoding: workspace_core::EncodingKind::Utf8,
            with_bom: false,
            line_endings: workspace_core::LineEndings::Lf,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum PendingClipboard {
    Cut {
        range: editor_types::TextRange,
        expected: String,
    },
    Paste {
        range: editor_types::TextRange,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PendingFormat {
    version: u64,
    save_after: bool,
}

fn panel_glyphs(text: &str) -> Vec<app_ui::language::Glyph> {
    text.chars()
        .map(|character| app_ui::language::Glyph::plain(character.to_string()))
        .collect()
}

fn panel_row(text: impl AsRef<str>) -> app_ui::language::PanelRow {
    app_ui::language::PanelRow::new(panel_glyphs(text.as_ref()))
}

fn json_label(value: &serde_json::Value) -> String {
    value.as_str().map_or_else(
        || serde_json::to_string(value).unwrap_or_else(|_| String::from("<invalid result>")),
        ToOwned::to_owned,
    )
}

fn json_position(value: &serde_json::Value) -> Option<LogicalPosition> {
    Some(LogicalPosition {
        line: value
            .get("line")?
            .as_u64()
            .and_then(|line| u32::try_from(line).ok())?,
        character: value
            .get("character")?
            .as_u64()
            .and_then(|character| u32::try_from(character).ok())?,
    })
}

fn json_path(uri: &str) -> PathBuf {
    let path = uri
        .strip_prefix("file:///")
        .or_else(|| uri.strip_prefix("file://"))
        .unwrap_or(uri);
    PathBuf::from(path.replace('/', std::path::MAIN_SEPARATOR_STR))
}

fn location_choice(
    value: &serde_json::Value,
    document: DocumentId,
) -> Option<app_ui::language::LocationChoice> {
    let uri = value.get("uri").and_then(serde_json::Value::as_str)?;
    let range = value.get("range")?;
    let position = json_position(range.get("start")?)?;
    let path = json_path(uri);
    let label = value
        .get("name")
        .or_else(|| value.get("targetUri"))
        .map_or_else(|| path.display().to_string(), json_label);
    Some(app_ui::language::LocationChoice {
        label: panel_glyphs(&label),
        document,
        path,
        position,
    })
}

fn location_choices(
    value: &serde_json::Value,
    document: DocumentId,
) -> Vec<app_ui::language::LocationChoice> {
    let values = value
        .as_array()
        .cloned()
        .or_else(|| {
            value
                .get("items")
                .and_then(serde_json::Value::as_array)
                .cloned()
        })
        .unwrap_or_default();
    values
        .iter()
        .filter_map(|item| {
            location_choice(item, document).or_else(|| {
                let target = item.get("targetSelectionRange").and_then(|_| {
                    Some(serde_json::json!({
                        "uri": item.get("targetUri")?,
                        "range": item.get("targetSelectionRange")?
                    }))
                })?;
                location_choice(&target, document)
            })
        })
        .collect()
}

#[allow(clippy::too_many_lines)]
fn language_result_from_response(
    method: &str,
    result: &serde_json::Value,
    version: u64,
    document: DocumentId,
) -> Option<app_ui::language::LanguageResult> {
    use app_ui::language::{
        CodeActionView, CompletionView, FormattingFeedbackView, HoverView, InlayHintsView,
        LanguageResult, LocationChooserView, PanelState, RenamePreviewView, SignatureView,
        SymbolsView, Versioned,
    };

    let result = match method {
        "textDocument/completion" => {
            let values = result
                .as_array()
                .cloned()
                .or_else(|| {
                    result
                        .get("items")
                        .and_then(serde_json::Value::as_array)
                        .cloned()
                })
                .unwrap_or_default();
            let rows = values
                .iter()
                .map(|item| {
                    let label = item
                        .get("label")
                        .map_or_else(|| json_label(item), json_label);
                    panel_row(label)
                })
                .collect();
            LanguageResult::Completion(Versioned::new(
                version,
                CompletionView {
                    list: PanelState::new(panel_glyphs("Completion"))
                        .with_rows(rows)
                        .with_selected(Some(0)),
                    details: PanelState::new(panel_glyphs("Details")),
                },
            ))
        }
        "textDocument/hover" => LanguageResult::Hover(Versioned::new(
            version,
            HoverView {
                card: PanelState::new(panel_glyphs("Hover")).with_rows(vec![panel_row(
                    result
                        .get("contents")
                        .map_or_else(|| json_label(result), json_label),
                )]),
            },
        )),
        "textDocument/signatureHelp" => {
            let rows = result
                .get("signatures")
                .and_then(serde_json::Value::as_array)
                .map(|values| {
                    values
                        .iter()
                        .map(|value| {
                            panel_row(
                                value
                                    .get("label")
                                    .map_or_else(|| json_label(value), json_label),
                            )
                        })
                        .collect()
                })
                .unwrap_or_default();
            LanguageResult::Signature(Versioned::new(
                version,
                SignatureView {
                    card: PanelState::new(panel_glyphs("Signature Help")).with_rows(rows),
                },
            ))
        }
        "textDocument/definition"
        | "textDocument/declaration"
        | "textDocument/implementation"
        | "textDocument/references" => {
            let choices = location_choices(result, document);
            let view = LocationChooserView {
                title: panel_glyphs(if method.ends_with("references") {
                    "References"
                } else {
                    "Go to definition"
                }),
                selected: (!choices.is_empty()).then_some(0),
                choices,
            };
            if method.ends_with("references") {
                LanguageResult::References(Versioned::new(version, view))
            } else {
                LanguageResult::GoTo(Versioned::new(version, view))
            }
        }
        "textDocument/rename" => {
            let changes = result
                .get("changes")
                .and_then(serde_json::Value::as_object)
                .map_or(0, serde_json::Map::len);
            LanguageResult::Rename(Versioned::new(
                version,
                RenamePreviewView {
                    title: panel_glyphs("Rename preview"),
                    before: PanelState::new(panel_glyphs("Before"))
                        .with_rows(vec![panel_row(format!("{changes} file(s)"))]),
                    after: PanelState::new(panel_glyphs("After")),
                    conflicts: PanelState::new(panel_glyphs("Conflicts")),
                },
            ))
        }
        "textDocument/codeAction" => {
            let rows = result
                .as_array()
                .map(|values| {
                    values
                        .iter()
                        .map(|value| {
                            panel_row(
                                value
                                    .get("title")
                                    .map_or_else(|| json_label(value), json_label),
                            )
                        })
                        .collect()
                })
                .unwrap_or_default();
            LanguageResult::CodeActions(Versioned::new(
                version,
                CodeActionView {
                    list: PanelState::new(panel_glyphs("Code Actions"))
                        .with_rows(rows)
                        .with_selected(Some(0)),
                },
            ))
        }
        "textDocument/inlayHint" => {
            let rows = result
                .as_array()
                .map(|values| values.iter().map(json_label).map(panel_row).collect())
                .unwrap_or_default();
            LanguageResult::InlayHints(Versioned::new(
                version,
                InlayHintsView {
                    list: PanelState::new(panel_glyphs("Inlay Hints")).with_rows(rows),
                },
            ))
        }
        "textDocument/documentSymbol" | "workspace/symbol" => {
            let rows = result
                .as_array()
                .map(|values| {
                    values
                        .iter()
                        .map(|value| {
                            panel_row(
                                value
                                    .get("name")
                                    .map_or_else(|| json_label(value), json_label),
                            )
                        })
                        .collect()
                })
                .unwrap_or_default();
            LanguageResult::Symbols(Versioned::new(
                version,
                SymbolsView {
                    list: PanelState::new(panel_glyphs("Symbols")).with_rows(rows),
                },
            ))
        }
        "textDocument/formatting" | "textDocument/rangeFormatting" => {
            let edits = result.as_array().map_or(0, Vec::len);
            LanguageResult::Formatting(Versioned::new(
                version,
                FormattingFeedbackView {
                    card: PanelState::new(panel_glyphs("Formatting"))
                        .with_rows(vec![panel_row(format!("{edits} edit(s) received"))]),
                },
            ))
        }
        _ => return None,
    };
    Some(result)
}

/// Decodes LSP semantic-token delta tuples into the versioned UI span model. Invalid or
/// out-of-range tuples are discarded as a whole response so malformed server data cannot paint
/// ranges onto a different document.
fn semantic_result_from_response(
    result: &serde_json::Value,
    version: u64,
    document: DocumentId,
    text: &str,
    encoding: lsp_client::protocol::PositionEncoding,
) -> Option<app_ui::language::LanguageResult> {
    use app_ui::language::{LanguageResult, SpanSet, StyledSpan, TokenStyle, Versioned};

    let values = result.get("data")?.as_array()?;
    if values.is_empty() || values.len() % 5 != 0 {
        return None;
    }
    let buffer = TextBuffer::new(text);
    let mut line = 0_u32;
    let mut start_character = 0_u32;
    let mut spans = Vec::with_capacity(values.len() / 5);
    for tuple in values.chunks_exact(5) {
        let delta_line = tuple[0]
            .as_u64()
            .and_then(|value| u32::try_from(value).ok())?;
        let delta_start = tuple[1]
            .as_u64()
            .and_then(|value| u32::try_from(value).ok())?;
        let length = tuple[2]
            .as_u64()
            .and_then(|value| u32::try_from(value).ok())?;
        let token_type = tuple[3]
            .as_u64()
            .and_then(|value| u32::try_from(value).ok())?;
        if delta_line == 0 {
            start_character = start_character.checked_add(delta_start)?;
        } else {
            line = line.checked_add(delta_line)?;
            start_character = delta_start;
        }
        let end_character = start_character.checked_add(length)?;
        let start = lsp_client::protocol::lsp_position_to_editor(
            text,
            lsp_client::protocol::Position {
                line,
                character: start_character,
            },
            encoding,
        )
        .ok()
        .and_then(|position| buffer.position_to_offset(position).ok())?;
        let end = lsp_client::protocol::lsp_position_to_editor(
            text,
            lsp_client::protocol::Position {
                line,
                character: end_character,
            },
            encoding,
        )
        .ok()
        .and_then(|position| buffer.position_to_offset(position).ok())?;
        let range = TextRange::new(start, end)?;
        let style = match token_type {
            0 => TokenStyle::SemanticType,
            1 => TokenStyle::SemanticFunction,
            2 => TokenStyle::SemanticVariable,
            _ => TokenStyle::Plain,
        };
        spans.push(StyledSpan { range, style });
    }
    Some(LanguageResult::Semantic(Versioned::new(
        version,
        SpanSet {
            document,
            version,
            spans,
        },
    )))
}

fn config_encoding_to_workspace(value: &config_core::EncodingKind) -> workspace_core::EncodingKind {
    match value {
        config_core::EncodingKind::Utf8 => workspace_core::EncodingKind::Utf8,
        config_core::EncodingKind::Utf16Le => workspace_core::EncodingKind::Utf16Le,
        config_core::EncodingKind::Utf16Be => workspace_core::EncodingKind::Utf16Be,
        config_core::EncodingKind::Legacy(name) => {
            workspace_core::EncodingKind::Legacy(name.clone())
        }
    }
}

fn workspace_encoding_to_config(value: &workspace_core::EncodingKind) -> config_core::EncodingKind {
    match value {
        workspace_core::EncodingKind::Utf8 => config_core::EncodingKind::Utf8,
        workspace_core::EncodingKind::Utf16Le => config_core::EncodingKind::Utf16Le,
        workspace_core::EncodingKind::Utf16Be => config_core::EncodingKind::Utf16Be,
        workspace_core::EncodingKind::Legacy(name) => {
            config_core::EncodingKind::Legacy(name.clone())
        }
    }
}

fn config_line_endings_to_workspace(
    value: config_core::LineEndings,
) -> workspace_core::LineEndings {
    match value {
        config_core::LineEndings::None => workspace_core::LineEndings::None,
        config_core::LineEndings::Lf => workspace_core::LineEndings::Lf,
        config_core::LineEndings::Crlf => workspace_core::LineEndings::Crlf,
        config_core::LineEndings::Mixed => workspace_core::LineEndings::Mixed,
    }
}

fn workspace_line_endings_to_config(
    value: workspace_core::LineEndings,
) -> config_core::LineEndings {
    match value {
        workspace_core::LineEndings::None => config_core::LineEndings::None,
        workspace_core::LineEndings::Lf => config_core::LineEndings::Lf,
        workspace_core::LineEndings::Crlf => config_core::LineEndings::Crlf,
        workspace_core::LineEndings::Mixed => config_core::LineEndings::Mixed,
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            running: true,
            workspace_trusted: false,
            trust_store: workspace_core::TrustStore::default(),
            frame_number: 0,
            language_server: LanguageServerStatus::Stopped,
            lsp_position_encoding: lsp_client::protocol::PositionEncoding::Utf16,
            git_status: None,
            git_dashboard: None,
            git_root: None,
            diagnostics: Vec::new(),
            workspace_ui: app_ui::workspace::WorkspaceUiState::default(),
            language_ui: app_ui::language::LanguageModel::new(0),
            lsp_results: HashMap::new(),
            last_language_method: None,
            syntax_snapshot: syntax_engine::SyntaxSnapshot::default(),
            document_id: DocumentId(1),
            output: Vec::new(),
            active_path: None,
            workspace_roots: Vec::new(),
            explorer_entries: Vec::new(),
            active_text: String::new(),
            active_dirty: false,
            explorer_visible: true,
            bottom_panel_visible: false,
            bottom_panel_view: BottomPanelView::Output,
            palette_visible: false,
            palette: CommandPaletteState::new(vec![
                CommandEntry::available("editor.save", "Save"),
                CommandEntry::available("editor.undo", "Undo"),
                CommandEntry::available("editor.redo", "Redo"),
                CommandEntry::available("editor.copy", "Copy"),
                CommandEntry::available("editor.cut", "Cut"),
                CommandEntry::available("editor.paste", "Paste"),
                CommandEntry::available("editor.format", "Format Document"),
                CommandEntry::available("editor.reopenUtf8", "Reopen with UTF-8"),
                CommandEntry::available("editor.reopenUtf16Le", "Reopen with UTF-16 LE"),
                CommandEntry::available("editor.reopenUtf16Be", "Reopen with UTF-16 BE"),
                CommandEntry::available("editor.convertUtf8", "Convert to UTF-8"),
                CommandEntry::available("editor.convertUtf16Le", "Convert to UTF-16 LE"),
                CommandEntry::available("editor.convertUtf16Be", "Convert to UTF-16 BE"),
                CommandEntry::available("editor.toggleFormatOnSave", "Toggle Format on Save"),
                CommandEntry::available("editor.toggleFormatOnPaste", "Toggle Format on Paste"),
                CommandEntry::available("workbench.splitVertical", "Split Editor Vertical"),
                CommandEntry::available("workbench.splitHorizontal", "Split Editor Horizontal"),
                CommandEntry::available("workbench.closeSplit", "Close Editor Split"),
                CommandEntry::available("workbench.showProblems", "Show Problems"),
                CommandEntry::available("workbench.showGit", "Show Source Control"),
                CommandEntry::available("workbench.showOutput", "Show Output"),
                CommandEntry::available("editor.quit", "Quit"),
                CommandEntry::available("git.refresh", "Refresh Git Status"),
                CommandEntry::available("git.showChanges", "Show Git Changes"),
                CommandEntry::available("git.showDiff", "Show Git Diff"),
                CommandEntry::available("git.showBranches", "Show Git Branches"),
                CommandEntry::available("git.showStashes", "Show Git Stashes"),
                CommandEntry::available("git.showHistory", "Show Git History"),
                CommandEntry::available("git.showCommit", "Show Git Commit"),
                CommandEntry::available("git.showConflicts", "Show Git Conflicts"),
            ]),
            tabs: vec![TabState::untitled()],
            active_tab: 0,
            split_axis: None,
            split_ratio_percent: 50,
            split_secondary_tab: None,
            pending_processes: HashMap::new(),
            pending_clipboard: HashMap::new(),
            pending_format: HashMap::new(),
            pending_lsp_format: HashMap::new(),
            pending_git_discard: None,
            deferred_effects: Vec::new(),
            format_on_save: false,
            format_on_paste: false,
            tab_width: 4,
            insert_spaces: true,
            show_line_numbers: true,
            external_formatter: None,
            latest_explorer_request: None,
            mouse_anchor: None,
            buffer: TextBuffer::default(),
        }
    }
}

impl AppState {
    /// Applies an editor-view action through the same root transition path used by terminal input.
    pub fn apply_editor_action(&mut self, action: app_ui::editor::EditorAction) -> Transition {
        use app_ui::editor::EditorAction;
        match action {
            EditorAction::MoveCursor {
                line_delta,
                column_delta,
                extend_selection,
            } => {
                let (code, count) = if line_delta != 0 {
                    (
                        if line_delta.is_negative() {
                            KeyCode::Up
                        } else {
                            KeyCode::Down
                        },
                        line_delta.unsigned_abs(),
                    )
                } else {
                    (
                        if column_delta.is_negative() {
                            KeyCode::Left
                        } else {
                            KeyCode::Right
                        },
                        column_delta.unsigned_abs(),
                    )
                };
                let modifiers = if extend_selection {
                    editor_types::Modifiers::from_modifiers([Modifier::Shift])
                } else {
                    editor_types::Modifiers::default()
                };
                for _ in 0..count {
                    self.apply_input(InputEvent::Key(editor_types::KeyEvent {
                        code: code.clone(),
                        modifiers,
                        repeat: false,
                    }));
                }
                Transition {
                    render: true,
                    ..Transition::default()
                }
            }
            EditorAction::SelectRange(range) => {
                let _ = self
                    .buffer
                    .set_selections(editor_core::SelectionSet::single(
                        editor_core::Selection::new(range.start, range.end),
                    ));
                self.sync_buffer_projection();
                Transition {
                    render: true,
                    ..Transition::default()
                }
            }
            EditorAction::ToggleFold { .. } | EditorAction::Scroll { .. } => Transition {
                render: true,
                ..Transition::default()
            },
        }
    }

    /// Replaces the active document's selection set through the authoritative root state path.
    ///
    /// # Errors
    ///
    /// Returns the editor-core validation error when a selection is outside the document.
    pub fn set_active_selections(
        &mut self,
        selections: editor_core::SelectionSet,
    ) -> Result<(), editor_core::EditorError> {
        self.buffer.set_selections(selections)?;
        self.sync_buffer_projection();
        Ok(())
    }

    /// Configures the already-authorized external formatter fallback used by `editor.format`.
    /// The caller must still trust-gate execution through the root policy.
    pub fn set_external_formatter(&mut self, spec: Option<ProcessSpec>) {
        self.external_formatter = spec;
    }

    /// Enables formatting after successful paste transactions when a trusted formatter exists.
    pub fn set_format_on_paste(&mut self, enabled: bool) {
        self.format_on_paste = enabled;
    }

    /// Enables formatting before saves when a trusted formatter exists.
    pub fn set_format_on_save(&mut self, enabled: bool) {
        self.format_on_save = enabled;
    }

    /// Applies the resolved settings snapshot to root behavior and every open buffer.
    pub fn apply_settings(&mut self, settings: &config_core::EditorSettings) {
        self.format_on_save = settings.format_on_save;
        self.format_on_paste = settings.format_on_paste;
        self.tab_width = usize::from(settings.tab_size.max(1));
        self.insert_spaces = settings.insert_spaces;
        self.show_line_numbers = settings.line_numbers;
        let threshold = usize::try_from(settings.large_file_threshold).unwrap_or(usize::MAX);
        self.buffer.set_large_file_threshold(threshold);
        for tab in &mut self.tabs {
            tab.buffer.set_large_file_threshold(threshold);
        }
    }

    /// Exposes the immutable language view model for host integrations and deterministic tests.
    #[must_use]
    pub const fn language_model(&self) -> &app_ui::language::LanguageModel {
        &self.language_ui
    }

    /// Exposes the latest syntax snapshot for host integrations and deterministic tests.
    #[must_use]
    pub const fn syntax_snapshot(&self) -> &syntax_engine::SyntaxSnapshot {
        &self.syntax_snapshot
    }

    /// Takes effects scheduled by asynchronous event application.
    pub(crate) fn take_deferred_effects(&mut self) -> Vec<Effect> {
        std::mem::take(&mut self.deferred_effects)
    }

    /// Queues a syntax refresh for the currently active document after startup or restore.
    pub(crate) fn schedule_syntax_refresh(&mut self) {
        self.deferred_effects.push(self.syntax_effect());
    }

    /// Installs the persisted trust store before startup paths are opened.
    pub fn set_trust_store(&mut self, store: workspace_core::TrustStore) {
        self.trust_store = store;
        self.refresh_workspace_trust();
    }

    /// Routes language-panel actions to root effects. Language servers are discovered only after
    /// an explicit trust decision; unavailable servers become a visible non-blocking status.
    #[allow(clippy::needless_pass_by_value)]
    pub fn apply_language_action(
        &mut self,
        action: app_ui::language::LanguageAction,
    ) -> Transition {
        use app_ui::language::LanguageAction;
        match action {
            LanguageAction::RevealProblems => {
                self.bottom_panel_visible = true;
            }
            LanguageAction::NavigateToProblem { group } => {
                if let Some(action) = self
                    .language_ui
                    .diagnostics
                    .current()
                    .and_then(|dashboard| dashboard.problems.activate(group))
                {
                    return self.apply_language_action(action);
                }
            }
            LanguageAction::NavigateToLocation { path, .. } => {
                return self.apply_action(Action::OpenPath(path));
            }
            LanguageAction::RestartLanguageServer => {
                if let Some(path) = self.active_path.as_ref().filter(|path| path.is_file()) {
                    if let Some(language) = syntax_engine::SyntaxLanguage::from_path(path) {
                        if let Some(command) =
                            lsp_client::CommandSpec::discover_known(language.name())
                        {
                            return self.apply_action(Action::RequestEffect(
                                Effect::ExternalProcess {
                                    request: RequestId(self.frame_number.saturating_add(1)),
                                    kind: super::effect::ExternalProcessKind::LanguageServer,
                                    spec: super::effect::ProcessSpec {
                                        executable: command
                                            .executable
                                            .to_string_lossy()
                                            .into_owned(),
                                        arguments: command.args,
                                    },
                                },
                            ));
                        }
                    }
                }
                self.language_server = LanguageServerStatus::Unavailable;
            }
            LanguageAction::AcceptCompletion { index }
            | LanguageAction::ExpandCompletionDetails { index } => {
                self.output.push(OutputMessage {
                    subsystem: "lsp".to_owned(),
                    operation: "completion".to_owned(),
                    level: OutputLevel::Information,
                    message: format!("completion item {index} selected"),
                });
            }
            LanguageAction::OpenHover
            | LanguageAction::OpenSignatureHelp
            | LanguageAction::AcceptRenamePreview
            | LanguageAction::AcceptCodeAction { .. }
            | LanguageAction::DismissPopup => {}
        }
        Transition {
            render: true,
            ..Transition::default()
        }
    }

    /// Converts a language-panel request into a trust-gated JSON-RPC effect.
    #[allow(clippy::too_many_lines)]
    pub fn apply_language_effect_request(
        &mut self,
        request: app_ui::language::LanguageEffectRequest,
    ) -> Transition {
        use app_ui::language::LanguageEffectRequest;
        if self.buffer.is_large_file()
            && !matches!(request, LanguageEffectRequest::RefreshSyntax { .. })
        {
            self.output.push(OutputMessage {
                subsystem: "lsp".to_owned(),
                operation: "large-file".to_owned(),
                level: OutputLevel::Information,
                message: "language services are disabled for large files".to_owned(),
            });
            return Transition {
                render: true,
                ..Transition::default()
            };
        }
        let (method, params, version) = match request {
            LanguageEffectRequest::RequestCompletion {
                version, position, ..
            } => (
                "textDocument/completion",
                serde_json::json!({"textDocument": {"uri": self.active_document_uri()}, "position": {"line": position.line, "character": position.character}}),
                version,
            ),
            LanguageEffectRequest::RequestHover {
                version, position, ..
            } => (
                "textDocument/hover",
                serde_json::json!({"textDocument": {"uri": self.active_document_uri()}, "position": {"line": position.line, "character": position.character}}),
                version,
            ),
            LanguageEffectRequest::RequestSignatureHelp {
                version, position, ..
            } => (
                "textDocument/signatureHelp",
                serde_json::json!({"textDocument": {"uri": self.active_document_uri()}, "position": {"line": position.line, "character": position.character}}),
                version,
            ),
            LanguageEffectRequest::RequestGoTo {
                version, position, ..
            } => (
                "textDocument/definition",
                serde_json::json!({"textDocument": {"uri": self.active_document_uri()}, "position": {"line": position.line, "character": position.character}}),
                version,
            ),
            LanguageEffectRequest::RequestReferences {
                version, position, ..
            } => (
                "textDocument/references",
                serde_json::json!({"textDocument": {"uri": self.active_document_uri()}, "position": {"line": position.line, "character": position.character}, "context": {"includeDeclaration": true}}),
                version,
            ),
            LanguageEffectRequest::RequestRenamePreview {
                version,
                position,
                new_name,
                ..
            } => (
                "textDocument/rename",
                serde_json::json!({"textDocument": {"uri": self.active_document_uri()}, "position": {"line": position.line, "character": position.character}, "newName": new_name}),
                version,
            ),
            LanguageEffectRequest::RequestCodeActions { version, .. } => (
                "textDocument/codeAction",
                serde_json::json!({"textDocument": {"uri": self.active_document_uri()}, "range": {"start": {"line": 0, "character": 0}, "end": {"line": 0, "character": 0},}, "context": {"diagnostics": []}}),
                version,
            ),
            LanguageEffectRequest::RequestInlayHints { version, .. } => (
                "textDocument/inlayHint",
                serde_json::json!({"textDocument": {"uri": self.active_document_uri()}, "range": {"start": {"line": 0, "character": 0}, "end": {"line": 0, "character": 0}}}),
                version,
            ),
            LanguageEffectRequest::RequestDocumentSymbols { version, .. } => (
                "textDocument/documentSymbol",
                serde_json::json!({"textDocument": {"uri": self.active_document_uri()}}),
                version,
            ),
            LanguageEffectRequest::RequestWorkspaceSymbols { version, query } => (
                "workspace/symbol",
                serde_json::json!({"query": query}),
                version,
            ),
            LanguageEffectRequest::RequestFormatting { version, .. } => (
                "textDocument/formatting",
                serde_json::json!({"textDocument": {"uri": self.active_document_uri()}, "options": {"tabSize": 4, "insertSpaces": true}}),
                version,
            ),
            LanguageEffectRequest::RefreshDiagnostics { version, .. } => (
                "textDocument/diagnostic",
                serde_json::json!({"textDocument": {"uri": self.active_document_uri()}}),
                version,
            ),
            LanguageEffectRequest::RefreshSemanticTokens { version, .. } => (
                "textDocument/semanticTokens/full",
                serde_json::json!({"textDocument": {"uri": self.active_document_uri()}}),
                version,
            ),
            LanguageEffectRequest::RestartLanguageServer => {
                return self.apply_language_action(
                    app_ui::language::LanguageAction::RestartLanguageServer,
                );
            }
            LanguageEffectRequest::RefreshSyntax { .. } => {
                return Transition {
                    render: true,
                    ..Transition::default()
                };
            }
        };
        let Some(spec) = self.discovered_lsp_spec() else {
            self.language_server = LanguageServerStatus::Unavailable;
            return Transition {
                render: true,
                ..Transition::default()
            };
        };
        let request_id = RequestId(self.frame_number.saturating_add(1));
        self.apply_action(Action::RequestEffect(Effect::LspRequest {
            request: request_id,
            version,
            spec,
            method: method.to_owned(),
            params,
        }))
    }

    fn active_document_uri(&self) -> String {
        self.active_path.as_ref().map_or_else(
            || "untitled:editor".to_owned(),
            |path| format!("file://{}", path.to_string_lossy().replace('\\', "/")),
        )
    }

    fn discovered_lsp_spec(&self) -> Option<ProcessSpec> {
        let language = self
            .active_path
            .as_ref()
            .and_then(syntax_engine::SyntaxLanguage::from_path)?;
        let command = lsp_client::CommandSpec::discover_known(language.name())?;
        Some(ProcessSpec {
            executable: command.executable.to_string_lossy().into_owned(),
            arguments: command.args,
        })
    }

    /// Converts Git UI intents into structured, trust-gated process effects. Destructive actions
    /// that require a confirmation remain visible but are not executed by this adapter.
    #[allow(clippy::too_many_lines)]
    pub fn apply_git_action(&mut self, action: app_ui::git::GitAction) -> Transition {
        use app_ui::git::GitAction;
        if let Some(dashboard) = self.git_dashboard.as_mut() {
            let _ = dashboard.dispatch(action.clone());
        }
        let root = self.workspace_roots.first().cloned();
        let request = RequestId(self.frame_number.saturating_add(1));
        if let GitAction::RequestDiscard(plan) = &action {
            self.pending_git_discard = Some(plan.clone());
        }
        if matches!(&action, GitAction::ConfirmDiscard) {
            if let (Some(root), Some(plan)) = (root.clone(), self.pending_git_discard.take()) {
                return self.apply_action(Action::RequestEffect(Effect::GitDiscard {
                    request,
                    root,
                    plan,
                    confirmed: true,
                }));
            }
        }
        if matches!(&action, GitAction::CancelDiscard) {
            self.pending_git_discard = None;
        }
        if let GitAction::OpenDiffFile(path) | GitAction::OpenConflict(path) = &action {
            let target = if path.is_absolute() {
                path.clone()
            } else {
                root.as_ref()
                    .map_or_else(|| path.clone(), |repository| repository.join(path))
            };
            if let Err(error) = self.open_tab(target.clone()) {
                self.output.push(OutputMessage {
                    subsystem: "git".to_owned(),
                    operation: "open-diff".to_owned(),
                    level: OutputLevel::Warning,
                    message: format!("could not open {}: {error}", target.display()),
                });
            } else {
                self.output.push(OutputMessage {
                    subsystem: "git".to_owned(),
                    operation: "open-diff".to_owned(),
                    level: OutputLevel::Information,
                    message: format!("opened {}", target.display()),
                });
            }
            return Transition {
                render: true,
                ..Transition::default()
            };
        }
        if let GitAction::StageHunk { path, hunk } | GitAction::UnstageHunk { path, hunk } = &action
        {
            let reverse = matches!(&action, GitAction::UnstageHunk { .. });
            let hunk_value = self
                .git_dashboard
                .as_ref()
                .and_then(|dashboard| {
                    dashboard
                        .diff_files
                        .iter()
                        .find(|file| &file.path == path)
                        .and_then(|file| file.hunks.get(*hunk))
                })
                .cloned();
            if let (Some(root), Some(hunk_value)) = (root.clone(), hunk_value) {
                return self.apply_action(Action::RequestEffect(Effect::GitHunk {
                    request,
                    root,
                    hunk: hunk_value,
                    reverse,
                }));
            }
            self.output.push(OutputMessage {
                subsystem: "git".to_owned(),
                operation: "hunk".to_owned(),
                level: OutputLevel::Warning,
                message: format!("could not find hunk {hunk} for {}", path.display()),
            });
            return Transition {
                render: true,
                ..Transition::default()
            };
        }
        let args = match action {
            GitAction::StageFile(path) => Some(vec![
                "add".to_owned(),
                "--".to_owned(),
                path.display().to_string(),
            ]),
            GitAction::UnstageFile(path) => Some(vec![
                "restore".to_owned(),
                "--staged".to_owned(),
                "--".to_owned(),
                path.display().to_string(),
            ]),
            GitAction::Fetch => Some(vec!["fetch".to_owned()]),
            GitAction::Pull => Some(vec!["pull".to_owned()]),
            GitAction::Push => Some(vec!["push".to_owned()]),
            GitAction::CreateBranch(name) => Some(vec!["switch".to_owned(), "-c".to_owned(), name]),
            GitAction::SwitchBranch(name) => Some(vec!["switch".to_owned(), name]),
            GitAction::DeleteBranch { name, force } => Some(vec![
                "branch".to_owned(),
                if force { "-D" } else { "-d" }.to_owned(),
                name,
            ]),
            GitAction::CreateStash => Some(vec!["stash".to_owned(), "push".to_owned()]),
            GitAction::ApplyStash(reference) => {
                Some(vec!["stash".to_owned(), "apply".to_owned(), reference])
            }
            GitAction::PopStash(reference) => {
                Some(vec!["stash".to_owned(), "pop".to_owned(), reference])
            }
            GitAction::Commit => {
                let Some(form) = self
                    .git_dashboard
                    .as_ref()
                    .map(|dashboard| dashboard.commit_form.clone())
                else {
                    self.output.push(OutputMessage {
                        subsystem: "git".to_owned(),
                        operation: "commit".to_owned(),
                        level: OutputLevel::Warning,
                        message: "Git dashboard is not initialized".to_owned(),
                    });
                    return Transition {
                        render: true,
                        ..Transition::default()
                    };
                };
                if !form.can_submit || form.message.trim().is_empty() {
                    self.output.push(OutputMessage {
                        subsystem: "git".to_owned(),
                        operation: "commit".to_owned(),
                        level: OutputLevel::Warning,
                        message: "commit message is required".to_owned(),
                    });
                    return Transition {
                        render: true,
                        ..Transition::default()
                    };
                }
                let mut args = vec!["commit".to_owned(), "-m".to_owned(), form.message];
                if form.amend {
                    args.insert(1, "--amend".to_owned());
                }
                if form.allow_empty {
                    args.insert(1, "--allow-empty".to_owned());
                }
                Some(args)
            }
            GitAction::ConfirmDiscard => {
                self.output.push(OutputMessage {
                    subsystem: "git".to_owned(),
                    operation: "confirmation".to_owned(),
                    level: OutputLevel::Warning,
                    message: "this Git action requires a complete confirmation form".to_owned(),
                });
                None
            }
            GitAction::RequestDiscard(_)
            | GitAction::CancelDiscard
            | GitAction::SwitchView(_)
            | GitAction::ToggleChangeMode
            | GitAction::SelectChange(_)
            | GitAction::SelectHunk(_)
            | GitAction::EditCommitMessage(_)
            | GitAction::ToggleAmend
            | GitAction::ToggleAllowEmpty
            | GitAction::OpenDiffFile(_)
            | GitAction::OpenConflict(_)
            | GitAction::StageHunk { .. }
            | GitAction::UnstageHunk { .. } => None,
        };
        let Some((root, args)) = root.zip(args) else {
            return Transition {
                render: true,
                ..Transition::default()
            };
        };
        self.git_root = Some(root.clone());
        self.apply_action(Action::RequestEffect(Effect::ExternalProcess {
            request: RequestId(self.frame_number.saturating_add(1)),
            kind: super::effect::ExternalProcessKind::Git,
            spec: super::effect::ProcessSpec {
                executable: "git".to_owned(),
                arguments: {
                    let mut all = vec!["-C".to_owned(), root.display().to_string()];
                    all.extend(args);
                    all
                },
            },
        }))
    }

    /// Routes workspace UI actions through the root workspace/search services.
    pub fn apply_workspace_action(
        &mut self,
        action: app_ui::workspace::QuickOpenAction,
    ) -> Transition {
        let opened = self.workspace_ui.quick_open.apply_action(action);
        if let Some((disposition, path)) = opened {
            match disposition {
                app_ui::workspace::QuickOpenDisposition::CurrentEditor => {
                    return self.apply_action(Action::OpenPath(path));
                }
                app_ui::workspace::QuickOpenDisposition::OpenToSide => {
                    if let Err(error) = self.open_tab(path) {
                        self.output.push(OutputMessage {
                            subsystem: "workspace".to_owned(),
                            operation: "quick-open".to_owned(),
                            level: OutputLevel::Error,
                            message: error.to_string(),
                        });
                    }
                }
            }
        }
        Transition {
            render: true,
            ..Transition::default()
        }
    }

    /// Starts cancellable project search and streams results through typed events.
    pub fn start_workspace_search(
        &mut self,
        query: impl Into<String>,
        options: app_ui::workspace::SearchOptionsView,
    ) -> Transition {
        let query = query.into();
        let session_id = self.frame_number.saturating_add(1);
        let core_options = workspace_core::SearchOptions {
            roots: self.workspace_roots.clone(),
            pattern: query.clone(),
            literal: options.literal,
            case_sensitive: options.case_sensitive,
            whole_word: options.whole_word,
            max_results: options.max_results,
            ..workspace_core::SearchOptions::default()
        };
        self.workspace_ui
            .apply_event(app_ui::workspace::WorkspaceModelEvent::SearchStarted {
                session_id,
                query: query.clone(),
                options,
            });
        self.bottom_panel_view = BottomPanelView::Search;
        self.bottom_panel_visible = true;
        Transition {
            effects: vec![Effect::SearchWorkspace {
                session_id,
                options: core_options,
            }],
            render: true,
            ..Transition::default()
        }
    }

    /// Serializes all open documents into the crash-safe session schema. The caller owns the
    /// `RecoveryStore` and decides when to persist this snapshot (normally after each committed
    /// action and once more during shutdown).
    #[must_use]
    pub fn session_state(&self) -> config_core::SessionState {
        let editors = self
            .tabs
            .iter()
            .enumerate()
            .map(|(index, tab)| {
                let selection = tab
                    .buffer
                    .selections()
                    .selections()
                    .iter()
                    .map(|item| {
                        let anchor = tab
                            .buffer
                            .offset_to_position(item.anchor)
                            .unwrap_or_default();
                        let active = tab
                            .buffer
                            .offset_to_position(item.active)
                            .unwrap_or_default();
                        config_core::SelectionState {
                            anchor: config_core::CursorState {
                                line: anchor.line,
                                character: anchor.character,
                            },
                            active: config_core::CursorState {
                                line: active.line,
                                character: active.character,
                            },
                        }
                    })
                    .collect::<Vec<_>>();
                let primary = tab.buffer.selections().primary();
                let cursor = tab
                    .buffer
                    .offset_to_position(primary.active)
                    .unwrap_or_default();
                config_core::EditorSession {
                    editor_id: format!("tab-{index}"),
                    original_path: tab.path.clone(),
                    original_encoding: Some(workspace_encoding_to_config(&tab.encoding)),
                    original_bom: tab.with_bom,
                    original_line_ending: Some(workspace_line_endings_to_config(tab.line_endings)),
                    cursor: config_core::CursorState {
                        line: cursor.line,
                        character: cursor.character,
                    },
                    selections: selection,
                    unsaved_text: tab.buffer.is_dirty().then(|| tab.buffer.to_string()),
                    dirty: tab.buffer.is_dirty(),
                    unknown: std::collections::BTreeMap::new(),
                }
            })
            .collect::<Vec<_>>();
        let tab_order = editors
            .iter()
            .map(|editor| editor.editor_id.clone())
            .collect::<Vec<_>>();
        config_core::SessionState {
            workspace_roots: self.workspace_roots.clone(),
            editors,
            tab_order,
            split_layout: self.split_axis.map_or_else(
                || config_core::SplitLayout::Editor {
                    editor_id: format!("tab-{}", self.active_tab),
                },
                |axis| config_core::SplitLayout::Split {
                    axis: match axis {
                        app_ui::shell::SplitAxis::Horizontal => config_core::SplitAxis::Horizontal,
                        app_ui::shell::SplitAxis::Vertical => config_core::SplitAxis::Vertical,
                    },
                    ratio: f32::from(self.split_ratio_percent) / 100.0,
                    first: Box::new(config_core::SplitLayout::Editor {
                        editor_id: format!("tab-{}", self.active_tab),
                    }),
                    second: Box::new(config_core::SplitLayout::Editor {
                        editor_id: format!(
                            "tab-{}",
                            self.split_secondary_tab.unwrap_or(self.active_tab)
                        ),
                    }),
                },
            ),
            active_editor: Some(format!("tab-{}", self.active_tab)),
            ..config_core::SessionState::default()
        }
    }

    /// Restores every editor tab from a previously persisted session. Missing files are retained
    /// as in-memory tabs when the session contains unsaved text, preventing recovery data loss.
    #[allow(
        clippy::too_many_lines,
        clippy::single_match_else,
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss
    )]
    pub fn restore_session(&mut self, session: &config_core::SessionState) {
        self.workspace_roots.clone_from(&session.workspace_roots);
        if session.editors.is_empty() {
            return;
        }
        self.tabs.clear();
        let ordered_editors = if session.tab_order.is_empty() {
            session.editors.iter().collect::<Vec<_>>()
        } else {
            session
                .tab_order
                .iter()
                .filter_map(|id| {
                    session
                        .editors
                        .iter()
                        .find(|editor| &editor.editor_id == id)
                })
                .collect::<Vec<_>>()
        };
        for editor in &ordered_editors {
            let editor = *editor;
            let (path, disk_text, encoding, with_bom, line_endings) =
                if let Some(path) = &editor.original_path {
                    match workspace_core::load_text_document(
                        path,
                        &workspace_core::DocumentLoadOptions::default(),
                    ) {
                        Ok(document) => (
                            Some(path.clone()),
                            document.text,
                            editor
                                .original_encoding
                                .as_ref()
                                .map_or(document.encoding, config_encoding_to_workspace),
                            editor.original_bom || document.had_bom,
                            editor
                                .original_line_ending
                                .as_ref()
                                .map_or(document.line_endings, |value| {
                                    config_line_endings_to_workspace(*value)
                                }),
                        ),
                        Err(_) => (
                            Some(path.clone()),
                            String::new(),
                            editor.original_encoding.as_ref().map_or(
                                workspace_core::EncodingKind::Utf8,
                                config_encoding_to_workspace,
                            ),
                            editor.original_bom,
                            editor
                                .original_line_ending
                                .as_ref()
                                .map_or(workspace_core::LineEndings::Lf, |value| {
                                    config_line_endings_to_workspace(*value)
                                }),
                        ),
                    }
                } else {
                    (
                        None,
                        String::new(),
                        editor.original_encoding.as_ref().map_or(
                            workspace_core::EncodingKind::Utf8,
                            config_encoding_to_workspace,
                        ),
                        editor.original_bom,
                        editor
                            .original_line_ending
                            .as_ref()
                            .map_or(workspace_core::LineEndings::Lf, |value| {
                                config_line_endings_to_workspace(*value)
                            }),
                    )
                };
            let text = editor.unsaved_text.as_deref().unwrap_or(&disk_text);
            let mut buffer = TextBuffer::new(text);
            if editor.dirty {
                buffer.mark_recovered_dirty();
            }
            let selections = editor
                .selections
                .iter()
                .filter_map(|selection| {
                    let anchor = buffer
                        .position_to_offset(LogicalPosition {
                            line: selection.anchor.line,
                            character: selection.anchor.character,
                        })
                        .ok()?;
                    let active = buffer
                        .position_to_offset(LogicalPosition {
                            line: selection.active.line,
                            character: selection.active.character,
                        })
                        .ok()?;
                    Some(editor_core::Selection::new(anchor, active))
                })
                .collect::<Vec<_>>();
            if !selections.is_empty() {
                if let Ok(selection_set) = editor_core::SelectionSet::new(selections, 0) {
                    let _ = buffer.set_selections(selection_set);
                }
            } else if let Ok(position) = buffer.position_to_offset(LogicalPosition {
                line: editor.cursor.line,
                character: editor.cursor.character,
            }) {
                let _ = buffer.set_selections(editor_core::SelectionSet::single(
                    editor_core::Selection::cursor(position),
                ));
            }
            self.tabs.push(TabState {
                path,
                buffer,
                encoding,
                with_bom,
                line_endings,
            });
        }
        if self.tabs.is_empty() {
            return;
        }
        self.active_tab = session
            .active_editor
            .as_ref()
            .and_then(|active| {
                ordered_editors
                    .iter()
                    .position(|editor| &editor.editor_id == active)
            })
            .unwrap_or(0)
            .min(self.tabs.len().saturating_sub(1));
        self.split_axis = match &session.split_layout {
            config_core::SplitLayout::Split {
                axis,
                ratio,
                first,
                second,
            } => {
                let first_id = match first.as_ref() {
                    config_core::SplitLayout::Editor { editor_id } => Some(editor_id.as_str()),
                    _ => None,
                };
                let second_id = match second.as_ref() {
                    config_core::SplitLayout::Editor { editor_id } => Some(editor_id.as_str()),
                    _ => None,
                };
                let first_index = first_id.and_then(|id| {
                    ordered_editors
                        .iter()
                        .position(|editor| editor.editor_id == id)
                });
                let second_index = second_id.and_then(|id| {
                    ordered_editors
                        .iter()
                        .position(|editor| editor.editor_id == id)
                });
                self.split_secondary_tab = second_index;
                if first_index.is_some() && second_index.is_some() {
                    self.split_ratio_percent = (*ratio * 100.0).round().clamp(10.0, 90.0) as u16;
                    Some(match axis {
                        config_core::SplitAxis::Horizontal => app_ui::shell::SplitAxis::Horizontal,
                        config_core::SplitAxis::Vertical => app_ui::shell::SplitAxis::Vertical,
                    })
                } else {
                    self.split_secondary_tab = None;
                    None
                }
            }
            _ => {
                self.split_secondary_tab = None;
                None
            }
        };
        let active = self.tabs[self.active_tab].clone();
        self.active_path = active.path;
        self.buffer = active.buffer;
        self.sync_buffer_projection();
        self.refresh_explorer_entries();
    }

    /// Loads a startup path into the view model without launching external processes.
    ///
    /// Directories become workspace roots; regular files are decoded using the workspace document
    /// adapter and retained as editable text. Missing or malformed files are reported as structured
    /// output while the editor remains usable.
    pub fn open_startup_path(&mut self, path: impl AsRef<Path>) {
        let path = path.as_ref().to_path_buf();
        if path.is_dir() {
            self.active_path = Some(path);
            self.tabs = vec![TabState::untitled()];
            self.active_tab = 0;
            self.workspace_roots = self.active_path.iter().cloned().collect();
            self.refresh_workspace_trust();
            self.refresh_explorer_entries();
            self.active_text.clear();
            self.active_dirty = false;
            self.buffer = TextBuffer::default();
            return;
        }
        match workspace_core::load_text_document(
            &path,
            &workspace_core::DocumentLoadOptions::default(),
        ) {
            Ok(document) => {
                self.active_path = Some(document.path);
                self.tabs = vec![TabState {
                    path: self.active_path.clone(),
                    buffer: TextBuffer::new(&document.text),
                    encoding: document.encoding,
                    with_bom: document.had_bom,
                    line_endings: document.line_endings,
                }];
                self.active_tab = 0;
                self.workspace_roots = self
                    .active_path
                    .as_ref()
                    .and_then(|path| path.parent())
                    .map(|path| vec![path.to_path_buf()])
                    .unwrap_or_default();
                self.refresh_workspace_trust();
                self.refresh_explorer_entries();
                self.active_text = document.text;
                self.active_dirty = false;
                self.buffer = TextBuffer::new(&self.active_text);
            }
            Err(error) => self.output.push(OutputMessage {
                subsystem: "workspace".to_owned(),
                operation: "open-file".to_owned(),
                level: OutputLevel::Error,
                message: format!("could not open {}: {error}", path.display()),
            }),
        }
    }

    /// Opens a regular file as a new tab while preserving the current tab snapshot.
    ///
    /// # Errors
    ///
    /// Returns the typed workspace document error when the file cannot be decoded.
    pub fn open_tab(
        &mut self,
        path: impl AsRef<Path>,
    ) -> Result<(), workspace_core::DocumentOpenError> {
        let path = path.as_ref().to_path_buf();
        let document = workspace_core::load_text_document(
            &path,
            &workspace_core::DocumentLoadOptions::default(),
        )?;
        self.sync_active_tab();
        self.tabs.push(TabState {
            path: Some(document.path.clone()),
            buffer: TextBuffer::new(&document.text),
            encoding: document.encoding,
            with_bom: document.had_bom,
            line_endings: document.line_endings,
        });
        self.active_tab = self.tabs.len().saturating_sub(1);
        self.active_path = Some(document.path);
        self.active_text = document.text;
        self.active_dirty = false;
        self.buffer = self.tabs[self.active_tab].buffer.clone();
        self.refresh_explorer_entries();
        Ok(())
    }

    #[must_use]
    pub fn tab_count(&self) -> usize {
        self.tabs.len()
    }

    #[must_use]
    pub fn active_tab_index(&self) -> usize {
        self.active_tab
    }

    fn switch_tab(&mut self, index: usize) {
        if index >= self.tabs.len() || index == self.active_tab {
            return;
        }
        self.sync_active_tab();
        self.active_tab = index;
        let tab = self.tabs[index].clone();
        self.active_path = tab.path;
        self.buffer = tab.buffer;
        self.sync_buffer_projection();
    }

    fn sync_active_tab(&mut self) {
        if let Some(tab) = self.tabs.get_mut(self.active_tab) {
            tab.path.clone_from(&self.active_path);
            tab.buffer = self.buffer.clone();
        }
    }

    fn active_tab_state(&self) -> &TabState {
        &self.tabs[self.active_tab]
    }

    fn reopen_with_encoding(&mut self, encoding: &workspace_core::EncodingKind) -> Transition {
        let Some(path) = self.active_path.clone() else {
            self.output.push(OutputMessage {
                subsystem: "workspace".to_owned(),
                operation: "reopen-encoding".to_owned(),
                level: OutputLevel::Warning,
                message: "an active file is required to reopen with another encoding".to_owned(),
            });
            return Transition {
                render: true,
                ..Transition::default()
            };
        };
        if self.active_dirty {
            self.output.push(OutputMessage {
                subsystem: "workspace".to_owned(),
                operation: "reopen-encoding".to_owned(),
                level: OutputLevel::Warning,
                message: "save or discard unsaved changes before reopening with another encoding"
                    .to_owned(),
            });
            return Transition {
                render: true,
                ..Transition::default()
            };
        }
        match workspace_core::load_text_document_with_encoding(
            &path,
            encoding,
            workspace_core::DecodePolicy::Strict,
        ) {
            Ok(document) => {
                self.sync_active_tab();
                self.tabs[self.active_tab] = TabState {
                    path: Some(document.path.clone()),
                    buffer: TextBuffer::new(&document.text),
                    encoding: document.encoding,
                    with_bom: document.had_bom,
                    line_endings: document.line_endings,
                };
                self.active_path = Some(document.path);
                self.buffer = self.tabs[self.active_tab].buffer.clone();
                self.active_text = document.text;
                self.active_dirty = false;
                self.language_ui.set_document_version(0);
                self.syntax_snapshot = syntax_engine::SyntaxSnapshot::default();
                self.sync_buffer_projection();
                self.output.push(OutputMessage {
                    subsystem: "workspace".to_owned(),
                    operation: "reopen-encoding".to_owned(),
                    level: OutputLevel::Information,
                    message: format!("reopened as {}", encoding.canonical_name()),
                });
            }
            Err(error) => self.output.push(OutputMessage {
                subsystem: "workspace".to_owned(),
                operation: "reopen-encoding".to_owned(),
                level: OutputLevel::Error,
                message: format!(
                    "could not reopen {} as {}: {error}",
                    path.display(),
                    encoding.canonical_name()
                ),
            }),
        }
        Transition {
            render: true,
            ..Transition::default()
        }
    }

    fn refresh_workspace_trust(&mut self) {
        if let Some(root) = self.workspace_roots.first() {
            self.workspace_trusted = self
                .trust_store
                .state_for_path(root)
                .allows_external_processes();
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Transition {
    pub effects: Vec<Effect>,
    pub events: Vec<Event>,
    pub render: bool,
}

impl AppState {
    #[must_use]
    #[allow(clippy::too_many_lines, clippy::needless_return)]
    pub fn apply_action(&mut self, action: Action) -> Transition {
        let before_version = self.buffer.snapshot().version();
        let mut transition = self.apply_action_inner(action);
        if self.buffer.snapshot().version() != before_version {
            self.language_ui
                .set_document_version(self.buffer.snapshot().version());
            transition.effects.push(self.syntax_effect());
        }
        transition
    }

    fn syntax_effect(&self) -> Effect {
        let request = RequestId(self.frame_number.saturating_add(1));
        Effect::RefreshSyntax {
            request,
            document: self.document_id,
            version: self.buffer.snapshot().version(),
            path: self.active_path.clone(),
            text: self.active_text.clone(),
            large_file: self.buffer.is_large_file(),
        }
    }

    #[allow(clippy::too_many_lines, clippy::needless_return)]
    fn apply_action_inner(&mut self, action: Action) -> Transition {
        match action {
            Action::Quit => {
                if self.active_dirty {
                    self.output.push(OutputMessage {
                        subsystem: "editor".to_owned(),
                        operation: "quit".to_owned(),
                        level: OutputLevel::Warning,
                        message: "unsaved changes prevent quitting; save or discard them first"
                            .to_owned(),
                    });
                } else {
                    self.running = false;
                }
                Transition {
                    render: true,
                    ..Transition::default()
                }
            }
            Action::SetWorkspaceTrust(trusted) => {
                self.workspace_trusted = trusted;
                if let Some(root) = self.workspace_roots.first() {
                    self.trust_store.set_state(
                        root,
                        if trusted {
                            workspace_core::TrustState::Trusted
                        } else {
                            workspace_core::TrustState::Untrusted
                        },
                    );
                }
                if trusted {
                    if let Some(path) = self.active_path.as_ref().filter(|path| path.is_file()) {
                        if let Some(language) = syntax_engine::SyntaxLanguage::from_path(path) {
                            if let Some(command) =
                                lsp_client::CommandSpec::discover_known(language.name())
                            {
                                let request = RequestId(self.frame_number.saturating_add(1));
                                self.pending_processes.insert(
                                    request,
                                    super::effect::ExternalProcessKind::LanguageServer,
                                );
                                self.language_server = LanguageServerStatus::Starting;
                                return Transition {
                                    effects: vec![Effect::ExternalProcess {
                                        request,
                                        kind: super::effect::ExternalProcessKind::LanguageServer,
                                        spec: super::effect::ProcessSpec {
                                            executable: command
                                                .executable
                                                .to_string_lossy()
                                                .into_owned(),
                                            arguments: command.args,
                                        },
                                    }],
                                    render: true,
                                    ..Transition::default()
                                };
                            }
                        }
                        self.language_server = LanguageServerStatus::Unavailable;
                    }
                }
                Transition {
                    render: true,
                    ..Transition::default()
                }
            }
            Action::OpenPath(path) => {
                if self.active_path.is_some() && path.is_file() {
                    if let Err(error) = self.open_tab(&path) {
                        self.output.push(OutputMessage {
                            subsystem: "workspace".to_owned(),
                            operation: "open-tab".to_owned(),
                            level: OutputLevel::Error,
                            message: error.to_string(),
                        });
                    }
                } else {
                    self.open_startup_path(path);
                }
                self.explorer_refresh_transition()
            }
            Action::ReopenWithEncoding(encoding) => self.reopen_with_encoding(&encoding),
            Action::SetEncoding { encoding, with_bom } => {
                self.sync_active_tab();
                if let Some(tab) = self.tabs.get_mut(self.active_tab) {
                    tab.encoding = encoding;
                    tab.with_bom = with_bom;
                }
                self.buffer.mark_recovered_dirty();
                self.sync_buffer_projection();
                self.output.push(OutputMessage {
                    subsystem: "workspace".to_owned(),
                    operation: "encoding".to_owned(),
                    level: OutputLevel::Information,
                    message: format!(
                        "next save uses {}{}",
                        self.active_tab_state().encoding.canonical_name(),
                        if with_bom { " with BOM" } else { "" }
                    ),
                });
                Transition {
                    render: true,
                    ..Transition::default()
                }
            }
            Action::SwitchTab(index) => {
                self.switch_tab(index);
                Transition {
                    render: true,
                    ..Transition::default()
                }
            }
            Action::CloseTab(index) => {
                if let Some(tab) = self.tabs.get(index) {
                    if tab.buffer.is_dirty() {
                        self.output.push(OutputMessage {
                            subsystem: "editor".to_owned(),
                            operation: "close-tab".to_owned(),
                            level: OutputLevel::Warning,
                            message: "unsaved changes prevent closing this tab".to_owned(),
                        });
                    } else if self.tabs.len() > 1 {
                        self.tabs.remove(index);
                        if self.active_tab >= self.tabs.len() {
                            self.active_tab = self.tabs.len().saturating_sub(1);
                        } else if index < self.active_tab {
                            self.active_tab = self.active_tab.saturating_sub(1);
                        }
                        let tab = self.tabs[self.active_tab].clone();
                        self.active_path = tab.path;
                        self.buffer = tab.buffer;
                        self.sync_buffer_projection();
                    } else {
                        self.tabs[0] = TabState::untitled();
                        self.active_tab = 0;
                        self.active_path = None;
                        self.buffer = TextBuffer::default();
                        self.sync_buffer_projection();
                    }
                }
                Transition {
                    render: true,
                    ..Transition::default()
                }
            }
            Action::SaveAs(path) => Transition {
                effects: vec![Effect::SaveDocumentAs {
                    path,
                    text: self.active_text.clone(),
                    encoding: self.active_tab_state().encoding.clone(),
                    with_bom: self.active_tab_state().with_bom,
                    line_endings: self.active_tab_state().line_endings,
                }],
                render: true,
                ..Transition::default()
            },
            Action::SplitPane {
                axis,
                ratio_percent,
            } => {
                self.split_axis = Some(axis);
                self.split_ratio_percent = ratio_percent.clamp(10, 90);
                self.split_secondary_tab = Some(
                    self.split_secondary_tab
                        .unwrap_or(self.active_tab)
                        .min(self.tabs.len().saturating_sub(1)),
                );
                Transition {
                    render: true,
                    ..Transition::default()
                }
            }
            Action::CloseSplit => {
                self.split_axis = None;
                self.split_secondary_tab = None;
                Transition {
                    render: true,
                    ..Transition::default()
                }
            }
            Action::SetSplitRatio(ratio_percent) => {
                if self.split_axis.is_some() {
                    self.split_ratio_percent = ratio_percent.clamp(10, 90);
                }
                Transition {
                    render: true,
                    ..Transition::default()
                }
            }
            Action::AddWorkspaceRoot(path) => {
                if path.is_dir() {
                    if !self.workspace_roots.iter().any(|root| root == &path) {
                        self.workspace_roots.push(path);
                        return self.explorer_refresh_transition();
                    }
                } else {
                    self.output.push(OutputMessage {
                        subsystem: "workspace".to_owned(),
                        operation: "add-root".to_owned(),
                        level: OutputLevel::Error,
                        message: format!("workspace root is not a directory: {}", path.display()),
                    });
                }
                Transition {
                    render: true,
                    ..Transition::default()
                }
            }
            Action::ApplyReplacementPlan(plan) => Transition {
                effects: vec![Effect::ApplyReplacementPlan {
                    request: RequestId(self.frame_number.saturating_add(1)),
                    plan,
                }],
                render: true,
                ..Transition::default()
            },
            Action::QuickOpen(action) => self.apply_workspace_action(action),
            Action::StartSearch { query, options } => self.start_workspace_search(query, options),
            Action::CancelSearch(session_id) => Transition {
                effects: vec![Effect::CancelSearch { session_id }],
                render: true,
                ..Transition::default()
            },
            Action::RequestEffect(effect)
                if effect.requires_trusted_workspace() && !self.workspace_trusted =>
            {
                let (request, kind) = match &effect {
                    Effect::ExternalProcess { request, kind, .. } => (*request, *kind),
                    Effect::RefreshGitStatus { request, .. }
                    | Effect::GitHunk { request, .. }
                    | Effect::GitDiscard { request, .. } => {
                        (*request, super::effect::ExternalProcessKind::Git)
                    }
                    Effect::LspRequest { request, .. } => {
                        (*request, super::effect::ExternalProcessKind::LanguageServer)
                    }
                    Effect::SaveDocument { .. }
                    | Effect::SaveDocumentAs { .. }
                    | Effect::RefreshExplorer { .. }
                    | Effect::ClipboardWrite { .. }
                    | Effect::ClipboardRead { .. }
                    | Effect::FormatDocument { .. }
                    | Effect::ApplyReplacementPlan { .. }
                    | Effect::RefreshSyntax { .. }
                    | Effect::SearchWorkspace { .. }
                    | Effect::CancelSearch { .. }
                    | Effect::Render => {
                        unreachable!("unguarded effect")
                    }
                };
                Transition {
                    events: vec![Event::ExternalProcessBlocked { request, kind }],
                    render: true,
                    ..Transition::default()
                }
            }
            Action::RequestEffect(effect) => {
                match &effect {
                    Effect::ExternalProcess { request, kind, .. } => {
                        self.pending_processes.insert(*request, *kind);
                    }
                    Effect::RefreshGitStatus { request, .. }
                    | Effect::GitHunk { request, .. }
                    | Effect::GitDiscard { request, .. } => {
                        self.pending_processes
                            .insert(*request, super::effect::ExternalProcessKind::Git);
                    }
                    Effect::SaveDocument { .. }
                    | Effect::SaveDocumentAs { .. }
                    | Effect::RefreshExplorer { .. }
                    | Effect::ClipboardWrite { .. }
                    | Effect::ClipboardRead { .. }
                    | Effect::FormatDocument { .. }
                    | Effect::ApplyReplacementPlan { .. }
                    | Effect::RefreshSyntax { .. }
                    | Effect::SearchWorkspace { .. }
                    | Effect::CancelSearch { .. }
                    | Effect::LspRequest { .. }
                    | Effect::Render => {}
                }
                Transition {
                    effects: vec![effect],
                    ..Transition::default()
                }
            }
            Action::Input(InputEvent::Key(key))
                if key.modifiers.contains(Modifier::Control)
                    && matches!(key.code, KeyCode::Character('c' | 'x' | 'v'))
                    && (!self.buffer.selections().primary().is_cursor()
                        || matches!(key.code, KeyCode::Character('x' | 'v'))) =>
            {
                let command = match key.code {
                    KeyCode::Character('c') => "editor.copy",
                    KeyCode::Character('x') => "editor.cut",
                    KeyCode::Character('v') => "editor.paste",
                    _ => unreachable!("clipboard shortcut guard"),
                };
                let effect = self.apply_command(command);
                return Transition {
                    effects: effect.into_iter().collect(),
                    render: true,
                    ..Transition::default()
                };
            }
            Action::Input(InputEvent::Key(key))
                if matches!(key.code, KeyCode::Character('c' | 'q'))
                    && key.modifiers.contains(Modifier::Control) =>
            {
                if self.active_dirty {
                    self.output.push(OutputMessage {
                        subsystem: "editor".to_owned(),
                        operation: "quit".to_owned(),
                        level: OutputLevel::Warning,
                        message: "unsaved changes prevent quitting; save or discard them first"
                            .to_owned(),
                    });
                } else {
                    self.running = false;
                }
                Transition {
                    render: true,
                    ..Transition::default()
                }
            }
            Action::Input(input) => {
                self.apply_input(input);
                Transition {
                    render: true,
                    ..Transition::default()
                }
            }
            Action::Invoke(command) => {
                let effect = self.apply_command(command.as_str());
                Transition {
                    effects: effect.into_iter().collect(),
                    render: true,
                    ..Transition::default()
                }
            }
        }
    }

    #[allow(
        clippy::too_many_lines,
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss
    )]
    fn apply_input(&mut self, input: InputEvent) {
        if let InputEvent::Key(ref key) = input {
            if key.code == KeyCode::Character('p') && key.modifiers.contains(Modifier::Control) {
                self.palette_visible = !self.palette_visible;
                if !self.palette_visible {
                    self.palette.clear_query();
                }
                return;
            }
            if key.code == KeyCode::Character('b') && key.modifiers.contains(Modifier::Control) {
                self.explorer_visible = !self.explorer_visible;
                return;
            }
            if key.code == KeyCode::Character('j') && key.modifiers.contains(Modifier::Control) {
                self.bottom_panel_visible = !self.bottom_panel_visible;
                return;
            }
            if self.palette_visible {
                match key.code {
                    KeyCode::Character(ch) if !key.modifiers.contains(Modifier::Control) => {
                        self.palette.push_query(ch);
                    }
                    KeyCode::Backspace => {
                        self.palette.pop_query();
                    }
                    KeyCode::Up => self.palette.move_selection(-1),
                    KeyCode::Down => self.palette.move_selection(1),
                    KeyCode::Enter => {
                        if let Some(command) = self.palette.activate_selected() {
                            let _ = self.apply_command(command.as_str());
                            self.palette_visible = false;
                            self.palette.clear_query();
                        }
                    }
                    KeyCode::Escape => {
                        self.palette_visible = false;
                        self.palette.clear_query();
                    }
                    _ => {}
                }
                return;
            }
        }
        let before_version = self.buffer.snapshot().version();
        let result = match input {
            InputEvent::Paste(text) => self
                .buffer
                .smart_insert(&text, &PairConfig::common_defaults())
                .map(|_| ()),
            InputEvent::Key(key) => match key.code {
                KeyCode::Character(character)
                    if !key.modifiers.contains(Modifier::Control)
                        && !key.modifiers.contains(Modifier::Alt)
                        && !key.modifiers.contains(Modifier::Meta) =>
                {
                    self.buffer
                        .smart_insert(&character.to_string(), &PairConfig::common_defaults())
                        .map(|_| ())
                }
                KeyCode::Enter => self
                    .buffer
                    .smart_enter(
                        &PairConfig::common_defaults(),
                        editor_core::IndentStyle::Spaces(4),
                    )
                    .map(|_| ()),
                KeyCode::Backspace => self.buffer.smart_backspace().map(|_| ()),
                KeyCode::Delete => self.delete_forward(),
                KeyCode::Left => self
                    .buffer
                    .move_left(key.modifiers.contains(Modifier::Shift)),
                KeyCode::Right => self
                    .buffer
                    .move_right(key.modifiers.contains(Modifier::Shift)),
                KeyCode::Up => {
                    self.buffer
                        .move_vertical(-1, key.modifiers.contains(Modifier::Shift), 4)
                }
                KeyCode::Down => {
                    self.buffer
                        .move_vertical(1, key.modifiers.contains(Modifier::Shift), 4)
                }
                KeyCode::Character('z') if key.modifiers.contains(Modifier::Control) => {
                    self.buffer.undo().map(|_| ())
                }
                KeyCode::Character('y') if key.modifiers.contains(Modifier::Control) => {
                    self.buffer.redo().map(|_| ())
                }
                _ => Ok(()),
            },
            InputEvent::Mouse(mouse) => {
                if mouse.position.row == 0
                    && matches!(mouse.action, MouseAction::Down(MouseButton::Left))
                {
                    let index = usize::from(mouse.position.column / 20);
                    self.switch_tab(index);
                    return;
                }
                if matches!(mouse.action, MouseAction::Drag(MouseButton::Left)) {
                    if let Some(axis) = self.split_axis {
                        let (coordinate, total) = match axis {
                            app_ui::shell::SplitAxis::Vertical => (
                                mouse
                                    .position
                                    .column
                                    .saturating_sub(if self.explorer_visible { 25 } else { 1 }),
                                80_u16.saturating_sub(if self.explorer_visible { 25 } else { 1 }),
                            ),
                            app_ui::shell::SplitAxis::Horizontal => {
                                (mouse.position.row.saturating_sub(1), 23)
                            }
                        };
                        if total > 0 {
                            self.split_ratio_percent = (u32::from(coordinate)
                                .saturating_mul(100)
                                .checked_div(u32::from(total))
                                .unwrap_or(50)
                                .clamp(10, 90))
                                as u16;
                            return;
                        }
                    }
                }
                if let Some(offset) = self.mouse_offset(mouse.position.row, mouse.position.column) {
                    match mouse.action {
                        MouseAction::Down(MouseButton::Left) => {
                            self.mouse_anchor = Some(offset);
                            let _ = self
                                .buffer
                                .set_selections(editor_core::SelectionSet::single(
                                    editor_core::Selection::cursor(offset),
                                ));
                        }
                        MouseAction::Drag(MouseButton::Left) => {
                            let anchor = self.mouse_anchor.unwrap_or(offset);
                            let _ = self
                                .buffer
                                .set_selections(editor_core::SelectionSet::single(
                                    editor_core::Selection::new(anchor, offset),
                                ));
                        }
                        MouseAction::Up(MouseButton::Left) => self.mouse_anchor = None,
                        _ => {}
                    }
                }
                Ok(())
            }
            InputEvent::Resize { .. } => Ok(()),
        };
        if let Err(error) = result {
            self.output.push(OutputMessage {
                subsystem: "editor".to_owned(),
                operation: "input".to_owned(),
                level: OutputLevel::Error,
                message: error.to_string(),
            });
        }
        if self.buffer.snapshot().version() != before_version {
            self.diagnostics.clear();
        }
        self.sync_buffer_projection();
    }

    fn delete_forward(&mut self) -> editor_core::Result<()> {
        let selections = self.buffer.selections().clone();
        let mut edits = Vec::new();
        for selection in selections.selections() {
            let range = selection.range();
            if !selection.is_cursor() {
                edits.push(editor_core::Edit::delete(range));
            } else if range.end.0 < self.buffer.len_chars() {
                edits.push(editor_core::Edit::delete(editor_types::TextRange {
                    start: range.end,
                    end: editor_types::CharacterOffset(range.end.0 + 1),
                }));
            }
        }
        if edits.is_empty() {
            return Ok(());
        }
        let transaction = editor_core::Transaction::new(edits)?;
        self.buffer.apply_transaction(transaction).map(|_| ())
    }

    fn start_format(&mut self, save_after: bool) -> Option<Effect> {
        if !self.workspace_trusted {
            return None;
        }
        let spec = self.external_formatter.clone()?;
        let request = RequestId(self.frame_number.saturating_add(1));
        let version = self.buffer.snapshot().version();
        self.pending_format.insert(
            request,
            PendingFormat {
                version,
                save_after,
            },
        );
        Some(Effect::FormatDocument {
            request,
            text: self.active_text.clone(),
            spec,
            timeout_ms: 5_000,
        })
    }

    fn start_lsp_format(&mut self, save_after: bool) -> Option<Effect> {
        if !self.workspace_trusted
            || !matches!(self.language_server, LanguageServerStatus::Running { .. })
        {
            return None;
        }
        let spec = self.discovered_lsp_spec()?;
        let request = RequestId(self.frame_number.saturating_add(1));
        let version = self.buffer.snapshot().version();
        self.pending_lsp_format.insert(
            request,
            PendingFormat {
                version,
                save_after,
            },
        );
        Some(Effect::LspRequest {
            request,
            version,
            spec,
            method: String::from("textDocument/formatting"),
            params: serde_json::json!({
                "textDocument": {"uri": self.active_document_uri()},
                "options": {"tabSize": 4, "insertSpaces": true}
            }),
        })
    }

    #[allow(clippy::too_many_lines)]
    fn apply_lsp_format_result(&mut self, request: RequestId, result: &serde_json::Value) {
        let Some(pending) = self.pending_lsp_format.remove(&request) else {
            return;
        };
        if pending.version != self.buffer.snapshot().version() {
            return;
        }
        let edits = result.as_array().cloned().unwrap_or_default();
        let text = self.buffer.snapshot().text().to_owned();
        let mut converted = Vec::with_capacity(edits.len());
        for edit in edits {
            let Some(range) = edit.get("range") else {
                self.output.push(OutputMessage {
                    subsystem: "lsp".to_owned(),
                    operation: "formatting".to_owned(),
                    level: OutputLevel::Error,
                    message: String::from("formatting response omitted an edit range"),
                });
                return;
            };
            let Ok(range) = serde_json::from_value::<lsp_client::protocol::Range>(range.clone())
            else {
                self.output.push(OutputMessage {
                    subsystem: "lsp".to_owned(),
                    operation: "formatting".to_owned(),
                    level: OutputLevel::Error,
                    message: String::from("formatting response contained an invalid range"),
                });
                return;
            };
            let Ok(start_position) = lsp_client::protocol::lsp_position_to_editor(
                &text,
                range.start,
                self.lsp_position_encoding,
            ) else {
                self.output.push(OutputMessage {
                    subsystem: "lsp".to_owned(),
                    operation: "formatting".to_owned(),
                    level: OutputLevel::Error,
                    message: String::from("formatting response range could not be mapped"),
                });
                return;
            };
            let Ok(start) = self.buffer.position_to_offset(start_position) else {
                self.output.push(OutputMessage {
                    subsystem: "lsp".to_owned(),
                    operation: "formatting".to_owned(),
                    level: OutputLevel::Error,
                    message: String::from("formatting response range could not be mapped"),
                });
                return;
            };
            let Ok(end_position) = lsp_client::protocol::lsp_position_to_editor(
                &text,
                range.end,
                self.lsp_position_encoding,
            ) else {
                self.output.push(OutputMessage {
                    subsystem: "lsp".to_owned(),
                    operation: "formatting".to_owned(),
                    level: OutputLevel::Error,
                    message: String::from("formatting response range could not be mapped"),
                });
                return;
            };
            let Ok(end) = self.buffer.position_to_offset(end_position) else {
                self.output.push(OutputMessage {
                    subsystem: "lsp".to_owned(),
                    operation: "formatting".to_owned(),
                    level: OutputLevel::Error,
                    message: String::from("formatting response range could not be mapped"),
                });
                return;
            };
            let Some(new_text) = edit.get("newText").and_then(serde_json::Value::as_str) else {
                self.output.push(OutputMessage {
                    subsystem: "lsp".to_owned(),
                    operation: "formatting".to_owned(),
                    level: OutputLevel::Error,
                    message: String::from("formatting response omitted replacement text"),
                });
                return;
            };
            converted.push(Edit::replace(
                editor_types::TextRange { start, end },
                new_text,
            ));
        }
        let changed = if converted.is_empty() {
            false
        } else {
            match Transaction::new(converted).and_then(|transaction| {
                self.buffer
                    .apply_transaction(transaction)
                    .map(|applied| applied.changed)
            }) {
                Ok(changed) => changed,
                Err(error) => {
                    self.output.push(OutputMessage {
                        subsystem: "lsp".to_owned(),
                        operation: "formatting".to_owned(),
                        level: OutputLevel::Error,
                        message: format!("could not apply formatting edits: {error}"),
                    });
                    false
                }
            }
        };
        if changed {
            self.language_ui
                .set_document_version(self.buffer.snapshot().version());
            self.sync_buffer_projection();
            self.deferred_effects.push(self.syntax_effect());
        }
        if pending.save_after {
            self.queue_save_after_format();
        }
    }

    fn queue_save_after_format(&mut self) {
        if let Some(path) = self.active_path.clone() {
            let (encoding, with_bom, line_endings) = {
                let tab = self.active_tab_state();
                (tab.encoding.clone(), tab.with_bom, tab.line_endings)
            };
            self.deferred_effects.push(Effect::SaveDocument {
                path,
                text: self.active_text.clone(),
                encoding,
                with_bom,
                line_endings,
            });
        }
    }

    #[allow(clippy::too_many_lines, clippy::needless_return)]
    fn apply_command(&mut self, command: &str) -> Option<Effect> {
        match command {
            "editor.undo" => {
                let _ = self.buffer.undo();
            }
            "editor.redo" => {
                let _ = self.buffer.redo();
            }
            "editor.quit" if !self.active_dirty => self.running = false,
            "editor.save" if self.active_dirty => {
                if self.format_on_save {
                    if let Some(effect) = self.start_lsp_format(true) {
                        return Some(effect);
                    }
                    if let Some(effect) = self.start_format(true) {
                        return Some(effect);
                    }
                }
                if let Some(path) = self.active_path.clone() {
                    return Some(Effect::SaveDocument {
                        path,
                        text: self.active_text.clone(),
                        encoding: self.active_tab_state().encoding.clone(),
                        with_bom: self.active_tab_state().with_bom,
                        line_endings: self.active_tab_state().line_endings,
                    });
                }
                self.output.push(OutputMessage {
                    subsystem: "workspace".to_owned(),
                    operation: "save-file".to_owned(),
                    level: OutputLevel::Warning,
                    message: "untitled buffer requires Save As before it can be written".to_owned(),
                });
            }
            "editor.save" => {}
            "editor.copy" | "editor.cut" => {
                let selection = self.buffer.selections().primary();
                let range = selection.range();
                if selection.is_cursor() {
                    self.output.push(OutputMessage {
                        subsystem: "editor".to_owned(),
                        operation: command.to_owned(),
                        level: OutputLevel::Information,
                        message: "no selection to copy".to_owned(),
                    });
                } else if let Ok(text) = self.buffer.snapshot().text_in_range(range) {
                    let cut = command == "editor.cut";
                    let request = self.next_clipboard_request();
                    if cut {
                        self.pending_clipboard.insert(
                            request,
                            PendingClipboard::Cut {
                                range,
                                expected: text.to_owned(),
                            },
                        );
                    }
                    return Some(Effect::ClipboardWrite {
                        request,
                        text: text.to_owned(),
                        cut,
                    });
                }
            }
            "editor.paste" => {
                let request = self.next_clipboard_request();
                self.pending_clipboard.insert(
                    request,
                    PendingClipboard::Paste {
                        range: self.buffer.selections().primary().range(),
                    },
                );
                return Some(Effect::ClipboardRead { request });
            }
            "editor.format" => {
                if !self.workspace_trusted {
                    self.output.push(OutputMessage {
                        subsystem: "workspace-trust".to_owned(),
                        operation: "format".to_owned(),
                        level: OutputLevel::Warning,
                        message: "trust the workspace before running an external formatter"
                            .to_owned(),
                    });
                    self.sync_buffer_projection();
                    return None;
                }
                if let Some(effect) = self.start_lsp_format(false) {
                    return Some(effect);
                }
                if let Some(effect) = self.start_format(false) {
                    return Some(effect);
                }
                let Some(_spec) = self.external_formatter.clone() else {
                    self.output.push(OutputMessage {
                        subsystem: "editor".to_owned(),
                        operation: "format".to_owned(),
                        level: OutputLevel::Information,
                        message: "no language-server or external formatter is configured"
                            .to_owned(),
                    });
                    self.sync_buffer_projection();
                    return None;
                };
                return None;
            }
            "editor.reopenUtf8" => {
                let _ = self.reopen_with_encoding(&workspace_core::EncodingKind::Utf8);
            }
            "editor.reopenUtf16Le" => {
                let _ = self.reopen_with_encoding(&workspace_core::EncodingKind::Utf16Le);
            }
            "editor.reopenUtf16Be" => {
                let _ = self.reopen_with_encoding(&workspace_core::EncodingKind::Utf16Be);
            }
            "editor.convertUtf8" => {
                self.apply_action_inner(Action::SetEncoding {
                    encoding: workspace_core::EncodingKind::Utf8,
                    with_bom: false,
                });
            }
            "editor.convertUtf16Le" => {
                self.apply_action_inner(Action::SetEncoding {
                    encoding: workspace_core::EncodingKind::Utf16Le,
                    with_bom: true,
                });
            }
            "editor.convertUtf16Be" => {
                self.apply_action_inner(Action::SetEncoding {
                    encoding: workspace_core::EncodingKind::Utf16Be,
                    with_bom: true,
                });
            }
            "editor.toggleFormatOnSave" => {
                self.format_on_save = !self.format_on_save;
                self.output.push(OutputMessage {
                    subsystem: "settings".to_owned(),
                    operation: "format-on-save".to_owned(),
                    level: OutputLevel::Information,
                    message: format!("format on save: {}", self.format_on_save),
                });
            }
            "editor.toggleFormatOnPaste" => {
                self.format_on_paste = !self.format_on_paste;
                self.output.push(OutputMessage {
                    subsystem: "settings".to_owned(),
                    operation: "format-on-paste".to_owned(),
                    level: OutputLevel::Information,
                    message: format!("format on paste: {}", self.format_on_paste),
                });
            }
            "workbench.splitVertical" => {
                self.split_axis = Some(app_ui::shell::SplitAxis::Vertical);
                self.split_ratio_percent = 50;
                self.split_secondary_tab = Some(self.active_tab);
            }
            "workbench.splitHorizontal" => {
                self.split_axis = Some(app_ui::shell::SplitAxis::Horizontal);
                self.split_ratio_percent = 50;
                self.split_secondary_tab = Some(self.active_tab);
            }
            "workbench.closeSplit" => {
                self.split_axis = None;
                self.split_secondary_tab = None;
            }
            "workbench.showProblems" => {
                self.bottom_panel_view = BottomPanelView::Problems;
                self.bottom_panel_visible = true;
            }
            "workbench.showGit" => {
                self.bottom_panel_view = BottomPanelView::Git;
                self.bottom_panel_visible = true;
            }
            "git.showChanges" => self.show_git_view(app_ui::git::GitView::Changes),
            "git.showDiff" => self.show_git_view(app_ui::git::GitView::Diff),
            "git.showBranches" => self.show_git_view(app_ui::git::GitView::Branches),
            "git.showStashes" => self.show_git_view(app_ui::git::GitView::Stashes),
            "git.showHistory" => self.show_git_view(app_ui::git::GitView::History),
            "git.showCommit" => self.show_git_view(app_ui::git::GitView::Commit),
            "git.showConflicts" => self.show_git_view(app_ui::git::GitView::Conflicts),
            "workbench.showOutput" => {
                self.bottom_panel_view = BottomPanelView::Output;
                self.bottom_panel_visible = true;
            }
            "git.refresh" if self.workspace_trusted => {
                if let Some(root) = self.workspace_roots.first().cloned() {
                    let request = RequestId(self.frame_number.saturating_add(1));
                    return Some(Effect::RefreshGitStatus { request, root });
                }
            }
            "git.refresh" => {
                self.output.push(OutputMessage {
                    subsystem: "workspace-trust".to_owned(),
                    operation: "git-status".to_owned(),
                    level: OutputLevel::Warning,
                    message: "trust the workspace before running Git operations".to_owned(),
                });
            }
            unknown => {
                self.output.push(OutputMessage {
                    subsystem: "editor".to_owned(),
                    operation: "command".to_owned(),
                    level: OutputLevel::Warning,
                    message: format!("command `{unknown}` is not available in this build"),
                });
            }
        }
        self.sync_buffer_projection();
        None
    }

    fn sync_buffer_projection(&mut self) {
        self.active_text = self.buffer.to_string();
        self.active_dirty = self.buffer.is_dirty();
        self.sync_active_tab();
    }

    fn show_git_view(&mut self, view: app_ui::git::GitView) {
        self.bottom_panel_view = BottomPanelView::Git;
        self.bottom_panel_visible = true;
        if let Some(dashboard) = self.git_dashboard.as_mut() {
            dashboard.view = view;
        }
    }

    fn next_clipboard_request(&self) -> RequestId {
        RequestId(
            self.frame_number
                .saturating_add(1)
                .saturating_add(self.pending_clipboard.len() as u64),
        )
    }

    fn refresh_explorer_entries(&mut self) {
        let mut entries = Vec::new();
        for root in &self.workspace_roots {
            let children = match workspace_core::canonicalize_path(root) {
                Ok(canonical) => {
                    workspace_core::ExplorerTree::new(vec![canonical.clone()], Vec::new())
                        .children(canonical.as_path())
                }
                Err(error) => {
                    self.output.push(OutputMessage {
                        subsystem: "workspace".to_owned(),
                        operation: "explorer".to_owned(),
                        level: OutputLevel::Warning,
                        message: error.to_string(),
                    });
                    continue;
                }
            };
            match children {
                Ok(children) => {
                    entries.extend(children.into_iter().map(|entry| ExplorerProjection {
                        depth: u8::try_from(entry.depth).unwrap_or(u8::MAX),
                        label: entry.path.file_name().map_or_else(
                            || entry.path.display().to_string(),
                            |name| name.to_string_lossy().into_owned(),
                        ),
                        active: self.active_path.as_ref() == Some(&entry.path),
                        expanded: false,
                    }));
                }
                Err(error) => self.output.push(OutputMessage {
                    subsystem: "workspace".to_owned(),
                    operation: "explorer".to_owned(),
                    level: OutputLevel::Warning,
                    message: error.to_string(),
                }),
            }
        }
        self.explorer_entries = entries;
    }

    fn explorer_refresh_transition(&mut self) -> Transition {
        let request = RequestId(self.frame_number.saturating_add(1));
        self.latest_explorer_request = Some(request);
        Transition {
            effects: vec![Effect::RefreshExplorer {
                request,
                roots: self.workspace_roots.clone(),
            }],
            render: true,
            ..Transition::default()
        }
    }

    fn mouse_offset(&self, screen_row: u16, screen_column: u16) -> Option<CharacterOffset> {
        // The shell reserves one row for tabs and a narrow Explorer gutter. On compact layouts
        // the subtraction saturates, keeping clicks within the first document line.
        let line = u32::from(screen_row.saturating_sub(2));
        let column = u32::from(if self.explorer_visible {
            screen_column.saturating_sub(25)
        } else {
            screen_column.saturating_sub(1)
        });
        self.buffer
            .position_to_offset(LogicalPosition {
                line,
                character: column,
            })
            .ok()
    }

    #[allow(clippy::too_many_lines)]
    pub fn apply_event(&mut self, event: Event) {
        match event {
            Event::DocumentSaved { path } => {
                if self.active_path.as_ref() == Some(&path) {
                    self.buffer.mark_saved();
                    self.sync_buffer_projection();
                }
                self.output.push(OutputMessage {
                    subsystem: "workspace".to_owned(),
                    operation: "save-file".to_owned(),
                    level: OutputLevel::Information,
                    message: format!("saved {}", path.display()),
                });
            }
            Event::DocumentSavedAs { path } => {
                self.active_path = Some(path.clone());
                if let Some(tab) = self.tabs.get_mut(self.active_tab) {
                    tab.path = Some(path.clone());
                }
                self.buffer.mark_saved();
                self.sync_buffer_projection();
                self.output.push(OutputMessage {
                    subsystem: "workspace".to_owned(),
                    operation: "save-as".to_owned(),
                    level: OutputLevel::Information,
                    message: format!("saved {}", path.display()),
                });
            }
            Event::DocumentSaveFailed { message, .. } | Event::Output(message) => {
                self.output.push(message);
            }
            Event::GitStatusUpdated {
                request,
                root,
                summary,
                entries,
                branch_state,
                head,
                conflicts,
                diff_files,
                branches,
                stashes,
                history,
            } => {
                self.pending_processes.remove(&request);
                self.git_status = Some(summary.clone());
                self.git_dashboard = Some(app_ui::git::GitDashboardState::from_repository(
                    root,
                    app_ui::git::GitRepositorySnapshot {
                        summary,
                        branch_state,
                        head,
                        trust: if self.workspace_trusted {
                            app_ui::git::GitTrustState::Trusted
                        } else {
                            app_ui::git::GitTrustState::Untrusted
                        },
                        changes: entries,
                        diff_files,
                        branches,
                        stashes,
                        history,
                        conflicts,
                    },
                ));
            }
            Event::ExplorerUpdated { request, entries }
                if self.latest_explorer_request == Some(request) =>
            {
                let candidates = entries
                    .iter()
                    .filter(|entry| entry.path.is_file())
                    .enumerate()
                    .map(
                        |(recent_rank, entry)| app_ui::workspace::QuickOpenCandidate {
                            label: entry.path.display().to_string(),
                            path: entry.path.clone(),
                            recent_rank,
                        },
                    )
                    .collect();
                self.workspace_ui.quick_open.set_candidates(candidates);
                self.explorer_entries = entries
                    .into_iter()
                    .map(|entry| ExplorerProjection {
                        depth: entry.depth,
                        label: entry.path.file_name().map_or_else(
                            || entry.path.display().to_string(),
                            |name| name.to_string_lossy().into_owned(),
                        ),
                        active: self.active_path.as_ref() == Some(&entry.path),
                        expanded: false,
                    })
                    .collect();
            }
            Event::ExplorerUpdated { .. } => {}
            Event::ClipboardWritten { request, cut } => {
                if cut {
                    if let Some(PendingClipboard::Cut { range, expected }) =
                        self.pending_clipboard.remove(&request)
                    {
                        let current = self
                            .buffer
                            .snapshot()
                            .text_in_range(range)
                            .ok()
                            .map(str::to_owned);
                        if current.as_deref() == Some(expected.as_str()) {
                            if let Ok(transaction) =
                                editor_core::Transaction::new(vec![editor_core::Edit::delete(
                                    range,
                                )])
                            {
                                let _ = self.buffer.apply_transaction(transaction);
                                self.sync_buffer_projection();
                            }
                        } else {
                            self.output.push(OutputMessage {
                                subsystem: "editor".to_owned(),
                                operation: "cut".to_owned(),
                                level: OutputLevel::Warning,
                                message: "selection changed before clipboard write completed"
                                    .to_owned(),
                            });
                        }
                    }
                }
            }
            Event::ClipboardRead { request, text } => {
                if let Some(PendingClipboard::Paste { range }) =
                    self.pending_clipboard.remove(&request)
                {
                    if let Ok(transaction) =
                        editor_core::Transaction::new(vec![editor_core::Edit::replace(
                            range, &text,
                        )])
                    {
                        if self.buffer.apply_transaction(transaction).is_ok() {
                            self.sync_buffer_projection();
                            if self.format_on_paste {
                                if let Some(effect) = self.start_format(false) {
                                    self.deferred_effects.push(effect);
                                }
                            }
                        }
                    }
                }
            }
            Event::ClipboardFailed { request, message } => {
                self.pending_clipboard.remove(&request);
                self.output.push(message);
            }
            Event::DocumentFormatted {
                request,
                replacement,
            } => {
                let Some(pending) = self.pending_format.remove(&request) else {
                    return;
                };
                if self.buffer.snapshot().version() != pending.version {
                    self.output.push(OutputMessage {
                        subsystem: "editor".to_owned(),
                        operation: "format".to_owned(),
                        level: OutputLevel::Warning,
                        message: "discarded stale formatter result".to_owned(),
                    });
                    return;
                }
                let range = editor_types::TextRange {
                    start: editor_types::CharacterOffset(0),
                    end: editor_types::CharacterOffset(self.buffer.len_chars()),
                };
                if let Ok(transaction) =
                    editor_core::Transaction::new(vec![editor_core::Edit::replace(
                        range,
                        &replacement,
                    )])
                {
                    if self.buffer.apply_transaction(transaction).is_ok() {
                        self.sync_buffer_projection();
                        if pending.save_after {
                            if let Some(path) = self.active_path.clone() {
                                let (encoding, with_bom, line_endings) = {
                                    let tab = self.active_tab_state();
                                    (tab.encoding.clone(), tab.with_bom, tab.line_endings)
                                };
                                self.deferred_effects.push(Effect::SaveDocument {
                                    path,
                                    text: self.active_text.clone(),
                                    encoding,
                                    with_bom,
                                    line_endings,
                                });
                            }
                        }
                    }
                }
            }
            Event::DocumentFormatFailed { request, message } => {
                let pending = self.pending_format.remove(&request);
                self.output.push(message);
                if pending.is_some_and(|pending| pending.save_after) {
                    if let Some(path) = self.active_path.clone() {
                        let (encoding, with_bom, line_endings) = {
                            let tab = self.active_tab_state();
                            (tab.encoding.clone(), tab.with_bom, tab.line_endings)
                        };
                        self.deferred_effects.push(Effect::SaveDocument {
                            path,
                            text: self.active_text.clone(),
                            encoding,
                            with_bom,
                            line_endings,
                        });
                    }
                }
            }
            Event::SyntaxUpdated { update, .. } => {
                let Some(ticket) = update.snapshot.ticket else {
                    return;
                };
                if ticket.document == self.document_id
                    && ticket.version == self.buffer.snapshot().version()
                {
                    self.syntax_snapshot = update.snapshot;
                }
            }
            Event::SearchStarted {
                session_id,
                query,
                options,
            } => self.workspace_ui.apply_event(
                app_ui::workspace::WorkspaceModelEvent::SearchStarted {
                    session_id,
                    query,
                    options,
                },
            ),
            Event::SearchResult { session_id, result } => self.workspace_ui.apply_event(
                app_ui::workspace::WorkspaceModelEvent::SearchResult { session_id, result },
            ),
            Event::SearchFinished { session_id } => self
                .workspace_ui
                .apply_event(app_ui::workspace::WorkspaceModelEvent::SearchFinished { session_id }),
            Event::SearchCancelled { session_id } => self.workspace_ui.apply_event(
                app_ui::workspace::WorkspaceModelEvent::SearchCancelled { session_id },
            ),
            Event::SearchFailed {
                session_id,
                message,
            } => {
                self.workspace_ui.apply_event(
                    app_ui::workspace::WorkspaceModelEvent::SearchCancelled { session_id },
                );
                self.output.push(message);
            }
            Event::LspResponse {
                request,
                version,
                method,
                result,
            } => {
                if version == self.buffer.snapshot().version() {
                    let status_name = method.clone();
                    self.lsp_results.insert(method.clone(), result.clone());
                    self.last_language_method = Some(method.clone());
                    if method == "textDocument/formatting" {
                        self.apply_lsp_format_result(request, &result);
                    }
                    if method == "textDocument/semanticTokens/full"
                        && let Some(language_result) = semantic_result_from_response(
                            &result,
                            version,
                            self.document_id,
                            self.buffer.snapshot().text(),
                            self.lsp_position_encoding,
                        )
                    {
                        let _ = self.language_ui.apply_result(language_result);
                    }
                    if let Some(language_result) =
                        language_result_from_response(&method, &result, version, self.document_id)
                    {
                        let _ = self.language_ui.apply_result(language_result);
                    }
                    if !matches!(
                        method.as_str(),
                        "textDocument/semanticTokens/full"
                            | "textDocument/diagnostic"
                            | "textDocument/publishDiagnostics"
                    ) {
                        self.bottom_panel_view = BottomPanelView::Language;
                        self.bottom_panel_visible = true;
                    }
                    self.language_server = LanguageServerStatus::Running { name: status_name };
                }
            }
            Event::LanguageServerReady { encoding, .. } => {
                self.lsp_position_encoding = encoding;
                self.language_server = LanguageServerStatus::Running {
                    name: "language server".to_owned(),
                };
            }
            Event::LanguageDiagnostics { params, .. } => {
                let snapshot = self.buffer.snapshot();
                let mut diagnostics = Vec::new();
                for diagnostic in params.diagnostics {
                    let Ok(start) = lsp_client::protocol::lsp_position_to_editor(
                        snapshot.text(),
                        diagnostic.range.start,
                        self.lsp_position_encoding,
                    ) else {
                        continue;
                    };
                    let Ok(end) = lsp_client::protocol::lsp_position_to_editor(
                        snapshot.text(),
                        diagnostic.range.end,
                        self.lsp_position_encoding,
                    ) else {
                        continue;
                    };
                    let severity = match diagnostic.severity.unwrap_or(1) {
                        1 => editor_types::DiagnosticSeverity::Error,
                        2 => editor_types::DiagnosticSeverity::Warning,
                        3 => editor_types::DiagnosticSeverity::Information,
                        _ => editor_types::DiagnosticSeverity::Hint,
                    };
                    if let (Ok(start), Ok(end)) = (
                        self.buffer.position_to_offset(start),
                        self.buffer.position_to_offset(end),
                    ) {
                        diagnostics.push(editor_types::Diagnostic {
                            document: editor_types::DocumentId(0),
                            range: editor_types::TextRange { start, end },
                            severity,
                            message: diagnostic.message,
                            source: None,
                            version: params
                                .version
                                .and_then(|version| u64::try_from(version).ok()),
                        });
                    }
                }
                self.diagnostics = diagnostics;
                let version = self.buffer.snapshot().version();
                let records = self
                    .diagnostics
                    .iter()
                    .cloned()
                    .map(|diagnostic| app_ui::language::DiagnosticRecord {
                        workspace: None,
                        path: self
                            .active_path
                            .clone()
                            .unwrap_or_else(|| PathBuf::from("untitled")),
                        diagnostic,
                        related: Vec::new(),
                    })
                    .collect();
                self.language_ui.set_document_version(version);
                let _ =
                    self.language_ui
                        .apply_result(app_ui::language::LanguageResult::Diagnostics(
                            app_ui::language::Versioned::new(
                                version,
                                app_ui::language::DiagnosticDashboard::from_records(records),
                            ),
                        ));
            }
            Event::ReplacementApplied { report, .. } => {
                self.output.push(OutputMessage {
                    subsystem: "workspace".to_owned(),
                    operation: "replace".to_owned(),
                    level: OutputLevel::Information,
                    message: format!(
                        "replaced {} file(s); {} file(s) failed",
                        report.modified_files.len(),
                        report.failed_files.len()
                    ),
                });
            }
            Event::ReplacementFailed { message, .. } => self.output.push(message),
            Event::ExternalProcessBlocked { kind, .. } => {
                if matches!(kind, super::effect::ExternalProcessKind::LanguageServer) {
                    self.language_server = LanguageServerStatus::DisabledByPolicy;
                }
                self.output.push(OutputMessage {
                    subsystem: "workspace-trust".to_owned(),
                    operation: "authorize-external-process".to_owned(),
                    level: OutputLevel::Warning,
                    message: format!("blocked {kind:?} in an untrusted workspace"),
                });
            }
            Event::EffectFailed { request, message } => {
                if self.pending_processes.remove(&request)
                    == Some(super::effect::ExternalProcessKind::LanguageServer)
                {
                    self.language_server = LanguageServerStatus::Crashed {
                        message: message.message.clone(),
                    };
                }
                self.output.push(message);
            }
            Event::EffectCompleted(request) => match self.pending_processes.remove(&request) {
                Some(super::effect::ExternalProcessKind::LanguageServer) => {
                    self.language_server = LanguageServerStatus::Running {
                        name: "language-server".to_owned(),
                    };
                }
                Some(super::effect::ExternalProcessKind::Git) => {
                    if let Some(root) = self.git_root.clone() {
                        self.deferred_effects.push(Effect::RefreshGitStatus {
                            request: RequestId(self.frame_number.saturating_add(1)),
                            root,
                        });
                    }
                }
                Some(super::effect::ExternalProcessKind::Formatter) | None => {}
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use editor_types::{InputEvent, KeyCode, KeyEvent, LanguageServerStatus, RequestId};

    use super::{AppState, PendingFormat};
    use crate::app::{
        action::Action,
        effect::{Effect, ExternalProcessKind, ProcessSpec},
        event::Event,
    };
    use editor_core::TextBuffer;

    fn lsp_effect() -> Effect {
        Effect::ExternalProcess {
            request: RequestId(1),
            kind: ExternalProcessKind::LanguageServer,
            spec: ProcessSpec {
                executable: "server".to_owned(),
                arguments: Vec::new(),
            },
        }
    }

    #[test]
    fn external_process_is_blocked_before_dispatch_when_untrusted() {
        let mut state = AppState::default();
        let transition = state.apply_action(Action::RequestEffect(lsp_effect()));
        assert!(transition.effects.is_empty());
        assert_eq!(transition.events.len(), 1);
        state.apply_event(transition.events[0].clone());
        assert_eq!(
            state.language_server,
            LanguageServerStatus::DisabledByPolicy
        );
    }

    #[test]
    fn lsp_request_is_trust_gated_before_dispatch() {
        let mut state = AppState::default();
        let transition = state.apply_action(Action::RequestEffect(Effect::LspRequest {
            request: RequestId(7),
            version: 0,
            spec: ProcessSpec {
                executable: "fake-lsp".to_owned(),
                arguments: Vec::new(),
            },
            method: "textDocument/hover".to_owned(),
            params: serde_json::json!({}),
        }));
        assert!(matches!(
            transition.events.first(),
            Some(Event::ExternalProcessBlocked {
                request: RequestId(7),
                ..
            })
        ));
        assert!(transition.effects.is_empty());
    }

    #[test]
    fn trusted_workspace_allows_external_process_dispatch() {
        let mut state = AppState::default();
        let _ = state.apply_action(Action::SetWorkspaceTrust(true));
        let transition = state.apply_action(Action::RequestEffect(lsp_effect()));
        assert_eq!(transition.effects.len(), 1);
        assert!(transition.events.is_empty());
    }

    #[test]
    fn control_q_and_control_c_exit_the_event_loop() {
        use editor_types::{KeyCode, KeyEvent, Modifiers};

        for key in ['q', 'c'] {
            let mut state = AppState::default();
            let action = Action::Input(InputEvent::Key(KeyEvent {
                code: KeyCode::Character(key),
                modifiers: Modifiers::from_modifiers([editor_types::Modifier::Control]),
                repeat: false,
            }));
            let transition = state.apply_action(action);
            assert!(!state.running);
            assert!(transition.render);
        }
    }

    #[test]
    fn text_input_updates_persistent_buffer_and_undoes() {
        use editor_types::{InputEvent, KeyCode, KeyEvent, Modifiers};

        let mut state = AppState::default();
        let key = |character| {
            Action::Input(InputEvent::Key(KeyEvent {
                code: KeyCode::Character(character),
                modifiers: Modifiers::default(),
                repeat: false,
            }))
        };
        let _ = state.apply_action(key('h'));
        let _ = state.apply_action(key('i'));
        assert_eq!(state.active_text, "hi");
        assert!(state.active_dirty);

        let _ = state.apply_action(Action::Invoke(editor_types::CommandId::new("editor.undo")));
        assert_eq!(state.active_text, "h");
        assert!(state.active_dirty);
    }

    #[test]
    fn large_file_mode_keeps_editing_and_suppresses_lsp_effects() {
        let mut state = AppState {
            buffer: TextBuffer::with_large_file_threshold("1234", 5),
            ..AppState::default()
        };
        state.sync_buffer_projection();
        assert!(!state.buffer.is_large_file());

        let transition = state.apply_action(Action::Input(InputEvent::Key(KeyEvent {
            code: KeyCode::Character('5'),
            modifiers: editor_types::Modifiers::default(),
            repeat: false,
        })));
        assert!(state.buffer.is_large_file());
        assert_eq!(state.active_text, "51234");
        let syntax = transition.effects.iter().find_map(|effect| {
            let Effect::RefreshSyntax { large_file, .. } = effect else {
                return None;
            };
            Some(*large_file)
        });
        assert_eq!(syntax, Some(true));
        assert!(
            transition
                .effects
                .iter()
                .all(|effect| !matches!(effect, Effect::LspRequest { .. }))
        );
    }

    #[test]
    fn large_file_language_requests_are_rejected_before_dispatch() {
        let mut state = AppState {
            buffer: TextBuffer::with_large_file_threshold("1234", 4),
            workspace_trusted: true,
            ..AppState::default()
        };
        state.sync_buffer_projection();
        let transition = state.apply_language_effect_request(
            app_ui::language::LanguageEffectRequest::RequestCompletion {
                document: state.document_id,
                version: state.buffer.snapshot().version(),
                position: editor_types::LogicalPosition {
                    line: 0,
                    character: 0,
                },
            },
        );
        assert!(transition.effects.is_empty());
        assert!(
            state
                .output
                .iter()
                .any(|message| message.operation == "large-file")
        );
    }

    #[test]
    fn dirty_quit_is_blocked_and_surfaces_a_warning() {
        use editor_types::{InputEvent, KeyCode, KeyEvent, Modifiers};

        let mut state = AppState::default();
        let _ = state.apply_action(Action::Input(InputEvent::Key(KeyEvent {
            code: KeyCode::Character('x'),
            modifiers: Modifiers::default(),
            repeat: false,
        })));
        let transition = state.apply_action(Action::Quit);
        assert!(state.running);
        assert!(transition.render);
        assert!(
            state
                .output
                .iter()
                .any(|message| message.operation == "quit")
        );
    }

    #[test]
    fn command_palette_routes_selected_command_to_root_state() {
        use editor_types::{InputEvent, KeyCode, KeyEvent, Modifier, Modifiers};
        let mut state = AppState::default();
        let key = |code| {
            Action::Input(InputEvent::Key(KeyEvent {
                code,
                modifiers: Modifiers::default(),
                repeat: false,
            }))
        };
        let _ = state.apply_action(Action::Input(InputEvent::Key(KeyEvent {
            code: KeyCode::Character('p'),
            modifiers: Modifiers::from_modifiers([Modifier::Control]),
            repeat: false,
        })));
        assert!(state.palette_visible);
        for character in "quit".chars() {
            let _ = state.apply_action(key(KeyCode::Character(character)));
        }
        let _ = state.apply_action(key(KeyCode::Enter));
        assert!(!state.running);
        assert!(!state.palette_visible);
    }

    #[test]
    fn git_view_commands_route_to_the_source_control_panel() {
        let mut state = AppState::default();
        for command in [
            "git.showChanges",
            "git.showDiff",
            "git.showBranches",
            "git.showStashes",
            "git.showHistory",
            "git.showCommit",
            "git.showConflicts",
        ] {
            let transition =
                state.apply_action(Action::Invoke(editor_types::CommandId::new(command)));
            assert!(transition.render, "{command} should redraw the panel");
            assert!(state.bottom_panel_visible);
            assert_eq!(state.bottom_panel_view, super::BottomPanelView::Git);
        }
    }

    #[test]
    fn workspace_settings_enable_formatting_and_configure_viewport() {
        let mut state = AppState::default();
        let settings = config_core::EditorSettings {
            format_on_save: true,
            format_on_paste: true,
            tab_size: 2,
            insert_spaces: false,
            line_numbers: false,
            ..config_core::EditorSettings::default()
        };
        state.apply_settings(&settings);
        assert!(state.format_on_save);
        assert!(state.format_on_paste);
        assert_eq!(state.tab_width, 2);
        assert!(!state.insert_spaces);
        assert!(!state.show_line_numbers);
        assert_eq!(
            state.buffer.large_file_threshold(),
            usize::try_from(settings.large_file_threshold).unwrap_or(usize::MAX)
        );
    }

    #[test]
    fn trusting_workspace_marks_lsp_unavailable_when_no_known_server_exists() {
        let directory = tempfile::tempdir().expect("temporary directory is available");
        let path = directory.path().join("sample.rs");
        std::fs::write(&path, "fn main() {}\n").expect("fixture is writable");
        let mut state = AppState::default();
        state.open_startup_path(&path);
        let transition = state.apply_action(Action::SetWorkspaceTrust(true));
        assert!(transition.render);
        assert!(matches!(
            state.language_server,
            LanguageServerStatus::Unavailable
                | LanguageServerStatus::Starting
                | LanguageServerStatus::Running { .. }
        ));
    }

    #[test]
    fn mouse_click_and_drag_create_a_logical_selection() {
        use editor_types::{
            InputEvent, Modifiers, MouseAction, MouseButton, MouseEvent, ScreenCell,
        };
        let mut state = AppState {
            buffer: TextBuffer::new("hello world"),
            ..AppState::default()
        };
        state.sync_buffer_projection();
        let click = MouseEvent {
            position: ScreenCell { row: 2, column: 25 },
            action: MouseAction::Down(MouseButton::Left),
            modifiers: Modifiers::default(),
        };
        let drag = MouseEvent {
            position: ScreenCell { row: 2, column: 30 },
            action: MouseAction::Drag(MouseButton::Left),
            modifiers: Modifiers::default(),
        };
        let _ = state.apply_action(Action::Input(InputEvent::Mouse(click)));
        let _ = state.apply_action(Action::Input(InputEvent::Mouse(drag)));
        assert_eq!(state.buffer.selections().primary().range().start.0, 0);
        assert_eq!(state.buffer.selections().primary().range().end.0, 5);
    }

    #[test]
    fn trusted_git_refresh_is_emitted_as_a_background_effect() {
        let directory = tempfile::tempdir().expect("temporary directory is available");
        let mut state = AppState::default();
        let _ = state.apply_action(Action::AddWorkspaceRoot(directory.path().to_path_buf()));
        let _ = state.apply_action(Action::SetWorkspaceTrust(true));
        let transition =
            state.apply_action(Action::Invoke(editor_types::CommandId::new("git.refresh")));
        assert!(matches!(
            transition.effects.first(),
            Some(Effect::RefreshGitStatus { .. })
        ));
    }

    #[test]
    fn cut_deletes_only_after_clipboard_write_succeeds() {
        use editor_core::{CharacterOffset, Selection, SelectionSet};
        let mut state = AppState {
            buffer: TextBuffer::new("hello"),
            ..AppState::default()
        };
        state.sync_buffer_projection();
        state
            .set_active_selections(SelectionSet::single(Selection::new(
                CharacterOffset(0),
                CharacterOffset(5),
            )))
            .expect("selection");
        let transition =
            state.apply_action(Action::Invoke(editor_types::CommandId::new("editor.cut")));
        let Effect::ClipboardWrite {
            request, cut: true, ..
        } = transition.effects[0].clone()
        else {
            panic!("cut should request a clipboard write");
        };
        assert_eq!(state.active_text, "hello");
        state.apply_event(Event::ClipboardWritten { request, cut: true });
        assert_eq!(state.active_text, "");
    }

    #[test]
    fn paste_is_applied_as_one_transaction_after_clipboard_read() {
        let mut state = AppState::default();
        let transition =
            state.apply_action(Action::Invoke(editor_types::CommandId::new("editor.paste")));
        let Effect::ClipboardRead { request } = transition.effects[0].clone() else {
            panic!("paste should request a clipboard read");
        };
        state.apply_event(Event::ClipboardRead {
            request,
            text: "typed".to_owned(),
        });
        assert_eq!(state.active_text, "typed");
        let _ = state.apply_action(Action::Invoke(editor_types::CommandId::new("editor.undo")));
        assert_eq!(state.active_text, "");
    }

    #[test]
    fn format_on_paste_schedules_formatter_after_clipboard_transaction() {
        let mut state = AppState {
            workspace_trusted: true,
            format_on_paste: true,
            ..AppState::default()
        };
        state.set_external_formatter(Some(ProcessSpec {
            executable: "formatter".to_owned(),
            arguments: Vec::new(),
        }));
        let transition =
            state.apply_action(Action::Invoke(editor_types::CommandId::new("editor.paste")));
        let Effect::ClipboardRead { request } = transition.effects[0].clone() else {
            panic!("paste should request clipboard read");
        };
        state.apply_event(Event::ClipboardRead {
            request,
            text: "typed".to_owned(),
        });
        assert!(matches!(
            state.take_deferred_effects().first(),
            Some(Effect::FormatDocument { .. })
        ));
    }

    #[test]
    fn format_on_save_saves_formatted_transaction() {
        let mut state = AppState {
            active_path: Some(std::path::PathBuf::from("file.txt")),
            buffer: TextBuffer::new("before"),
            ..AppState::default()
        };
        state.sync_buffer_projection();
        let _ = state.apply_action(Action::Input(InputEvent::Key(KeyEvent {
            code: KeyCode::Character('!'),
            modifiers: editor_types::Modifiers::default(),
            repeat: false,
        })));
        state.workspace_trusted = true;
        state.format_on_save = true;
        state.set_external_formatter(Some(ProcessSpec {
            executable: "formatter".to_owned(),
            arguments: Vec::new(),
        }));
        let transition =
            state.apply_action(Action::Invoke(editor_types::CommandId::new("editor.save")));
        let Effect::FormatDocument { request, .. } = transition.effects[0].clone() else {
            panic!("save should format first");
        };
        state.apply_event(Event::DocumentFormatted {
            request,
            replacement: "after".to_owned(),
        });
        assert!(matches!(
            state.take_deferred_effects().first(),
            Some(Effect::SaveDocument { .. })
        ));
    }

    #[test]
    fn text_edits_schedule_versioned_syntax_refresh() {
        use editor_types::{KeyCode, KeyEvent};
        let mut state = AppState {
            active_path: Some(std::path::PathBuf::from("main.rs")),
            buffer: TextBuffer::new("fn main() {}"),
            ..AppState::default()
        };
        state.sync_buffer_projection();
        let transition = state.apply_action(Action::Input(InputEvent::Key(KeyEvent {
            code: KeyCode::Character('x'),
            modifiers: editor_types::Modifiers::default(),
            repeat: false,
        })));
        assert!(matches!(
            transition.effects.last(),
            Some(Effect::RefreshSyntax {
                version: 1,
                large_file: false,
                ..
            })
        ));
    }

    #[test]
    fn workspace_search_events_update_only_the_active_session() {
        let mut state = AppState::default();
        let transition =
            state.start_workspace_search("needle", app_ui::workspace::SearchOptionsView::default());
        let Effect::SearchWorkspace { session_id, .. } = transition.effects[0].clone() else {
            panic!("search effect");
        };
        state.apply_event(Event::SearchResult {
            session_id,
            result: app_ui::workspace::SearchResult {
                path: std::path::PathBuf::from("a.txt"),
                line_number: 1,
                line_text: "needle".to_owned(),
                matched_text: "needle".to_owned(),
            },
        });
        state.apply_event(Event::SearchResult {
            session_id: session_id.saturating_sub(1),
            result: app_ui::workspace::SearchResult {
                path: std::path::PathBuf::from("stale.txt"),
                line_number: 1,
                line_text: "needle".to_owned(),
                matched_text: "needle".to_owned(),
            },
        });
        assert_eq!(state.workspace_ui.search.results.len(), 1);
    }

    #[test]
    fn format_effect_is_trust_gated_and_result_is_one_undoable_transaction() {
        let mut state = AppState {
            buffer: TextBuffer::new("before"),
            ..AppState::default()
        };
        state.sync_buffer_projection();
        state.set_external_formatter(Some(ProcessSpec {
            executable: "formatter".to_owned(),
            arguments: Vec::new(),
        }));
        let blocked = state.apply_action(Action::Invoke(editor_types::CommandId::new(
            "editor.format",
        )));
        assert!(blocked.effects.is_empty());
        let _ = state.apply_action(Action::SetWorkspaceTrust(true));
        let transition = state.apply_action(Action::Invoke(editor_types::CommandId::new(
            "editor.format",
        )));
        let Effect::FormatDocument { request, .. } = transition.effects[0].clone() else {
            panic!("format should dispatch the configured formatter");
        };
        state.apply_event(Event::DocumentFormatted {
            request,
            replacement: "after".to_owned(),
        });
        assert_eq!(state.active_text, "after");
        let _ = state.apply_action(Action::Invoke(editor_types::CommandId::new("editor.undo")));
        assert_eq!(state.active_text, "before");
    }

    #[test]
    fn lsp_diagnostics_are_converted_to_root_ranges() {
        let mut state = AppState {
            buffer: TextBuffer::new("diagnostic\n"),
            ..AppState::default()
        };
        state.sync_buffer_projection();
        state.apply_event(Event::LanguageDiagnostics {
            request: RequestId(1),
            params: lsp_client::protocol::PublishDiagnosticsParams {
                uri: lsp_client::protocol::DocumentUri("file:///test.rs".to_owned()),
                diagnostics: vec![lsp_client::protocol::Diagnostic {
                    range: lsp_client::protocol::Range {
                        start: lsp_client::protocol::Position {
                            line: 0,
                            character: 0,
                        },
                        end: lsp_client::protocol::Position {
                            line: 0,
                            character: 4,
                        },
                    },
                    severity: Some(1),
                    code: None,
                    message: "error".to_owned(),
                }],
                version: Some(1),
            },
        });
        assert_eq!(state.diagnostics.len(), 1);
        assert_eq!(state.diagnostics[0].range.end.0, 4);
    }

    #[test]
    fn lsp_results_update_versioned_language_views() {
        let mut state = AppState::default();
        let version = state.buffer.snapshot().version();
        state.apply_event(Event::LspResponse {
            request: RequestId(10),
            version,
            method: String::from("textDocument/completion"),
            result: serde_json::json!({
                "items": [{"label": "println!"}, {"label": "print!"}]
            }),
        });
        let completion = state
            .language_ui
            .completion
            .current()
            .expect("completion view should be populated");
        assert_eq!(completion.list.rows.len(), 2);
        assert_eq!(completion.list.selected, Some(0));
    }

    #[test]
    fn semantic_token_delta_response_updates_versioned_spans() {
        let mut state = AppState {
            buffer: TextBuffer::new("hello\n"),
            ..AppState::default()
        };
        state.sync_buffer_projection();
        let version = state.buffer.snapshot().version();
        state.apply_event(Event::LspResponse {
            request: RequestId(11),
            version,
            method: String::from("textDocument/semanticTokens/full"),
            result: serde_json::json!({"data": [0, 0, 5, 1, 0]}),
        });
        let semantic = state
            .language_ui
            .semantic
            .current()
            .expect("semantic spans should be populated");
        assert_eq!(semantic.spans.len(), 1);
        assert_eq!(semantic.spans[0].range.start.0, 0);
        assert_eq!(semantic.spans[0].range.end.0, 5);
        assert_eq!(
            state.last_language_method.as_deref(),
            Some("textDocument/semanticTokens/full")
        );
    }

    #[test]
    fn explicit_encoding_reopen_and_conversion_are_safe_and_saveable() {
        let directory = tempfile::tempdir().expect("workspace");
        let path = directory.path().join("encoded.txt");
        let bytes = workspace_core::encode_text_document(
            "日本語\r\n",
            &workspace_core::EncodingKind::Utf16Be,
            true,
            workspace_core::DecodePolicy::Strict,
        )
        .expect("encode fixture");
        std::fs::write(&path, bytes).expect("write fixture");
        let mut state = AppState::default();
        state.open_startup_path(&path);
        assert_eq!(state.active_text, "日本語\r\n");
        let reopen = state.apply_action(Action::ReopenWithEncoding(
            workspace_core::EncodingKind::Utf16Be,
        ));
        assert!(reopen.render);
        assert_eq!(
            state.active_tab_state().encoding,
            workspace_core::EncodingKind::Utf16Be
        );
        assert!(!state.active_dirty);

        let convert = state.apply_action(Action::SetEncoding {
            encoding: workspace_core::EncodingKind::Utf8,
            with_bom: true,
        });
        assert!(convert.render);
        assert!(state.active_dirty);
        let save = state.apply_action(Action::Invoke(editor_types::CommandId::new("editor.save")));
        assert!(matches!(
            save.effects.first(),
            Some(Effect::SaveDocument {
                encoding: workspace_core::EncodingKind::Utf8,
                with_bom: true,
                ..
            })
        ));
    }

    #[test]
    fn git_hunk_and_confirmed_discard_dispatch_typed_effects() {
        let root = std::path::PathBuf::from("repo");
        let path = root.join("src/main.rs");
        let hunk = vcs_git::GitDiffHunk {
            header: String::from("@@ -1 +1 @@"),
            old_range: (1, 1),
            new_range: (1, 1),
            lines: Vec::new(),
            patch: String::from("@@ -1 +1 @@\n-old\n+new\n"),
        };
        let snapshot = app_ui::git::GitRepositorySnapshot {
            summary: editor_types::GitStatusSummary::default(),
            branch_state: Some(String::from("main")),
            head: None,
            trust: app_ui::git::GitTrustState::Trusted,
            changes: Vec::new(),
            diff_files: vec![vcs_git::GitDiffFile {
                change: vcs_git::GitFileChange::Modified,
                path: path.clone(),
                previous_path: None,
                binary: false,
                hunks: vec![hunk.clone()],
                status: String::from(" M"),
            }],
            branches: Vec::new(),
            stashes: Vec::new(),
            history: Vec::new(),
            conflicts: Vec::new(),
        };
        let mut state = AppState {
            workspace_trusted: true,
            workspace_roots: vec![root.clone()],
            git_dashboard: Some(app_ui::git::GitDashboardState::from_repository(
                root.clone(),
                snapshot,
            )),
            ..AppState::default()
        };
        let transition = state.apply_git_action(app_ui::git::GitAction::StageHunk {
            path: path.clone(),
            hunk: 0,
        });
        assert!(matches!(
            transition.effects.as_slice(),
            [Effect::GitHunk {
                reverse: false,
                hunk: selected,
                ..
            }] if selected == &hunk
        ));

        let plan = vcs_git::GitDiscardPlan {
            path,
            target: vcs_git::DiffTarget::WorkingTree,
            scope: vcs_git::GitDiscardScope::File,
            requires_confirmation: true,
            reason: String::from("discard local edits"),
            untracked: false,
            hunk: None,
        };
        let request = app_ui::git::GitAction::RequestDiscard(plan.clone());
        assert!(state.apply_git_action(request).effects.is_empty());
        let confirmed = state.apply_git_action(app_ui::git::GitAction::ConfirmDiscard);
        assert!(matches!(
            confirmed.effects.as_slice(),
            [Effect::GitDiscard {
                confirmed: true,
                plan: selected,
                ..
            }] if selected == &plan
        ));
    }

    #[test]
    fn lsp_formatting_edits_are_one_undoable_transaction() {
        let mut state = AppState {
            buffer: TextBuffer::new("foo\n"),
            ..AppState::default()
        };
        state.sync_buffer_projection();
        let version = state.buffer.snapshot().version();
        let request = RequestId(88);
        state.pending_lsp_format.insert(
            request,
            PendingFormat {
                version,
                save_after: false,
            },
        );
        state.apply_event(Event::LspResponse {
            request,
            version,
            method: String::from("textDocument/formatting"),
            result: serde_json::json!([{
                "range": {
                    "start": {"line": 0, "character": 0},
                    "end": {"line": 0, "character": 3}
                },
                "newText": "bar"
            }]),
        });
        assert_eq!(state.active_text, "bar\n");
        let _ = state.apply_action(Action::Invoke(editor_types::CommandId::new("editor.undo")));
        assert_eq!(state.active_text, "foo\n");
    }
}
