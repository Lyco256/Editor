//! Authoritative root state and pure transition logic.
#![allow(clippy::struct_excessive_bools)]

use std::{
    collections::{HashMap, HashSet},
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
    lsp_open_documents: HashSet<PathBuf>,
    lsp_document_texts: HashMap<PathBuf, String>,
    pub git_status: Option<GitStatusSummary>,
    pub(crate) git_dashboard: Option<app_ui::git::GitDashboardState>,
    pub(crate) git_root: Option<PathBuf>,
    pub diagnostics: Vec<editor_types::Diagnostic>,
    pub(crate) workspace_ui: app_ui::workspace::WorkspaceUiState,
    pub(crate) language_ui: app_ui::language::LanguageModel,
    pub(crate) lsp_results: HashMap<String, serde_json::Value>,
    pub(crate) find_query: String,
    pub(crate) find_matches: Vec<TextRange>,
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
    pending_file_operation: Option<workspace_core::FileOperationPlan>,
    deferred_effects: Vec<Effect>,
    pub(crate) format_on_save: bool,
    pub(crate) format_on_paste: bool,
    pub(crate) tab_width: usize,
    pub(crate) insert_spaces: bool,
    workspace_tab_width: usize,
    workspace_insert_spaces: bool,
    pub(crate) document_trim_trailing_whitespace: Option<bool>,
    pub(crate) document_insert_final_newline: Option<bool>,
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
    let bytes = path.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' && index + 2 < bytes.len() {
            let digit = |value: u8| match value.to_ascii_lowercase() {
                b'0'..=b'9' => Some(value.to_ascii_lowercase() - b'0'),
                b'a'..=b'f' => Some(value.to_ascii_lowercase() - b'a' + 10),
                _ => None,
            };
            if let (Some(high), Some(low)) = (digit(bytes[index + 1]), digit(bytes[index + 2])) {
                decoded.push(high * 16 + low);
                index += 3;
                continue;
            }
        }
        decoded.push(bytes[index]);
        index += 1;
    }
    PathBuf::from(String::from_utf8_lossy(&decoded).replace('/', std::path::MAIN_SEPARATOR_STR))
}

fn workspace_edits_for_path(edit: &serde_json::Value, path: &Path) -> Vec<serde_json::Value> {
    let mut edits = Vec::new();
    if let Some(changes) = edit.get("changes").and_then(serde_json::Value::as_object) {
        for (uri, document_edits) in changes {
            if workspace_core::path_eq(&json_path(uri), path) {
                edits.extend(document_edits.as_array().into_iter().flatten().cloned());
            }
        }
    }
    if let Some(document_changes) = edit
        .get("documentChanges")
        .and_then(serde_json::Value::as_array)
    {
        for document_change in document_changes {
            let Some(uri) = document_change
                .get("textDocument")
                .and_then(|document| document.get("uri"))
                .and_then(serde_json::Value::as_str)
            else {
                continue;
            };
            if workspace_core::path_eq(&json_path(uri), path) {
                edits.extend(
                    document_change
                        .get("edits")
                        .and_then(serde_json::Value::as_array)
                        .into_iter()
                        .flatten()
                        .cloned(),
                );
            }
        }
    }
    edits
}

fn workspace_edit_target_paths(edit: &serde_json::Value) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    if let Some(changes) = edit.get("changes").and_then(serde_json::Value::as_object) {
        paths.extend(changes.keys().map(|uri| json_path(uri)));
    }
    if let Some(document_changes) = edit
        .get("documentChanges")
        .and_then(serde_json::Value::as_array)
    {
        paths.extend(document_changes.iter().flat_map(|change| {
            if change.get("kind").is_some() {
                match change.get("kind").and_then(serde_json::Value::as_str) {
                    Some("rename") => change
                        .get("oldUri")
                        .and_then(serde_json::Value::as_str)
                        .into_iter()
                        .chain(change.get("newUri").and_then(serde_json::Value::as_str))
                        .map(json_path)
                        .collect::<Vec<_>>(),
                    _ => change
                        .get("uri")
                        .and_then(serde_json::Value::as_str)
                        .into_iter()
                        .map(json_path)
                        .collect::<Vec<_>>(),
                }
            } else {
                change
                    .get("textDocument")
                    .and_then(|document| document.get("uri"))
                    .and_then(serde_json::Value::as_str)
                    .into_iter()
                    .map(json_path)
                    .collect::<Vec<_>>()
            }
        }));
    }
    paths.sort_by_key(|path| workspace_core::canonical_workspace_key(path));
    paths.dedup_by(|left, right| workspace_core::path_eq(left, right));
    paths
}

fn convert_workspace_edits(
    buffer: &TextBuffer,
    edits: &[serde_json::Value],
    encoding: lsp_client::protocol::PositionEncoding,
) -> Result<Vec<Edit>, ()> {
    let snapshot = buffer.snapshot();
    let text = snapshot.text();
    edits
        .iter()
        .map(|edit| {
            let range = edit.get("range").ok_or(())?;
            let start = range
                .get("start")
                .cloned()
                .and_then(|value| serde_json::from_value(value).ok())
                .ok_or(())
                .and_then(|position: lsp_client::protocol::Position| {
                    lsp_client::protocol::lsp_position_to_editor(text, position, encoding)
                        .map_err(|_| ())
                })
                .and_then(|position| buffer.position_to_offset(position).map_err(|_| ()))?;
            let end = range
                .get("end")
                .cloned()
                .and_then(|value| serde_json::from_value(value).ok())
                .ok_or(())
                .and_then(|position: lsp_client::protocol::Position| {
                    lsp_client::protocol::lsp_position_to_editor(text, position, encoding)
                        .map_err(|_| ())
                })
                .and_then(|position| buffer.position_to_offset(position).map_err(|_| ()))?;
            let text = edit
                .get("newText")
                .and_then(serde_json::Value::as_str)
                .ok_or(())?;
            Ok(Edit::replace(TextRange { start, end }, text))
        })
        .collect()
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
        "completionItem/resolve" => LanguageResult::Completion(Versioned::new(
            version,
            CompletionView {
                list: PanelState::new(panel_glyphs("Completion")),
                details: PanelState::new(panel_glyphs("Details")).with_rows(vec![panel_row(
                    result
                        .get("documentation")
                        .or_else(|| result.get("detail"))
                        .map_or_else(|| json_label(result), json_label),
                )]),
            },
        )),
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
                } else if method.ends_with("declaration") {
                    "Go to declaration"
                } else if method.ends_with("implementation") {
                    "Go to implementation"
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
        "textDocument/prepareRename" => LanguageResult::Rename(Versioned::new(
            version,
            RenamePreviewView {
                title: panel_glyphs("Prepare rename"),
                before: PanelState::new(panel_glyphs("Range"))
                    .with_rows(vec![panel_row(json_label(result))]),
                after: PanelState::new(panel_glyphs("New name")),
                conflicts: PanelState::new(panel_glyphs("Conflicts")),
            },
        )),
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
    #[allow(clippy::too_many_lines)]
    fn default() -> Self {
        Self {
            running: true,
            workspace_trusted: false,
            trust_store: workspace_core::TrustStore::default(),
            frame_number: 0,
            language_server: LanguageServerStatus::Stopped,
            lsp_position_encoding: lsp_client::protocol::PositionEncoding::Utf16,
            lsp_open_documents: HashSet::new(),
            lsp_document_texts: HashMap::new(),
            git_status: None,
            git_dashboard: None,
            git_root: None,
            diagnostics: Vec::new(),
            workspace_ui: app_ui::workspace::WorkspaceUiState::default(),
            language_ui: app_ui::language::LanguageModel::new(0),
            lsp_results: HashMap::new(),
            find_query: String::new(),
            find_matches: Vec::new(),
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
                CommandEntry::available("editor.find", "Find in Document"),
                CommandEntry::available("editor.replace", "Replace in Document"),
                CommandEntry::available("editor.expandSelection", "Expand Selection"),
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
                CommandEntry::available("workspace.confirmFileOperation", "Confirm File Operation"),
                CommandEntry::available("workspace.cancelFileOperation", "Cancel File Operation"),
                CommandEntry::available("editor.quit", "Quit"),
                CommandEntry::available("git.refresh", "Refresh Git Status"),
                CommandEntry::available("git.showChanges", "Show Git Changes"),
                CommandEntry::available("git.showDiff", "Show Git Diff"),
                CommandEntry::available("git.showBranches", "Show Git Branches"),
                CommandEntry::available("git.showStashes", "Show Git Stashes"),
                CommandEntry::available("git.showHistory", "Show Git History"),
                CommandEntry::available("git.showCommit", "Show Git Commit"),
                CommandEntry::available("git.showConflicts", "Show Git Conflicts"),
                CommandEntry::available("language.showProblems", "Show Language Problems"),
                CommandEntry::available("language.openHover", "Show Hover"),
                CommandEntry::available("language.openSignature", "Show Signature Help"),
                CommandEntry::available("language.acceptCompletion", "Accept Completion"),
                CommandEntry::available("language.resolveCompletion", "Resolve Completion Details"),
                CommandEntry::available("language.acceptCodeAction", "Accept Code Action"),
                CommandEntry::available("language.prepareRename", "Prepare Rename"),
                CommandEntry::available("language.restart", "Restart Language Server"),
                CommandEntry::available("language.dismiss", "Dismiss Language Popup"),
                CommandEntry::available("language.requestCompletion", "Request Completion"),
                CommandEntry::available("language.requestHover", "Request Hover"),
                CommandEntry::available("language.requestSignature", "Request Signature Help"),
                CommandEntry::available("language.goToDefinition", "Go to Definition"),
                CommandEntry::available("language.goToDeclaration", "Go to Declaration"),
                CommandEntry::available("language.goToImplementation", "Go to Implementation"),
                CommandEntry::available("language.findReferences", "Find References"),
                CommandEntry::available("language.requestRename", "Preview Rename"),
                CommandEntry::available("language.requestCodeActions", "Request Code Actions"),
                CommandEntry::available("language.requestInlayHints", "Request Inlay Hints"),
                CommandEntry::available("language.requestSymbols", "Request Document Symbols"),
                CommandEntry::available(
                    "language.requestWorkspaceSymbols",
                    "Request Workspace Symbols",
                ),
                CommandEntry::available("language.requestFormatting", "Request Formatting"),
                CommandEntry::available(
                    "language.requestRangeFormatting",
                    "Request Range Formatting",
                ),
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
            pending_file_operation: None,
            deferred_effects: Vec::new(),
            format_on_save: false,
            format_on_paste: false,
            tab_width: 4,
            insert_spaces: true,
            workspace_tab_width: 4,
            workspace_insert_spaces: true,
            document_trim_trailing_whitespace: None,
            document_insert_final_newline: None,
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
            EditorAction::ExpandSelection => {
                let current = self.buffer.selections().primary().range();
                let candidate = self
                    .syntax_snapshot
                    .symbols
                    .iter()
                    .map(|symbol| symbol.range)
                    .filter(|range| {
                        range.start <= current.start
                            && range.end >= current.end
                            && (range.start < current.start || range.end > current.end)
                    })
                    .min_by_key(|range| range.end.0.saturating_sub(range.start.0));
                if let Some(range) = candidate {
                    let _ = self
                        .buffer
                        .set_selections(editor_core::SelectionSet::single(
                            editor_core::Selection::new(range.start, range.end),
                        ));
                    self.sync_buffer_projection();
                }
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
        self.workspace_tab_width = self.tab_width;
        self.workspace_insert_spaces = self.insert_spaces;
        self.show_line_numbers = settings.line_numbers;
        let threshold = usize::try_from(settings.large_file_threshold).unwrap_or(usize::MAX);
        self.buffer.set_large_file_threshold(threshold);
        for tab in &mut self.tabs {
            tab.buffer.set_large_file_threshold(threshold);
        }
        if let Some(path) = self.active_path.clone() {
            let document_settings = self.load_editorconfig(&path);
            self.apply_editorconfig_settings(&document_settings);
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
    #[allow(clippy::needless_pass_by_value, clippy::too_many_lines)]
    pub fn apply_language_action(
        &mut self,
        action: app_ui::language::LanguageAction,
    ) -> Transition {
        use app_ui::language::LanguageAction;
        match action {
            LanguageAction::RevealProblems => {
                self.bottom_panel_view = BottomPanelView::Problems;
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
            LanguageAction::NavigateToLocation { path, position, .. } => {
                let already_active = self
                    .active_path
                    .as_deref()
                    .is_some_and(|active| workspace_core::path_eq(active, &path));
                let transition = if already_active {
                    Transition::default()
                } else {
                    self.apply_action(Action::OpenPath(path))
                };
                if let Ok(offset) = self.buffer.position_to_offset(position) {
                    let _ = self
                        .buffer
                        .set_selections(editor_core::SelectionSet::single(
                            editor_core::Selection::cursor(offset),
                        ));
                    self.sync_buffer_projection();
                }
                return transition;
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
            LanguageAction::AcceptCompletion { index } => {
                self.last_language_method = Some("textDocument/completion".to_owned());
                self.bottom_panel_view = BottomPanelView::Language;
                self.bottom_panel_visible = true;
                let applied = self.apply_completion_item(index);
                self.output.push(OutputMessage {
                    subsystem: "lsp".to_owned(),
                    operation: "completion".to_owned(),
                    level: OutputLevel::Information,
                    message: if applied {
                        format!("completion item {index} inserted")
                    } else {
                        format!("completion item {index} selected")
                    },
                });
            }
            LanguageAction::ExpandCompletionDetails { index } => {
                self.last_language_method = Some("textDocument/completion".to_owned());
                self.bottom_panel_view = BottomPanelView::Language;
                self.bottom_panel_visible = true;
                let transition = self.apply_language_effect_request(
                    app_ui::language::LanguageEffectRequest::RequestCompletionResolve {
                        document: self.document_id,
                        version: self.buffer.snapshot().version(),
                        index,
                    },
                );
                if !transition.effects.is_empty() {
                    return transition;
                }
                self.output.push(OutputMessage {
                    subsystem: "lsp".to_owned(),
                    operation: "completion".to_owned(),
                    level: OutputLevel::Information,
                    message: format!("completion details expanded for item {index}"),
                });
            }
            LanguageAction::PrepareRename => {
                self.last_language_method = Some("textDocument/prepareRename".to_owned());
                self.bottom_panel_view = BottomPanelView::Language;
                self.bottom_panel_visible = true;
                return self.apply_language_effect_request(
                    app_ui::language::LanguageEffectRequest::RequestPrepareRename {
                        document: self.document_id,
                        version: self.buffer.snapshot().version(),
                        position: self.primary_language_position(),
                    },
                );
            }
            LanguageAction::OpenHover => {
                self.last_language_method = Some("textDocument/hover".to_owned());
                self.bottom_panel_view = BottomPanelView::Language;
                self.bottom_panel_visible = true;
            }
            LanguageAction::OpenSignatureHelp => {
                self.last_language_method = Some("textDocument/signatureHelp".to_owned());
                self.bottom_panel_view = BottomPanelView::Language;
                self.bottom_panel_visible = true;
            }
            LanguageAction::AcceptRenamePreview => {
                self.last_language_method = Some("textDocument/rename".to_owned());
                self.bottom_panel_view = BottomPanelView::Language;
                self.bottom_panel_visible = true;
                let Some(edit) = self.lsp_results.get("textDocument/rename").cloned() else {
                    self.output.push(OutputMessage {
                        subsystem: "lsp".to_owned(),
                        operation: "rename".to_owned(),
                        level: OutputLevel::Warning,
                        message: "rename response is unavailable".to_owned(),
                    });
                    return Transition {
                        render: true,
                        ..Transition::default()
                    };
                };
                if self.workspace_edit_needs_background(&edit) {
                    return self.queue_lsp_workspace_edit(edit);
                }
                let applied = self.apply_workspace_edit(&edit);
                self.output.push(OutputMessage {
                    subsystem: "lsp".to_owned(),
                    operation: "rename".to_owned(),
                    level: OutputLevel::Information,
                    message: if applied {
                        "rename preview applied".to_owned()
                    } else {
                        "rename preview selected; no applicable edit was supplied".to_owned()
                    },
                });
            }
            LanguageAction::AcceptCodeAction { index } => {
                self.last_language_method = Some("textDocument/codeAction".to_owned());
                self.bottom_panel_view = BottomPanelView::Language;
                self.bottom_panel_visible = true;
                if let Some(edit) = self
                    .lsp_results
                    .get("textDocument/codeAction")
                    .and_then(|result| result.as_array())
                    .and_then(|actions| actions.get(index))
                    .and_then(|action| action.get("edit"))
                    .filter(|edit| self.workspace_edit_needs_background(edit))
                    .cloned()
                {
                    return self.queue_lsp_workspace_edit(edit);
                }
                let applied = self.apply_code_action(index);
                self.output.push(OutputMessage {
                    subsystem: "lsp".to_owned(),
                    operation: "code-action".to_owned(),
                    level: OutputLevel::Information,
                    message: if applied {
                        format!("code action {index} applied")
                    } else {
                        format!("code action {index} selected; no applicable edit was supplied")
                    },
                });
            }
            LanguageAction::DismissPopup => {
                self.last_language_method = None;
                self.bottom_panel_visible = false;
            }
        }
        Transition {
            render: true,
            ..Transition::default()
        }
    }

    fn apply_completion_item(&mut self, index: usize) -> bool {
        let Some((range, text)) = self.completion_edit(index) else {
            return false;
        };
        let Ok(transaction) = Transaction::new(vec![Edit::replace(range, text)]) else {
            return false;
        };
        if self.buffer.apply_transaction(transaction).is_err() {
            return false;
        }
        self.language_ui
            .set_document_version(self.buffer.snapshot().version());
        self.sync_buffer_projection();
        self.deferred_effects.push(self.syntax_effect());
        true
    }

    fn completion_edit(&self, index: usize) -> Option<(TextRange, String)> {
        let result = self.lsp_results.get("textDocument/completion")?;
        let values = result
            .as_array()
            .or_else(|| result.get("items")?.as_array())?;
        let item = values.get(index)?;
        let text = item
            .get("textEdit")
            .and_then(|edit| edit.get("newText"))
            .and_then(serde_json::Value::as_str)
            .or_else(|| item.get("insertText").and_then(serde_json::Value::as_str))
            .or_else(|| item.get("label").and_then(serde_json::Value::as_str))?
            .to_owned();
        let range = item
            .get("textEdit")
            .and_then(|edit| edit.get("range"))
            .and_then(|range| {
                let start = json_position(range.get("start")?)?;
                let end = json_position(range.get("end")?)?;
                Some(TextRange {
                    start: self.buffer.position_to_offset(start).ok()?,
                    end: self.buffer.position_to_offset(end).ok()?,
                })
            })
            .unwrap_or_else(|| self.buffer.selections().primary().range());
        Some((range, text))
    }

    fn apply_code_action(&mut self, index: usize) -> bool {
        let Some(result) = self.lsp_results.get("textDocument/codeAction").cloned() else {
            return false;
        };
        let Some(action) = result.as_array().and_then(|actions| actions.get(index)) else {
            return false;
        };
        action
            .get("edit")
            .is_some_and(|edit| self.apply_workspace_edit(edit))
    }

    fn workspace_edit_needs_background(&self, edit: &serde_json::Value) -> bool {
        if edit
            .get("documentChanges")
            .and_then(serde_json::Value::as_array)
            .is_some_and(|changes| changes.iter().any(|change| change.get("kind").is_some()))
        {
            return true;
        }
        workspace_edit_target_paths(edit).iter().any(|path| {
            !self.tabs.iter().any(|tab| {
                tab.path
                    .as_deref()
                    .is_some_and(|open| workspace_core::path_eq(open, path))
            })
        })
    }

    fn queue_lsp_workspace_edit(&mut self, edit: serde_json::Value) -> Transition {
        self.apply_action(Action::RequestEffect(Effect::LspApplyWorkspaceEdit {
            request: RequestId(self.frame_number.saturating_add(1)),
            edit,
            roots: self.workspace_roots.clone(),
            encoding: self.lsp_position_encoding,
        }))
    }

    fn apply_workspace_edit(&mut self, edit: &serde_json::Value) -> bool {
        self.sync_active_tab();
        let mut prepared = Vec::with_capacity(self.tabs.len());
        let mut changed = false;
        for tab in &self.tabs {
            let Some(path) = tab.path.as_deref() else {
                prepared.push(tab.buffer.clone());
                continue;
            };
            if !self.path_is_within_workspace(path) {
                prepared.push(tab.buffer.clone());
                continue;
            }
            let edits = workspace_edits_for_path(edit, path);
            if edits.is_empty() {
                prepared.push(tab.buffer.clone());
                continue;
            }
            let converted =
                convert_workspace_edits(&tab.buffer, &edits, self.lsp_position_encoding);
            let Ok(converted) = converted else {
                return false;
            };
            let Ok(transaction) = Transaction::new(converted) else {
                return false;
            };
            let mut buffer = tab.buffer.clone();
            if buffer.apply_transaction(transaction).is_err() {
                return false;
            }
            changed = true;
            prepared.push(buffer);
        }
        if !changed {
            return false;
        }
        for (tab, buffer) in self.tabs.iter_mut().zip(prepared) {
            tab.buffer = buffer;
        }
        self.buffer = self.tabs[self.active_tab].buffer.clone();
        self.language_ui
            .set_document_version(self.buffer.snapshot().version());
        self.sync_buffer_projection();
        self.deferred_effects.push(self.syntax_effect());
        true
    }

    fn path_is_within_workspace(&self, path: &Path) -> bool {
        let target = workspace_core::canonical_workspace_identity(path);
        self.workspace_roots
            .iter()
            .any(|root| target.starts_with(workspace_core::canonical_workspace_identity(root)))
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
            LanguageEffectRequest::RequestDeclaration {
                version, position, ..
            } => (
                "textDocument/declaration",
                serde_json::json!({"textDocument": {"uri": self.active_document_uri()}, "position": {"line": position.line, "character": position.character}}),
                version,
            ),
            LanguageEffectRequest::RequestImplementation {
                version, position, ..
            } => (
                "textDocument/implementation",
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
            LanguageEffectRequest::RequestPrepareRename {
                version, position, ..
            } => (
                "textDocument/prepareRename",
                serde_json::json!({"textDocument": {"uri": self.active_document_uri()}, "position": {"line": position.line, "character": position.character}}),
                version,
            ),
            LanguageEffectRequest::RequestCompletionResolve { version, index, .. } => {
                let Some(item) = self
                    .lsp_results
                    .get("textDocument/completion")
                    .and_then(|value| value.as_array().or_else(|| value.get("items")?.as_array()))
                    .and_then(|items| items.get(index))
                    .cloned()
                else {
                    self.output.push(OutputMessage {
                        subsystem: "lsp".to_owned(),
                        operation: "completion-resolve".to_owned(),
                        level: OutputLevel::Warning,
                        message: format!("completion item {index} is no longer available"),
                    });
                    return Transition {
                        render: true,
                        ..Transition::default()
                    };
                };
                ("completionItem/resolve", item, version)
            }
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
                serde_json::json!({"textDocument": {"uri": self.active_document_uri()}, "options": {"tabSize": self.tab_width, "insertSpaces": self.insert_spaces}}),
                version,
            ),
            LanguageEffectRequest::RequestRangeFormatting { version, range, .. } => {
                let Ok(start) = self.buffer.offset_to_position(range.start) else {
                    return Transition {
                        render: true,
                        ..Transition::default()
                    };
                };
                let Ok(end) = self.buffer.offset_to_position(range.end) else {
                    return Transition {
                        render: true,
                        ..Transition::default()
                    };
                };
                (
                    "textDocument/rangeFormatting",
                    serde_json::json!({
                        "textDocument": {"uri": self.active_document_uri()},
                        "range": {
                            "start": {"line": start.line, "character": start.character},
                            "end": {"line": end.line, "character": end.character}
                        },
                        "options": {"tabSize": self.tab_width, "insertSpaces": self.insert_spaces}
                    }),
                    version,
                )
            }
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
        if matches!(
            method,
            "textDocument/formatting" | "textDocument/rangeFormatting"
        ) {
            self.pending_lsp_format.insert(
                request_id,
                PendingFormat {
                    version,
                    save_after: false,
                },
            );
        }
        self.apply_action(Action::RequestEffect(Effect::LspRequest {
            request: request_id,
            version,
            spec,
            method: method.to_owned(),
            params,
        }))
    }

    fn request_file_operation(&mut self, plan: workspace_core::FileOperationPlan) -> Transition {
        if let workspace_core::FileOperationPlan::Delete(delete) = &plan
            && self.active_dirty
            && self.active_path.as_ref() == Some(&delete.path)
        {
            self.output.push(OutputMessage {
                subsystem: "workspace".to_owned(),
                operation: "delete".to_owned(),
                level: OutputLevel::Warning,
                message: "save or close the dirty buffer before deleting its file".to_owned(),
            });
            return Transition {
                render: true,
                ..Transition::default()
            };
        }
        if !self.file_operation_is_in_workspace(&plan) {
            self.output.push(OutputMessage {
                subsystem: "workspace".to_owned(),
                operation: "file-operation".to_owned(),
                level: OutputLevel::Warning,
                message: "file operation target is outside the active workspace".to_owned(),
            });
            return Transition {
                render: true,
                ..Transition::default()
            };
        }
        self.workspace_ui.prompt = Some(match &plan {
            workspace_core::FileOperationPlan::CreateFile { path } => {
                app_ui::workspace::WorkspacePrompt::CreateFile { path: path.clone() }
            }
            workspace_core::FileOperationPlan::Rename(rename) => {
                app_ui::workspace::WorkspacePrompt::Rename {
                    source: rename.source.clone(),
                    target: rename.target.clone(),
                }
            }
            workspace_core::FileOperationPlan::Move(move_plan) => {
                app_ui::workspace::WorkspacePrompt::Move {
                    source: move_plan.source.clone(),
                    target: move_plan.target.clone(),
                }
            }
            workspace_core::FileOperationPlan::Delete(delete) => {
                app_ui::workspace::WorkspacePrompt::Delete {
                    path: delete.path.clone(),
                    recursive: delete.recursive,
                    byte_len: delete.byte_len,
                }
            }
        });
        self.pending_file_operation = Some(plan);
        self.bottom_panel_view = BottomPanelView::Output;
        self.bottom_panel_visible = true;
        Transition {
            render: true,
            ..Transition::default()
        }
    }

    fn confirmed_file_operation_effect(&mut self) -> Option<Effect> {
        let Some(plan) = self.pending_file_operation.take() else {
            self.output.push(OutputMessage {
                subsystem: "workspace".to_owned(),
                operation: "file-operation".to_owned(),
                level: OutputLevel::Warning,
                message: "no file operation is waiting for confirmation".to_owned(),
            });
            return None;
        };
        self.workspace_ui.prompt = None;
        Some(Effect::FileOperation {
            request: RequestId(self.frame_number.saturating_add(1)),
            plan,
        })
    }

    fn confirm_file_operation(&mut self) -> Transition {
        Transition {
            effects: self.confirmed_file_operation_effect().into_iter().collect(),
            render: true,
            ..Transition::default()
        }
    }

    fn cancel_file_operation(&mut self) -> Transition {
        self.pending_file_operation = None;
        self.workspace_ui.prompt = None;
        Transition {
            render: true,
            ..Transition::default()
        }
    }

    fn file_operation_is_in_workspace(&self, plan: &workspace_core::FileOperationPlan) -> bool {
        match plan {
            workspace_core::FileOperationPlan::CreateFile { path }
            | workspace_core::FileOperationPlan::Delete(workspace_core::DeletePlan {
                path, ..
            }) => self.path_is_in_workspace(path),
            workspace_core::FileOperationPlan::Rename(rename) => {
                self.path_is_in_workspace(&rename.source)
                    && self.path_is_in_workspace(&rename.target)
            }
            workspace_core::FileOperationPlan::Move(move_plan) => {
                self.path_is_in_workspace(&move_plan.source)
                    && self.path_is_in_workspace(&move_plan.target)
            }
        }
    }

    fn path_is_in_workspace(&self, path: &Path) -> bool {
        let candidate = if path.exists() {
            std::fs::canonicalize(path).ok()
        } else {
            path.parent()
                .and_then(|parent| std::fs::canonicalize(parent).ok())
        };
        let Some(candidate) = candidate else {
            return false;
        };
        self.workspace_roots
            .iter()
            .any(|root| std::fs::canonicalize(root).is_ok_and(|root| candidate.starts_with(root)))
    }

    fn active_document_uri(&self) -> String {
        self.active_path.as_ref().map_or_else(
            || "untitled:editor".to_owned(),
            |path| Self::document_uri(path),
        )
    }

    fn document_uri(path: &Path) -> String {
        format!("file://{}", path.to_string_lossy().replace('\\', "/"))
    }

    fn queue_lsp_notification(&mut self, method: &str, params: serde_json::Value) {
        if self.workspace_trusted
            && matches!(self.language_server, LanguageServerStatus::Running { .. })
        {
            self.deferred_effects.push(Effect::LspNotification {
                request: RequestId(self.frame_number.saturating_add(1)),
                method: method.to_owned(),
                params,
            });
        }
    }

    fn queue_lsp_did_open_for(&mut self, path: PathBuf, text: &str) {
        if self
            .lsp_open_documents
            .iter()
            .any(|open| workspace_core::path_eq(open, &path))
        {
            return;
        }
        let language_id = syntax_engine::SyntaxLanguage::from_path(&path).map_or_else(
            || "plaintext".to_owned(),
            |language| language.name().to_owned(),
        );
        self.queue_lsp_notification(
            "textDocument/didOpen",
            serde_json::json!({
                "textDocument": {
                    "uri": Self::document_uri(&path),
                    "languageId": language_id,
                    "version": 1,
                    "text": text,
                }
            }),
        );
        self.lsp_document_texts
            .insert(path.clone(), text.to_owned());
        self.lsp_open_documents.insert(path);
    }

    fn queue_lsp_did_open(&mut self) {
        let documents = self
            .tabs
            .iter()
            .filter_map(|tab| {
                tab.path
                    .clone()
                    .map(|path| (path, tab.buffer.snapshot().text().to_owned()))
            })
            .collect::<Vec<_>>();
        for (path, text) in documents {
            self.queue_lsp_did_open_for(path, &text);
        }
    }

    fn queue_lsp_did_close(&mut self, path: &Path) {
        let Some(open) = self
            .lsp_open_documents
            .iter()
            .find(|open| workspace_core::path_eq(open, path))
            .cloned()
        else {
            return;
        };
        self.queue_lsp_notification(
            "textDocument/didClose",
            serde_json::json!({"textDocument": {"uri": Self::document_uri(&open)}}),
        );
        self.lsp_open_documents.remove(&open);
        self.lsp_document_texts.remove(&open);
    }

    fn queue_lsp_did_change(&mut self, previous_text: String) {
        let Some(path) = self.active_path.clone() else {
            return;
        };
        let Some(open) = self
            .lsp_open_documents
            .iter()
            .find(|open| workspace_core::path_eq(open, &path))
            .cloned()
        else {
            return;
        };
        let current = self.buffer.snapshot().text().to_owned();
        if self.lsp_document_texts.get(&open) == Some(&current) {
            return;
        }
        let old_buffer = TextBuffer::new(&previous_text);
        let old_end = old_buffer
            .offset_to_position(CharacterOffset(old_buffer.len_chars()))
            .unwrap_or_default();
        let old_lsp_end = lsp_client::PositionMapper::new(
            &lsp_client::TextSnapshot::new(previous_text),
            self.lsp_position_encoding,
        )
        .to_lsp(old_end)
        .unwrap_or_default();
        self.queue_lsp_notification(
            "textDocument/didChange",
            serde_json::json!({
                "textDocument": {
                    "uri": Self::document_uri(&open),
                    "version": self.buffer.snapshot().version(),
                },
                "contentChanges": [{
                    "range": {
                        "start": {"line": 0, "character": 0},
                        "end": {"line": old_lsp_end.line, "character": old_lsp_end.character}
                    },
                    "text": current,
                }]
            }),
        );
        self.lsp_document_texts.insert(open, current);
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
        self.bottom_panel_view = BottomPanelView::Git;
        self.bottom_panel_visible = true;
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
        let document_settings = self.load_editorconfig(&path);
        let mut load_options = workspace_core::DocumentLoadOptions::default();
        if let Some(charset) = document_settings.charset.clone() {
            load_options.fallback_encoding = config_encoding_to_workspace(&charset);
        }
        match workspace_core::load_text_document(&path, &load_options) {
            Ok(document) => {
                let line_endings = document_settings
                    .end_of_line
                    .map_or(document.line_endings, config_line_endings_to_workspace);
                self.active_path = Some(document.path);
                self.tabs = vec![TabState {
                    path: self.active_path.clone(),
                    buffer: TextBuffer::new(&document.text),
                    encoding: document.encoding,
                    with_bom: document.had_bom,
                    line_endings,
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
                self.apply_editorconfig_settings(&document_settings);
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
        let document_settings = self.load_editorconfig(&path);
        let mut load_options = workspace_core::DocumentLoadOptions::default();
        if let Some(charset) = document_settings.charset.clone() {
            load_options.fallback_encoding = config_encoding_to_workspace(&charset);
        }
        let document = workspace_core::load_text_document(&path, &load_options)?;
        let line_endings = document_settings
            .end_of_line
            .map_or(document.line_endings, config_line_endings_to_workspace);
        self.sync_active_tab();
        self.tabs.push(TabState {
            path: Some(document.path.clone()),
            buffer: TextBuffer::new(&document.text),
            encoding: document.encoding,
            with_bom: document.had_bom,
            line_endings,
        });
        self.active_tab = self.tabs.len().saturating_sub(1);
        self.active_path = Some(document.path);
        self.active_text = document.text;
        self.active_dirty = false;
        self.buffer = self.tabs[self.active_tab].buffer.clone();
        self.apply_editorconfig_settings(&document_settings);
        self.refresh_explorer_entries();
        self.queue_lsp_did_open();
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
        if let Some(path) = self.active_path.clone() {
            let settings = self.load_editorconfig(&path);
            self.apply_editorconfig_settings(&settings);
        } else {
            self.apply_editorconfig_settings(&config_core::DocumentSettings::default());
        }
    }

    fn load_editorconfig(&mut self, path: &Path) -> config_core::DocumentSettings {
        match config_core::load_editorconfig(path) {
            Ok(settings) => {
                for warning in &settings.warnings {
                    self.output.push(OutputMessage {
                        subsystem: "editorconfig".to_owned(),
                        operation: "parse".to_owned(),
                        level: OutputLevel::Warning,
                        message: format!(
                            "{}:{} invalid {}={}",
                            warning.path.display(),
                            warning.line,
                            warning.property,
                            warning.value
                        ),
                    });
                }
                settings
            }
            Err(error) => {
                self.output.push(OutputMessage {
                    subsystem: "editorconfig".to_owned(),
                    operation: "load".to_owned(),
                    level: OutputLevel::Warning,
                    message: error.to_string(),
                });
                config_core::DocumentSettings::default()
            }
        }
    }

    fn apply_editorconfig_settings(&mut self, settings: &config_core::DocumentSettings) {
        self.tab_width = self.workspace_tab_width;
        self.insert_spaces = self.workspace_insert_spaces;
        self.document_trim_trailing_whitespace = settings.trim_trailing_whitespace;
        self.document_insert_final_newline = settings.insert_final_newline;
        if let Some(style) = settings.indent_style {
            self.insert_spaces = matches!(style, config_core::editorconfig::IndentStyle::Space);
        }
        if let Some(config_core::editorconfig::IndentSize::Width(width)) = settings.indent_size {
            self.tab_width = usize::from(width.max(1));
        }
        if let Some(width) = settings.tab_width {
            self.tab_width = usize::from(width.max(1));
        }
        if let Some(line_endings) = settings.end_of_line {
            if let Some(tab) = self.tabs.get_mut(self.active_tab) {
                tab.line_endings = config_line_endings_to_workspace(line_endings);
            }
        }
    }

    /// Applies document-scoped `EditorConfig` newline/whitespace rules before a save.
    ///
    /// The normalization is recorded in the buffer as one transaction so the saved snapshot
    /// always corresponds to the bytes written to disk and undo remains available.
    fn normalize_document_for_save(&mut self) {
        let trim = self.document_trim_trailing_whitespace == Some(true);
        let final_newline = self.document_insert_final_newline;
        if !trim && final_newline.is_none() {
            return;
        }
        let original = self.active_text.clone();
        let mut normalized = original.clone();
        if trim {
            normalized = normalized
                .split('\n')
                .map(|line| line.trim_end_matches([' ', '\t', '\r']))
                .collect::<Vec<_>>()
                .join("\n");
        }
        if final_newline == Some(true) {
            if !normalized.is_empty() && !normalized.ends_with('\n') {
                normalized.push('\n');
            }
        } else if final_newline == Some(false) {
            while normalized.ends_with('\n') {
                normalized.pop();
                if normalized.ends_with('\r') {
                    normalized.pop();
                }
            }
        }
        if normalized == original {
            return;
        }
        let range = editor_types::TextRange {
            start: CharacterOffset(0),
            end: CharacterOffset(self.buffer.len_chars()),
        };
        if let Ok(transaction) = Transaction::new(vec![Edit::replace(range, normalized)]) {
            if self.buffer.apply_transaction(transaction).is_ok() {
                self.sync_buffer_projection();
                self.deferred_effects.push(self.syntax_effect());
            }
        }
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
        self.workspace_trusted = !self.workspace_roots.is_empty()
            && self.workspace_roots.iter().all(|root| {
                self.trust_store
                    .state_for_path(root)
                    .allows_external_processes()
            });
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
            Action::Language(action) => return self.apply_language_action(action),
            Action::Git(action) => return self.apply_git_action(action),
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
                let stop_language_server = !trusted
                    && matches!(
                        self.language_server,
                        LanguageServerStatus::Running { .. } | LanguageServerStatus::Starting
                    );
                self.workspace_trusted = trusted;
                for root in &self.workspace_roots {
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
                } else if stop_language_server {
                    self.language_server = LanguageServerStatus::Stopped;
                    self.lsp_open_documents.clear();
                    self.lsp_document_texts.clear();
                    return Transition {
                        effects: vec![Effect::StopLanguageServer {
                            request: RequestId(self.frame_number.saturating_add(1)),
                        }],
                        render: true,
                        ..Transition::default()
                    };
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
                    let tab_dirty = tab.buffer.is_dirty();
                    let tab_path = tab.path.clone();
                    if !tab_dirty {
                        if let Some(path) = tab_path.as_ref() {
                            self.queue_lsp_did_close(path);
                        }
                    }
                    if tab_dirty {
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
                        if let Some(path) = self.active_path.clone() {
                            let settings = self.load_editorconfig(&path);
                            self.apply_editorconfig_settings(&settings);
                        } else {
                            self.apply_editorconfig_settings(
                                &config_core::DocumentSettings::default(),
                            );
                        }
                        self.queue_lsp_did_open();
                    } else {
                        self.tabs[0] = TabState::untitled();
                        self.active_tab = 0;
                        self.active_path = None;
                        self.buffer = TextBuffer::default();
                        self.sync_buffer_projection();
                        self.apply_editorconfig_settings(&config_core::DocumentSettings::default());
                    }
                }
                Transition {
                    render: true,
                    ..Transition::default()
                }
            }
            Action::SaveAs(path) => {
                self.normalize_document_for_save();
                Transition {
                    effects: vec![Effect::SaveDocumentAs {
                        path,
                        text: self.active_text.clone(),
                        encoding: self.active_tab_state().encoding.clone(),
                        with_bom: self.active_tab_state().with_bom,
                        line_endings: self.active_tab_state().line_endings,
                    }],
                    render: true,
                    ..Transition::default()
                }
            }
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
            Action::RequestFileOperation(plan) => self.request_file_operation(plan),
            Action::ConfirmFileOperation => self.confirm_file_operation(),
            Action::CancelFileOperation => self.cancel_file_operation(),
            Action::QuickOpen(action) => self.apply_workspace_action(action),
            Action::StartSearch { query, options } => self.start_workspace_search(query, options),
            Action::FindInDocument { query, options } => {
                self.find_query.clone_from(&query);
                self.find_matches = self
                    .buffer
                    .find(&query, options)
                    .unwrap_or_default()
                    .into_iter()
                    .map(|matched| matched.range)
                    .collect();
                self.output.push(OutputMessage {
                    subsystem: "editor".to_owned(),
                    operation: "find".to_owned(),
                    level: OutputLevel::Information,
                    message: format!("{} match(es) in active document", self.find_matches.len()),
                });
                Transition {
                    render: true,
                    ..Transition::default()
                }
            }
            Action::ReplaceInDocument {
                query,
                replacement,
                options,
            } => {
                match self.buffer.replace_all(&query, &replacement, options) {
                    Ok(applied) => {
                        self.find_query = query;
                        self.find_matches.clear();
                        if applied.changed {
                            self.sync_buffer_projection();
                            self.deferred_effects.push(self.syntax_effect());
                        }
                        self.output.push(OutputMessage {
                            subsystem: "editor".to_owned(),
                            operation: "replace".to_owned(),
                            level: OutputLevel::Information,
                            message: if applied.changed {
                                "document matches replaced".to_owned()
                            } else {
                                "no document matches replaced".to_owned()
                            },
                        });
                    }
                    Err(error) => self.output.push(OutputMessage {
                        subsystem: "editor".to_owned(),
                        operation: "replace".to_owned(),
                        level: OutputLevel::Error,
                        message: error.to_string(),
                    }),
                }
                Transition {
                    render: true,
                    ..Transition::default()
                }
            }
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
                    Effect::LspRequest { request, .. }
                    | Effect::LspNotification { request, .. }
                    | Effect::LspWorkspaceEdit { request, .. }
                    | Effect::LspApplyWorkspaceEdit { request, .. } => {
                        (*request, super::effect::ExternalProcessKind::LanguageServer)
                    }
                    Effect::LspServerResponse { .. } => unreachable!("unguarded effect"),
                    Effect::SaveDocument { .. }
                    | Effect::SaveDocumentAs { .. }
                    | Effect::StopLanguageServer { .. }
                    | Effect::FileOperation { .. }
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
                    | Effect::StopLanguageServer { .. }
                    | Effect::FileOperation { .. }
                    | Effect::RefreshExplorer { .. }
                    | Effect::ClipboardWrite { .. }
                    | Effect::ClipboardRead { .. }
                    | Effect::FormatDocument { .. }
                    | Effect::ApplyReplacementPlan { .. }
                    | Effect::RefreshSyntax { .. }
                    | Effect::SearchWorkspace { .. }
                    | Effect::CancelSearch { .. }
                    | Effect::LspRequest { .. }
                    | Effect::LspNotification { .. }
                    | Effect::LspServerResponse { .. }
                    | Effect::LspWorkspaceEdit { .. }
                    | Effect::LspApplyWorkspaceEdit { .. }
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
        let before_text = self.buffer.snapshot().text().to_owned();
        let direct_paste = matches!(input, InputEvent::Paste(_));
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
            self.queue_lsp_did_change(before_text);
            if direct_paste && self.format_on_paste {
                if let Some(effect) = self.start_lsp_format(false) {
                    self.deferred_effects.push(effect);
                } else if let Some(effect) = self.start_format(false) {
                    self.deferred_effects.push(effect);
                }
            }
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
            self.normalize_document_for_save();
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
                    self.normalize_document_for_save();
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
            "editor.expandSelection" => {
                let _ = self.apply_editor_action(app_ui::editor::EditorAction::ExpandSelection);
            }
            "editor.find" => {
                if self.find_query.is_empty() {
                    self.output.push(OutputMessage {
                        subsystem: "editor".to_owned(),
                        operation: "find".to_owned(),
                        level: OutputLevel::Information,
                        message: "provide a query with FindInDocument".to_owned(),
                    });
                } else {
                    self.find_matches = self
                        .buffer
                        .find(&self.find_query, editor_core::FindOptions::default())
                        .unwrap_or_default()
                        .into_iter()
                        .map(|matched| matched.range)
                        .collect();
                }
            }
            "editor.replace" => self.output.push(OutputMessage {
                subsystem: "editor".to_owned(),
                operation: "replace".to_owned(),
                level: OutputLevel::Information,
                message: "provide query and replacement with ReplaceInDocument".to_owned(),
            }),
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
            "language.showProblems" => {
                return self
                    .apply_language_command(app_ui::language::LanguageAction::RevealProblems);
            }
            "language.openHover" => {
                return self.apply_language_command(app_ui::language::LanguageAction::OpenHover);
            }
            "language.openSignature" => {
                return self
                    .apply_language_command(app_ui::language::LanguageAction::OpenSignatureHelp);
            }
            "language.acceptCompletion" => {
                return self.apply_language_command(
                    app_ui::language::LanguageAction::AcceptCompletion { index: 0 },
                );
            }
            "language.resolveCompletion" => {
                return self.apply_language_command(
                    app_ui::language::LanguageAction::ExpandCompletionDetails { index: 0 },
                );
            }
            "language.acceptCodeAction" => {
                return self.apply_language_command(
                    app_ui::language::LanguageAction::AcceptCodeAction { index: 0 },
                );
            }
            "language.restart" => {
                return self.apply_language_command(
                    app_ui::language::LanguageAction::RestartLanguageServer,
                );
            }
            "language.dismiss" => {
                return self.apply_language_command(app_ui::language::LanguageAction::DismissPopup);
            }
            "language.requestCompletion" => {
                return self.apply_language_effect_command(
                    app_ui::language::LanguageEffectRequest::RequestCompletion {
                        document: self.document_id,
                        version: self.buffer.snapshot().version(),
                        position: self.primary_language_position(),
                    },
                );
            }
            "language.requestHover" => {
                return self.apply_language_effect_command(
                    app_ui::language::LanguageEffectRequest::RequestHover {
                        document: self.document_id,
                        version: self.buffer.snapshot().version(),
                        position: self.primary_language_position(),
                    },
                );
            }
            "language.requestSignature" => {
                return self.apply_language_effect_command(
                    app_ui::language::LanguageEffectRequest::RequestSignatureHelp {
                        document: self.document_id,
                        version: self.buffer.snapshot().version(),
                        position: self.primary_language_position(),
                    },
                );
            }
            "language.goToDefinition" => {
                return self.apply_language_effect_command(
                    app_ui::language::LanguageEffectRequest::RequestGoTo {
                        document: self.document_id,
                        version: self.buffer.snapshot().version(),
                        position: self.primary_language_position(),
                    },
                );
            }
            "language.goToDeclaration" => {
                return self.apply_language_effect_command(
                    app_ui::language::LanguageEffectRequest::RequestDeclaration {
                        document: self.document_id,
                        version: self.buffer.snapshot().version(),
                        position: self.primary_language_position(),
                    },
                );
            }
            "language.goToImplementation" => {
                return self.apply_language_effect_command(
                    app_ui::language::LanguageEffectRequest::RequestImplementation {
                        document: self.document_id,
                        version: self.buffer.snapshot().version(),
                        position: self.primary_language_position(),
                    },
                );
            }
            "language.findReferences" => {
                return self.apply_language_effect_command(
                    app_ui::language::LanguageEffectRequest::RequestReferences {
                        document: self.document_id,
                        version: self.buffer.snapshot().version(),
                        position: self.primary_language_position(),
                    },
                );
            }
            "language.requestRename" => {
                return self.apply_language_effect_command(
                    app_ui::language::LanguageEffectRequest::RequestRenamePreview {
                        document: self.document_id,
                        version: self.buffer.snapshot().version(),
                        position: self.primary_language_position(),
                        new_name: "renamed".to_owned(),
                    },
                );
            }
            "language.prepareRename" => {
                return self
                    .apply_language_command(app_ui::language::LanguageAction::PrepareRename);
            }
            "language.requestCodeActions" => {
                return self.apply_language_effect_command(
                    app_ui::language::LanguageEffectRequest::RequestCodeActions {
                        document: self.document_id,
                        version: self.buffer.snapshot().version(),
                    },
                );
            }
            "language.requestInlayHints" => {
                return self.apply_language_effect_command(
                    app_ui::language::LanguageEffectRequest::RequestInlayHints {
                        document: self.document_id,
                        version: self.buffer.snapshot().version(),
                    },
                );
            }
            "language.requestSymbols" => {
                return self.apply_language_effect_command(
                    app_ui::language::LanguageEffectRequest::RequestDocumentSymbols {
                        document: self.document_id,
                        version: self.buffer.snapshot().version(),
                    },
                );
            }
            "language.requestWorkspaceSymbols" => {
                return self.apply_language_effect_command(
                    app_ui::language::LanguageEffectRequest::RequestWorkspaceSymbols {
                        version: self.buffer.snapshot().version(),
                        query: String::new(),
                    },
                );
            }
            "language.requestFormatting" => {
                return self.apply_language_effect_command(
                    app_ui::language::LanguageEffectRequest::RequestFormatting {
                        document: self.document_id,
                        version: self.buffer.snapshot().version(),
                    },
                );
            }
            "language.requestRangeFormatting" => {
                return self.apply_language_effect_command(
                    app_ui::language::LanguageEffectRequest::RequestRangeFormatting {
                        document: self.document_id,
                        version: self.buffer.snapshot().version(),
                        range: self.buffer.selections().primary().range(),
                    },
                );
            }
            "workbench.showOutput" => {
                self.bottom_panel_view = BottomPanelView::Output;
                self.bottom_panel_visible = true;
            }
            "workspace.confirmFileOperation" => return self.confirmed_file_operation_effect(),
            "workspace.cancelFileOperation" => {
                self.cancel_file_operation();
                return None;
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

    fn apply_language_command(
        &mut self,
        action: app_ui::language::LanguageAction,
    ) -> Option<Effect> {
        self.apply_language_action(action)
            .effects
            .into_iter()
            .next()
    }

    fn apply_language_effect_command(
        &mut self,
        request: app_ui::language::LanguageEffectRequest,
    ) -> Option<Effect> {
        self.apply_language_effect_request(request)
            .effects
            .into_iter()
            .next()
    }

    fn primary_language_position(&self) -> LogicalPosition {
        self.buffer
            .offset_to_position(self.buffer.selections().primary().active)
            .unwrap_or_default()
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
                    self.queue_lsp_notification(
                        "textDocument/didSave",
                        serde_json::json!({
                            "textDocument": {"uri": self.active_document_uri()},
                            "text": self.buffer.snapshot().text(),
                        }),
                    );
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
            Event::DocumentSaveFailed { message, .. }
            | Event::Output(message)
            | Event::ReplacementFailed { message, .. }
            | Event::FileOperationFailed { message, .. } => {
                self.output.push(message);
            }
            Event::FileOperationCompleted { plan, .. } => {
                let operation = match &plan {
                    workspace_core::FileOperationPlan::CreateFile { .. } => "create-file",
                    workspace_core::FileOperationPlan::Rename(_) => "rename",
                    workspace_core::FileOperationPlan::Move(_) => "move",
                    workspace_core::FileOperationPlan::Delete(_) => "delete",
                };
                match &plan {
                    workspace_core::FileOperationPlan::Rename(rename) => {
                        if self.active_path.as_ref() == Some(&rename.source) {
                            self.active_path = Some(rename.target.clone());
                            if let Some(tab) = self.tabs.get_mut(self.active_tab) {
                                tab.path = Some(rename.target.clone());
                            }
                        }
                    }
                    workspace_core::FileOperationPlan::Move(move_plan) => {
                        if self.active_path.as_ref() == Some(&move_plan.source) {
                            self.active_path = Some(move_plan.target.clone());
                            if let Some(tab) = self.tabs.get_mut(self.active_tab) {
                                tab.path = Some(move_plan.target.clone());
                            }
                        }
                    }
                    workspace_core::FileOperationPlan::Delete(delete)
                        if self.active_path.as_ref() == Some(&delete.path) =>
                    {
                        self.active_path = None;
                        self.buffer = TextBuffer::default();
                        self.active_text.clear();
                        self.active_dirty = false;
                    }
                    _ => {}
                }
                self.sync_active_tab();
                let refresh = self.explorer_refresh_transition();
                self.deferred_effects.extend(refresh.effects);
                self.output.push(OutputMessage {
                    subsystem: "workspace".to_owned(),
                    operation: operation.to_owned(),
                    level: OutputLevel::Information,
                    message: "filesystem operation completed".to_owned(),
                });
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
                            self.queue_save_after_format();
                        }
                    }
                }
            }
            Event::DocumentFormatFailed { request, message } => {
                let pending = self.pending_format.remove(&request);
                self.output.push(message);
                if pending.is_some_and(|pending| pending.save_after) {
                    self.queue_save_after_format();
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
                    if matches!(
                        method.as_str(),
                        "textDocument/formatting" | "textDocument/rangeFormatting"
                    ) {
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
            Event::LspWorkspaceEditCompleted {
                request,
                id,
                applied,
                failure_reason,
                documents,
            } => {
                if applied {
                    self.sync_active_tab();
                    for document in documents {
                        if let Some((index, tab)) =
                            self.tabs.iter_mut().enumerate().find(|(_, tab)| {
                                tab.path.as_deref().is_some_and(|path| {
                                    workspace_core::path_eq(path, &document.path)
                                })
                            })
                        {
                            tab.path = Some(document.path.clone());
                            tab.buffer = TextBuffer::new(&document.text);
                            tab.encoding = document.encoding.clone();
                            tab.with_bom = document.had_bom;
                            tab.line_endings = document.line_endings;
                            if index == self.active_tab {
                                self.buffer = tab.buffer.clone();
                                self.active_path = tab.path.clone();
                            }
                        }
                    }
                    self.sync_buffer_projection();
                    self.deferred_effects.push(self.syntax_effect());
                    self.output.push(OutputMessage {
                        subsystem: "lsp".to_owned(),
                        operation: "workspace-edit".to_owned(),
                        level: OutputLevel::Information,
                        message: "language server workspace edit applied".to_owned(),
                    });
                } else if let Some(reason) = &failure_reason {
                    self.output.push(OutputMessage {
                        subsystem: "lsp".to_owned(),
                        operation: "workspace-edit".to_owned(),
                        level: OutputLevel::Error,
                        message: reason.clone(),
                    });
                }
                self.deferred_effects.push(Effect::LspServerResponse {
                    request,
                    id,
                    result: Some(serde_json::json!({
                        "applied": applied,
                        "failureReason": failure_reason,
                    })),
                    error: None,
                });
            }
            Event::LspWorkspaceEditApplied {
                applied,
                failure_reason,
                documents,
                ..
            } => {
                if applied {
                    self.sync_active_tab();
                    for document in documents {
                        if let Some((index, tab)) =
                            self.tabs.iter_mut().enumerate().find(|(_, tab)| {
                                tab.path.as_deref().is_some_and(|path| {
                                    workspace_core::path_eq(path, &document.path)
                                })
                            })
                        {
                            tab.buffer = TextBuffer::new(&document.text);
                            tab.encoding = document.encoding.clone();
                            tab.with_bom = document.had_bom;
                            tab.line_endings = document.line_endings;
                            if index == self.active_tab {
                                self.buffer = tab.buffer.clone();
                                self.active_text.clone_from(&document.text);
                                self.active_dirty = false;
                            }
                        }
                    }
                    self.sync_buffer_projection();
                    self.deferred_effects.push(self.syntax_effect());
                    self.output.push(OutputMessage {
                        subsystem: "lsp".to_owned(),
                        operation: "workspace-edit".to_owned(),
                        level: OutputLevel::Information,
                        message: "language server workspace edit applied".to_owned(),
                    });
                } else {
                    self.output.push(OutputMessage {
                        subsystem: "lsp".to_owned(),
                        operation: "workspace-edit".to_owned(),
                        level: OutputLevel::Error,
                        message: failure_reason
                            .unwrap_or_else(|| "language server workspace edit failed".to_owned()),
                    });
                }
            }
            Event::LspServerRequest {
                request,
                id,
                method,
                params,
            } => {
                if !self.workspace_trusted {
                    self.deferred_effects.push(Effect::LspServerResponse {
                        request,
                        id,
                        result: Some(serde_json::json!({
                            "applied": false,
                            "failureReason": "workspace is no longer trusted",
                        })),
                        error: None,
                    });
                    return;
                }
                if method == "workspace/applyEdit" {
                    let edit = params
                        .as_ref()
                        .and_then(|value| value.get("edit").or(Some(value)))
                        .cloned();
                    let failure = edit.as_ref().and_then(|edit| {
                        let paths = workspace_edit_target_paths(edit);
                        if paths.is_empty() {
                            return Some(
                                "workspace edit did not contain text document changes".to_owned(),
                            );
                        }
                        if paths
                            .iter()
                            .any(|path| !self.path_is_within_workspace(path))
                        {
                            return Some(
                                "workspace edit targeted a path outside the trusted workspace"
                                    .to_owned(),
                            );
                        }
                        if self.tabs.iter().any(|tab| {
                            tab.path.as_deref().is_some_and(|path| {
                                tab.buffer.is_dirty()
                                    && paths
                                        .iter()
                                        .any(|target| workspace_core::path_eq(target, path))
                            })
                        }) {
                            return Some(
                                "workspace edit would overwrite an unsaved buffer".to_owned(),
                            );
                        }
                        None
                    });
                    if failure.is_none() {
                        if let Some(edit) = edit {
                            self.deferred_effects.push(Effect::LspWorkspaceEdit {
                                request,
                                id,
                                edit,
                                roots: self.workspace_roots.clone(),
                                encoding: self.lsp_position_encoding,
                            });
                            return;
                        }
                    }
                    self.deferred_effects.push(Effect::LspServerResponse {
                        request,
                        id,
                        result: Some(serde_json::json!({
                            "applied": false,
                            "failureReason": failure.as_deref().unwrap_or("invalid workspace edit"),
                        })),
                        error: None,
                    });
                } else {
                    self.output.push(OutputMessage {
                        subsystem: "lsp".to_owned(),
                        operation: "server-request".to_owned(),
                        level: OutputLevel::Warning,
                        message: format!("unsupported language-server request: {method}"),
                    });
                    self.deferred_effects.push(Effect::LspServerResponse {
                        request,
                        id,
                        result: Some(serde_json::json!({
                            "applied": false,
                            "failureReason": format!("unsupported server request: {method}"),
                        })),
                        error: None,
                    });
                }
            }
            Event::LanguageServerReady { encoding, .. } => {
                self.lsp_position_encoding = encoding;
                self.language_server = LanguageServerStatus::Running {
                    name: "language server".to_owned(),
                };
                self.queue_lsp_did_open();
                let folders = self
                    .workspace_roots
                    .iter()
                    .map(|root| {
                        serde_json::json!({
                            "uri": format!("file://{}", root.to_string_lossy().replace('\\', "/")),
                            "name": root.file_name().and_then(|name| name.to_str()).unwrap_or("workspace"),
                        })
                    })
                    .collect::<Vec<_>>();
                if !folders.is_empty() {
                    self.queue_lsp_notification(
                        "workspace/didChangeWorkspaceFolders",
                        serde_json::json!({"event": {"added": folders, "removed": []}}),
                    );
                }
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
    fn language_server_ready_queues_document_lifecycle_notifications() {
        let directory = tempfile::tempdir().expect("workspace");
        let path = directory.path().join("main.rs");
        std::fs::write(&path, "fn main() {}").expect("fixture");
        let mut state = AppState::default();
        state.workspace_roots.push(directory.path().to_path_buf());
        state.open_startup_path(&path);
        state.workspace_trusted = true;
        state.apply_event(Event::LanguageServerReady {
            request: RequestId(9),
            encoding: lsp_client::protocol::PositionEncoding::Utf16,
        });
        let effects = state.take_deferred_effects();
        assert!(effects.iter().any(|effect| matches!(
            effect,
            Effect::LspNotification { method, .. } if method == "textDocument/didOpen"
        )));
        assert!(effects.iter().any(|effect| matches!(
            effect,
            Effect::LspNotification { method, .. } if method == "workspace/didChangeWorkspaceFolders"
        )));

        let second = directory.path().join("lib.rs");
        std::fs::write(&second, "pub fn lib() {}").expect("second fixture");
        let _ = state.take_deferred_effects();
        state.open_tab(&second).expect("open second tab");
        assert!(state.take_deferred_effects().iter().any(|effect| matches!(
            effect,
            Effect::LspNotification { method, params, .. }
                if method == "textDocument/didOpen"
                    && params["textDocument"]["uri"].as_str().is_some_and(|uri| uri.contains("lib.rs"))
        )));
        let _ = state.apply_action(Action::CloseTab(1));
        assert!(state.take_deferred_effects().iter().any(|effect| matches!(
            effect,
            Effect::LspNotification { method, params, .. }
                if method == "textDocument/didClose"
                    && params["textDocument"]["uri"].as_str().is_some_and(|uri| uri.contains("lib.rs"))
        )));
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
    fn typed_language_and_git_actions_enter_the_root_transition_path() {
        let mut state = AppState::default();
        let language = state.apply_action(Action::Language(
            app_ui::language::LanguageAction::RevealProblems,
        ));
        assert!(language.render);
        assert!(state.bottom_panel_visible);
        let git = state.apply_action(Action::Git(app_ui::git::GitAction::SwitchView(
            app_ui::git::GitView::History,
        )));
        assert!(git.render);
        assert_eq!(state.bottom_panel_view, super::BottomPanelView::Git);
    }

    #[test]
    fn every_language_panel_action_updates_root_visibility_and_selection_state() {
        let mut state = AppState::default();
        let cases = [
            (
                app_ui::language::LanguageAction::OpenHover,
                Some("textDocument/hover"),
            ),
            (
                app_ui::language::LanguageAction::OpenSignatureHelp,
                Some("textDocument/signatureHelp"),
            ),
            (
                app_ui::language::LanguageAction::AcceptCompletion { index: 0 },
                Some("textDocument/completion"),
            ),
            (
                app_ui::language::LanguageAction::ExpandCompletionDetails { index: 0 },
                Some("textDocument/completion"),
            ),
            (
                app_ui::language::LanguageAction::AcceptRenamePreview,
                Some("textDocument/rename"),
            ),
            (
                app_ui::language::LanguageAction::AcceptCodeAction { index: 0 },
                Some("textDocument/codeAction"),
            ),
        ];
        for (action, method) in cases {
            let transition = state.apply_language_action(action);
            assert!(transition.render);
            assert!(state.bottom_panel_visible);
            assert_eq!(state.bottom_panel_view, super::BottomPanelView::Language);
            assert_eq!(state.last_language_method.as_deref(), method);
        }
        let transition =
            state.apply_language_action(app_ui::language::LanguageAction::DismissPopup);
        assert!(transition.render);
        assert!(!state.bottom_panel_visible);
        assert_eq!(state.last_language_method, None);
    }

    #[test]
    fn language_location_navigation_reuses_active_tab_and_moves_cursor() {
        let directory = tempfile::tempdir().expect("workspace");
        let path = directory.path().join("location.rs");
        std::fs::write(&path, "first\nsecond\n").expect("source");
        let mut state = AppState::default();
        state.open_startup_path(&path);
        let transition =
            state.apply_language_action(app_ui::language::LanguageAction::NavigateToLocation {
                document: state.document_id,
                path: path.clone(),
                position: editor_types::LogicalPosition {
                    line: 1,
                    character: 2,
                },
            });
        assert!(!transition.render);
        assert_eq!(state.tab_count(), 1);
        assert_eq!(state.buffer.selections().primary().active.0, 8);
    }

    #[test]
    fn file_operations_require_confirmation_and_refresh_active_path() {
        let directory = tempfile::tempdir().expect("workspace");
        let source = directory.path().join("old.rs");
        let target = directory.path().join("new.rs");
        std::fs::write(&source, "fn main() {}\n").expect("source");
        let mut state = AppState::default();
        state.open_startup_path(&source);
        let plan = workspace_core::FileOperationPlan::Rename(
            workspace_core::plan_rename(&source, &target).expect("rename plan"),
        );
        let prompt = state.apply_action(Action::RequestFileOperation(plan));
        assert!(prompt.effects.is_empty());
        assert!(state.workspace_ui.prompt.is_some());
        let confirmed = state.apply_action(Action::ConfirmFileOperation);
        let Some(Effect::FileOperation { request, plan }) = confirmed.effects.first().cloned()
        else {
            panic!("confirmed file operation effect expected");
        };
        state.apply_event(Event::FileOperationCompleted { request, plan });
        assert_eq!(state.active_path.as_deref(), Some(target.as_path()));
        assert!(
            state
                .deferred_effects
                .iter()
                .any(|effect| matches!(effect, Effect::RefreshExplorer { .. }))
        );
    }

    #[test]
    fn dirty_active_file_cannot_be_deleted_by_file_operation() {
        let directory = tempfile::tempdir().expect("workspace");
        let path = directory.path().join("dirty.rs");
        std::fs::write(&path, "fn main() {}\n").expect("source");
        let mut state = AppState::default();
        state.open_startup_path(&path);
        state.buffer.mark_recovered_dirty();
        state.sync_buffer_projection();
        let plan = workspace_core::FileOperationPlan::Delete(
            workspace_core::plan_delete(&path).expect("delete plan"),
        );
        let transition = state.apply_action(Action::RequestFileOperation(plan));
        assert!(transition.effects.is_empty());
        assert!(state.workspace_ui.prompt.is_none());
        assert!(state.output.iter().any(|message| {
            message.operation == "delete" && message.level == editor_types::OutputLevel::Warning
        }));
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
    fn editorconfig_document_settings_override_workspace_indentation_and_eol() {
        let directory = tempfile::tempdir().expect("workspace");
        std::fs::write(
            directory.path().join(".editorconfig"),
            "root = true\n[*.rs]\nindent_style = tab\nindent_size = 8\nend_of_line = crlf\n",
        )
        .expect("editorconfig");
        let path = directory.path().join("main.rs");
        std::fs::write(&path, "fn main() {}\n").expect("source");
        let mut state = AppState::default();
        state.open_startup_path(&path);
        assert_eq!(state.tab_width, 8);
        assert!(!state.insert_spaces);
        assert_eq!(
            state.active_tab_state().line_endings,
            workspace_core::LineEndings::Crlf
        );
    }

    #[test]
    fn editorconfig_save_rules_normalize_trailing_whitespace_and_final_newline() {
        let directory = tempfile::tempdir().expect("workspace");
        std::fs::write(
            directory.path().join(".editorconfig"),
            "root = true\n[*]\ntrim_trailing_whitespace = true\ninsert_final_newline = false\nend_of_line = crlf\n",
        )
        .expect("editorconfig");
        let path = directory.path().join("main.rs");
        std::fs::write(&path, "placeholder\r\n").expect("source");
        let mut state = AppState::default();
        state.open_startup_path(&path);
        let range = editor_types::TextRange {
            start: editor_types::CharacterOffset(0),
            end: editor_types::CharacterOffset(state.buffer.len_chars()),
        };
        let transaction = editor_core::Transaction::new(vec![editor_core::Edit::replace(
            range,
            "fn main() {  }   \r\n",
        )])
        .expect("valid replacement");
        state.buffer.apply_transaction(transaction).expect("edit");
        state.sync_buffer_projection();
        let transition =
            state.apply_action(Action::Invoke(editor_types::CommandId::new("editor.save")));
        let Some(Effect::SaveDocument { text, .. }) = transition.effects.first() else {
            panic!("save effect expected");
        };
        assert_eq!(text, "fn main() {  }");
    }

    #[test]
    fn server_workspace_edit_applies_to_all_matching_open_tabs_and_queues_response() {
        let directory = tempfile::tempdir().expect("workspace");
        let first = directory.path().join("first.rs");
        let second = directory.path().join("second.rs");
        std::fs::write(&first, "one\n").expect("first source");
        std::fs::write(&second, "two\n").expect("second source");
        let mut state = AppState::default();
        state.open_startup_path(&first);
        state.open_tab(&second).expect("second tab");
        state.workspace_trusted = true;
        let first_uri = format!("file:///{}", first.to_string_lossy().replace('\\', "/"));
        let second_uri = format!("file:///{}", second.to_string_lossy().replace('\\', "/"));
        let edit = serde_json::json!({
            "changes": {
                first_uri: [{
                    "range": {"start": {"line": 0, "character": 0}, "end": {"line": 0, "character": 3}},
                    "newText": "ONE"
                }],
                second_uri: [{
                    "range": {"start": {"line": 0, "character": 0}, "end": {"line": 0, "character": 3}},
                    "newText": "TWO"
                }]
            }
        });
        let _ = state.take_deferred_effects();
        state.apply_event(Event::LspServerRequest {
            request: RequestId(91),
            id: lsp_client::protocol::RequestId::Number(7),
            method: "workspace/applyEdit".to_owned(),
            params: Some(serde_json::json!({"edit": edit})),
        });
        let effects = state.take_deferred_effects();
        assert!(effects.iter().any(|effect| {
            matches!(
                effect,
                Effect::LspWorkspaceEdit {
                    request: RequestId(91),
                    id: lsp_client::protocol::RequestId::Number(7),
                    edit: _,
                    roots: _,
                    encoding: lsp_client::protocol::PositionEncoding::Utf16,
                }
            )
        }));
        state.apply_event(Event::LspWorkspaceEditCompleted {
            request: RequestId(91),
            id: lsp_client::protocol::RequestId::Number(7),
            applied: true,
            failure_reason: None,
            documents: vec![
                workspace_core::TextDocument {
                    path: first,
                    text: "ONE\n".to_owned(),
                    encoding: workspace_core::EncodingKind::Utf8,
                    had_bom: false,
                    line_endings: workspace_core::LineEndings::Lf,
                    large_file: false,
                },
                workspace_core::TextDocument {
                    path: second,
                    text: "TWO\n".to_owned(),
                    encoding: workspace_core::EncodingKind::Utf8,
                    had_bom: false,
                    line_endings: workspace_core::LineEndings::Lf,
                    large_file: false,
                },
            ],
        });
        assert_eq!(state.active_text, "TWO\n");
        assert_eq!(state.tabs[0].buffer.snapshot().text(), "ONE\n");
        assert!(state.take_deferred_effects().iter().any(|effect| matches!(
            effect,
            Effect::LspServerResponse {
                request: RequestId(91),
                id: lsp_client::protocol::RequestId::Number(7),
                result: Some(result),
                error: None,
            } if result["applied"] == true
        )));
    }

    #[test]
    fn server_workspace_edit_is_rejected_after_trust_is_revoked() {
        let mut state = AppState::default();
        state.apply_event(Event::LspServerRequest {
            request: RequestId(44),
            id: lsp_client::protocol::RequestId::Number(1),
            method: "workspace/applyEdit".to_owned(),
            params: Some(serde_json::json!({"edit": {"changes": {}}})),
        });
        let effects = state.take_deferred_effects();
        assert!(matches!(
            effects.first(),
            Some(Effect::LspServerResponse { result: Some(result), .. })
                if result["applied"] == false
        ));
        assert!(
            !effects
                .iter()
                .any(|effect| matches!(effect, Effect::LspWorkspaceEdit { .. }))
        );
    }

    #[test]
    fn revoking_workspace_trust_stops_a_running_language_server() {
        let mut state = AppState {
            workspace_trusted: true,
            language_server: LanguageServerStatus::Running {
                name: "fake".to_owned(),
            },
            ..AppState::default()
        };
        let transition = state.apply_action(Action::SetWorkspaceTrust(false));
        assert!(matches!(
            transition.effects.first(),
            Some(Effect::StopLanguageServer { .. })
        ));
        assert_eq!(state.language_server, LanguageServerStatus::Stopped);
    }

    #[test]
    fn multi_root_trust_requires_every_canonical_root() {
        let first = tempfile::tempdir().expect("first root");
        let second = tempfile::tempdir().expect("second root");
        let mut state = AppState {
            workspace_roots: vec![first.path().to_path_buf(), second.path().to_path_buf()],
            ..AppState::default()
        };
        let _ = state.apply_action(Action::SetWorkspaceTrust(true));
        assert!(state.workspace_trusted);
        let _ = state.apply_action(Action::SetWorkspaceTrust(false));
        assert!(!state.workspace_trusted);
        assert_eq!(
            state.trust_store.state_for_path(first.path()),
            workspace_core::TrustState::Untrusted
        );
        assert_eq!(
            state.trust_store.state_for_path(second.path()),
            workspace_core::TrustState::Untrusted
        );
    }

    #[test]
    fn rename_with_unopened_document_uses_background_workspace_edit() {
        let directory = tempfile::tempdir().expect("workspace");
        let active = directory.path().join("main.rs");
        let unopened = directory.path().join("lib.rs");
        std::fs::write(&active, "fn main() {}").expect("active");
        std::fs::write(&unopened, "fn old() {}").expect("unopened");
        let mut state = AppState::default();
        state.open_startup_path(&active);
        state.workspace_trusted = true;
        let uri = format!("file:///{}", unopened.to_string_lossy().replace('\\', "/"));
        state.lsp_results.insert(
            "textDocument/rename".to_owned(),
            serde_json::json!({
                "changes": {
                    uri: [{
                        "range": {"start": {"line": 0, "character": 3}, "end": {"line": 0, "character": 6}},
                        "newText": "new"
                    }]
                }
            }),
        );
        let transition =
            state.apply_language_action(app_ui::language::LanguageAction::AcceptRenamePreview);
        assert!(matches!(
            transition.effects.first(),
            Some(Effect::LspApplyWorkspaceEdit { .. })
        ));
    }

    #[test]
    fn structural_selection_expands_to_next_syntax_symbol_range() {
        let mut state = AppState {
            buffer: TextBuffer::new("fn main() { value(); }"),
            syntax_snapshot: syntax_engine::SyntaxSnapshot {
                symbols: vec![syntax_engine::SyntaxSymbol {
                    kind: syntax_engine::SyntaxSymbolKind::Function,
                    name: "main".to_owned(),
                    range: editor_types::TextRange {
                        start: editor_types::CharacterOffset(0),
                        end: editor_types::CharacterOffset(22),
                    },
                    selection_range: editor_types::TextRange {
                        start: editor_types::CharacterOffset(3),
                        end: editor_types::CharacterOffset(7),
                    },
                }],
                ..syntax_engine::SyntaxSnapshot::default()
            },
            ..AppState::default()
        };
        state.sync_buffer_projection();
        let _ = state.apply_editor_action(app_ui::editor::EditorAction::ExpandSelection);
        assert_eq!(state.buffer.selections().primary().range().start.0, 0);
        assert_eq!(state.buffer.selections().primary().range().end.0, 22);
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
    fn direct_terminal_paste_schedules_formatting() {
        let mut state = AppState {
            workspace_trusted: true,
            format_on_paste: true,
            external_formatter: Some(ProcessSpec {
                executable: "formatter".to_owned(),
                arguments: Vec::new(),
            }),
            ..AppState::default()
        };
        let _ = state.apply_action(Action::Input(InputEvent::Paste(" pasted".to_owned())));
        assert!(
            state
                .take_deferred_effects()
                .iter()
                .any(|effect| matches!(effect, Effect::FormatDocument { .. }))
        );
    }

    #[test]
    fn in_document_find_and_replace_route_through_root_actions() {
        let mut state = AppState {
            buffer: TextBuffer::new("one two one"),
            ..AppState::default()
        };
        state.sync_buffer_projection();
        let find = state.apply_action(Action::FindInDocument {
            query: "one".to_owned(),
            options: editor_core::FindOptions::default(),
        });
        assert!(find.render);
        assert_eq!(state.find_matches.len(), 2);
        let replace = state.apply_action(Action::ReplaceInDocument {
            query: "one".to_owned(),
            replacement: "1".to_owned(),
            options: editor_core::FindOptions::default(),
        });
        assert!(replace.render);
        assert_eq!(state.active_text, "1 two 1");
        assert!(state.buffer.undo().expect("undo replacement"));
        assert_eq!(state.buffer.to_string(), "one two one");
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
        let transition = state
            .apply_language_action(app_ui::language::LanguageAction::AcceptCompletion { index: 1 });
        assert!(transition.render);
        assert_eq!(state.active_text, "print!");
        assert!(state.active_dirty);
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
    fn rename_and_code_action_edits_apply_as_undoable_root_transactions() {
        let directory = tempfile::tempdir().expect("workspace");
        let path = directory.path().join("edit.rs");
        std::fs::write(&path, "old\n").expect("source");
        let mut state = AppState::default();
        state.open_startup_path(&path);
        let uri = state.active_document_uri();
        let version = state.buffer.snapshot().version();
        let edit = |new_text: &str, end: u32| {
            serde_json::json!({
                "range": {
                    "start": {"line": 0, "character": 0},
                    "end": {"line": 0, "character": end}
                },
                "newText": new_text
            })
        };
        state.apply_event(Event::LspResponse {
            request: RequestId(20),
            version,
            method: "textDocument/rename".to_owned(),
            result: serde_json::json!({"changes": {uri.clone(): [edit("renamed", 3)]}}),
        });
        let rename =
            state.apply_language_action(app_ui::language::LanguageAction::AcceptRenamePreview);
        assert!(rename.render);
        assert_eq!(state.active_text, "renamed\n");
        let version = state.buffer.snapshot().version();
        state.apply_event(Event::LspResponse {
            request: RequestId(21),
            version,
            method: "textDocument/codeAction".to_owned(),
            result: serde_json::json!([{"title": "replace", "edit": {"changes": {
                uri: [edit("fixed", 7)]
            }}}]),
        });
        let action = state
            .apply_language_action(app_ui::language::LanguageAction::AcceptCodeAction { index: 0 });
        assert!(action.render);
        assert_eq!(state.active_text, "fixed\n");
        let _ = state.apply_action(Action::Invoke(editor_types::CommandId::new("editor.undo")));
        assert_eq!(state.active_text, "renamed\n");
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
