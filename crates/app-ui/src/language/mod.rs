//! Language-intelligence view and action models.
//!
//! This module stays on the UI side of the architecture boundary: it normalizes language-server
//! results into view state, suppresses stale responses by document version, and renders
//! deterministic framebuffer snapshots for tests.

use std::{collections::BTreeMap, path::PathBuf};

#[cfg(test)]
use editor_types::CharacterOffset;
use editor_types::{
    Diagnostic, DiagnosticSeverity, DocumentId, LanguageServerStatus, LogicalPosition, StyleRole,
    TextRange, WorkspaceId,
};
use terminal_backend::{Cell, Framebuffer};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Glyph {
    pub symbol: String,
    pub foreground: StyleRole,
    pub background: StyleRole,
    pub bold: bool,
}

impl Glyph {
    #[must_use]
    pub fn plain(symbol: impl Into<String>) -> Self {
        Self {
            symbol: symbol.into(),
            foreground: StyleRole::EditorText,
            background: StyleRole::EditorBackground,
            bold: false,
        }
    }

    #[must_use]
    pub fn severity(symbol: impl Into<String>, severity: DiagnosticSeverity) -> Self {
        Self {
            symbol: symbol.into(),
            foreground: severity_role(severity),
            background: StyleRole::EditorBackground,
            bold: true,
        }
    }

    #[must_use]
    pub fn panel(mut self) -> Self {
        self.background = StyleRole::Panel;
        self
    }

    #[must_use]
    pub fn selected(mut self) -> Self {
        self.background = StyleRole::Selection;
        self
    }

    #[must_use]
    pub fn status(mut self) -> Self {
        self.background = StyleRole::StatusBar;
        self
    }
}

impl Glyph {
    fn to_cell(&self) -> Cell {
        Cell {
            symbol: self.symbol.clone(),
            foreground: self.foreground,
            background: self.background,
            bold: self.bold,
            continuation: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PanelRow {
    pub cells: Vec<Glyph>,
    pub severity: Option<DiagnosticSeverity>,
}

impl PanelRow {
    #[must_use]
    pub fn new(cells: Vec<Glyph>) -> Self {
        Self {
            cells,
            severity: None,
        }
    }

    #[must_use]
    pub fn severity(mut self, severity: DiagnosticSeverity) -> Self {
        self.severity = Some(severity);
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PanelState {
    pub title: Vec<Glyph>,
    pub rows: Vec<PanelRow>,
    pub selected: Option<usize>,
    pub footer: Option<Vec<Glyph>>,
}

impl PanelState {
    #[must_use]
    pub fn new(title: Vec<Glyph>) -> Self {
        Self {
            title,
            rows: Vec::new(),
            selected: None,
            footer: None,
        }
    }

    #[must_use]
    pub fn with_rows(mut self, rows: Vec<PanelRow>) -> Self {
        self.rows = rows;
        self
    }

    #[must_use]
    pub fn with_selected(mut self, selected: Option<usize>) -> Self {
        self.selected = selected;
        self
    }

    #[must_use]
    pub fn with_footer(mut self, footer: Vec<Glyph>) -> Self {
        self.footer = Some(footer);
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Versioned<T> {
    pub version: u64,
    pub value: T,
}

impl<T> Versioned<T> {
    #[must_use]
    pub fn new(version: u64, value: T) -> Self {
        Self { version, value }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VersionedState<T> {
    active_version: u64,
    value: Option<Versioned<T>>,
}

impl<T> VersionedState<T> {
    #[must_use]
    pub fn new(active_version: u64) -> Self {
        Self {
            active_version,
            value: None,
        }
    }

    pub fn set_active_version(&mut self, version: u64) {
        self.active_version = version;
        self.value = None;
    }

    #[must_use]
    pub const fn active_version(&self) -> u64 {
        self.active_version
    }

    #[must_use]
    pub fn current(&self) -> Option<&T> {
        self.value.as_ref().map(|entry| &entry.value)
    }

    #[must_use]
    pub fn current_version(&self) -> Option<u64> {
        self.value.as_ref().map(|entry| entry.version)
    }

    #[must_use]
    pub fn apply(&mut self, result: Versioned<T>) -> bool {
        if result.version == self.active_version {
            self.value = Some(result);
            true
        } else {
            false
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenStyle {
    SyntaxKeyword,
    SyntaxString,
    SyntaxComment,
    SemanticType,
    SemanticFunction,
    SemanticVariable,
    Plain,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StyledSpan {
    pub range: TextRange,
    pub style: TokenStyle,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpanSet {
    pub document: DocumentId,
    pub version: u64,
    pub spans: Vec<StyledSpan>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticDecorationKind {
    Underline,
    Gutter,
    OverviewRuler,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiagnosticDecoration {
    pub document: DocumentId,
    pub path: PathBuf,
    pub range: TextRange,
    pub severity: DiagnosticSeverity,
    pub kind: DiagnosticDecorationKind,
    pub message: String,
    pub source: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiagnosticSummary {
    pub error: usize,
    pub warning: usize,
    pub information: usize,
    pub hint: usize,
}

impl DiagnosticSummary {
    #[must_use]
    pub const fn empty() -> Self {
        Self {
            error: 0,
            warning: 0,
            information: 0,
            hint: 0,
        }
    }

    #[must_use]
    pub fn from_diagnostics(diagnostics: &[DiagnosticRecord]) -> Self {
        let mut summary = Self::empty();
        for diagnostic in diagnostics {
            match diagnostic.diagnostic.severity {
                DiagnosticSeverity::Error => summary.error += 1,
                DiagnosticSeverity::Warning => summary.warning += 1,
                DiagnosticSeverity::Information => summary.information += 1,
                DiagnosticSeverity::Hint => summary.hint += 1,
            }
        }
        summary
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiagnosticRecord {
    pub workspace: Option<WorkspaceId>,
    pub path: PathBuf,
    pub diagnostic: Diagnostic,
    pub related: Vec<RelatedLocation>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelatedLocation {
    pub label: Vec<Glyph>,
    pub path: PathBuf,
    pub position: LogicalPosition,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProblemGroup {
    pub workspace: Option<WorkspaceId>,
    pub path: PathBuf,
    pub severity: DiagnosticSeverity,
    pub items: Vec<DiagnosticRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProblemsView {
    pub groups: Vec<ProblemGroup>,
    pub selected_group: Option<usize>,
}

impl ProblemsView {
    #[must_use]
    pub fn empty() -> Self {
        Self {
            groups: Vec::new(),
            selected_group: None,
        }
    }

    #[must_use]
    pub fn from_diagnostics(mut diagnostics: Vec<DiagnosticRecord>) -> Self {
        diagnostics.sort_by_key(problem_sort_key);
        let has_diagnostics = !diagnostics.is_empty();
        let mut groups: BTreeMap<(Option<WorkspaceId>, PathBuf), Vec<DiagnosticRecord>> =
            BTreeMap::new();
        for diagnostic in diagnostics {
            groups
                .entry((diagnostic.workspace, diagnostic.path.clone()))
                .or_default()
                .push(diagnostic);
        }
        let groups = groups
            .into_iter()
            .map(|((workspace, path), items)| {
                let severity = items
                    .iter()
                    .map(|record| record.diagnostic.severity)
                    .min_by_key(|severity| severity_rank(*severity))
                    .unwrap_or(DiagnosticSeverity::Hint);
                ProblemGroup {
                    workspace,
                    path,
                    severity,
                    items,
                }
            })
            .collect();
        Self {
            groups,
            selected_group: has_diagnostics.then_some(0),
        }
    }

    #[must_use]
    pub fn activate(&self, index: usize) -> Option<LanguageAction> {
        self.groups.get(index).and_then(|group| {
            group
                .items
                .first()
                .map(|diagnostic| LanguageAction::NavigateToLocation {
                    document: diagnostic.diagnostic.document,
                    path: diagnostic.path.clone(),
                    position: diagnostic.related.first().map_or(
                        LogicalPosition {
                            line: 0,
                            character: 0,
                        },
                        |related| related.position,
                    ),
                })
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompletionView {
    pub list: PanelState,
    pub details: PanelState,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HoverView {
    pub card: PanelState,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignatureView {
    pub card: PanelState,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocationChoice {
    pub label: Vec<Glyph>,
    pub document: DocumentId,
    pub path: PathBuf,
    pub position: LogicalPosition,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocationChooserView {
    pub title: Vec<Glyph>,
    pub choices: Vec<LocationChoice>,
    pub selected: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenamePreviewView {
    pub title: Vec<Glyph>,
    pub before: PanelState,
    pub after: PanelState,
    pub conflicts: PanelState,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodeActionView {
    pub list: PanelState,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InlayHintView {
    pub label: Vec<Glyph>,
    pub document: DocumentId,
    pub position: LogicalPosition,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InlayHintsView {
    pub list: PanelState,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SymbolView {
    pub label: Vec<Glyph>,
    pub document: DocumentId,
    pub path: PathBuf,
    pub position: LogicalPosition,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SymbolsView {
    pub list: PanelState,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FormattingFeedbackView {
    pub card: PanelState,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LanguageServerView {
    pub card: PanelState,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiagnosticDashboard {
    pub decorations: Vec<DiagnosticDecoration>,
    pub problems: ProblemsView,
    pub summary: DiagnosticSummary,
    pub gutter: Vec<SeverityMarker>,
    pub ruler: Vec<SeverityMarker>,
}

impl DiagnosticDashboard {
    #[must_use]
    pub fn from_records(records: Vec<DiagnosticRecord>) -> Self {
        let summary = DiagnosticSummary::from_diagnostics(&records);
        let decorations = records
            .iter()
            .flat_map(|record| {
                [
                    DiagnosticDecoration {
                        document: record.diagnostic.document,
                        path: record.path.clone(),
                        range: record.diagnostic.range,
                        severity: record.diagnostic.severity,
                        kind: DiagnosticDecorationKind::Underline,
                        message: record.diagnostic.message.clone(),
                        source: record.diagnostic.source.clone(),
                    },
                    DiagnosticDecoration {
                        document: record.diagnostic.document,
                        path: record.path.clone(),
                        range: record.diagnostic.range,
                        severity: record.diagnostic.severity,
                        kind: DiagnosticDecorationKind::Gutter,
                        message: record.diagnostic.message.clone(),
                        source: record.diagnostic.source.clone(),
                    },
                    DiagnosticDecoration {
                        document: record.diagnostic.document,
                        path: record.path.clone(),
                        range: record.diagnostic.range,
                        severity: record.diagnostic.severity,
                        kind: DiagnosticDecorationKind::OverviewRuler,
                        message: record.diagnostic.message.clone(),
                        source: record.diagnostic.source.clone(),
                    },
                ]
            })
            .collect();
        let problems = ProblemsView::from_diagnostics(records);
        let gutter = markers_from_summary(&summary);
        let ruler = markers_from_summary(&summary);
        Self {
            decorations,
            problems,
            summary,
            gutter,
            ruler,
        }
    }

    #[must_use]
    pub fn status_row(&self) -> PanelRow {
        let mut cells = Vec::new();
        cells.extend(text_cells(&["Errors"]));
        cells.extend(number_cells(self.summary.error, DiagnosticSeverity::Error));
        cells.extend(text_cells(&["Warnings"]));
        cells.extend(number_cells(
            self.summary.warning,
            DiagnosticSeverity::Warning,
        ));
        cells.extend(text_cells(&["Info"]));
        cells.extend(number_cells(
            self.summary.information,
            DiagnosticSeverity::Information,
        ));
        cells.extend(text_cells(&["Hints"]));
        cells.extend(number_cells(self.summary.hint, DiagnosticSeverity::Hint));
        PanelRow::new(cells)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SeverityMarker {
    pub severity: DiagnosticSeverity,
    pub count: usize,
}

fn markers_from_summary(summary: &DiagnosticSummary) -> Vec<SeverityMarker> {
    [
        (DiagnosticSeverity::Error, summary.error),
        (DiagnosticSeverity::Warning, summary.warning),
        (DiagnosticSeverity::Information, summary.information),
        (DiagnosticSeverity::Hint, summary.hint),
    ]
    .into_iter()
    .map(|(severity, count)| SeverityMarker { severity, count })
    .collect()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LanguageAction {
    RevealProblems,
    NavigateToProblem {
        group: usize,
    },
    NavigateToLocation {
        document: DocumentId,
        path: PathBuf,
        position: LogicalPosition,
    },
    AcceptCompletion {
        index: usize,
    },
    ExpandCompletionDetails {
        index: usize,
    },
    OpenHover,
    OpenSignatureHelp,
    AcceptRenamePreview,
    AcceptCodeAction {
        index: usize,
    },
    DismissPopup,
    RestartLanguageServer,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LanguageEffectRequest {
    RefreshSyntax {
        document: DocumentId,
        version: u64,
    },
    RefreshSemanticTokens {
        document: DocumentId,
        version: u64,
    },
    RefreshDiagnostics {
        document: DocumentId,
        version: u64,
    },
    RequestCompletion {
        document: DocumentId,
        version: u64,
        position: LogicalPosition,
    },
    RequestHover {
        document: DocumentId,
        version: u64,
        position: LogicalPosition,
    },
    RequestSignatureHelp {
        document: DocumentId,
        version: u64,
        position: LogicalPosition,
    },
    RequestGoTo {
        document: DocumentId,
        version: u64,
        position: LogicalPosition,
    },
    RequestReferences {
        document: DocumentId,
        version: u64,
        position: LogicalPosition,
    },
    RequestRenamePreview {
        document: DocumentId,
        version: u64,
        position: LogicalPosition,
        new_name: String,
    },
    RequestCodeActions {
        document: DocumentId,
        version: u64,
    },
    RequestInlayHints {
        document: DocumentId,
        version: u64,
    },
    RequestDocumentSymbols {
        document: DocumentId,
        version: u64,
    },
    RequestWorkspaceSymbols {
        version: u64,
        query: String,
    },
    RequestFormatting {
        document: DocumentId,
        version: u64,
    },
    RestartLanguageServer,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LanguageResult {
    Syntax(Versioned<SpanSet>),
    Semantic(Versioned<SpanSet>),
    Diagnostics(Versioned<DiagnosticDashboard>),
    Completion(Versioned<CompletionView>),
    Hover(Versioned<HoverView>),
    Signature(Versioned<SignatureView>),
    GoTo(Versioned<LocationChooserView>),
    References(Versioned<LocationChooserView>),
    Rename(Versioned<RenamePreviewView>),
    CodeActions(Versioned<CodeActionView>),
    InlayHints(Versioned<InlayHintsView>),
    Symbols(Versioned<SymbolsView>),
    Formatting(Versioned<FormattingFeedbackView>),
    ServerStatus(LanguageServerStatus),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LanguageModel {
    pub document_version: u64,
    pub syntax: VersionedState<SpanSet>,
    pub semantic: VersionedState<SpanSet>,
    pub diagnostics: VersionedState<DiagnosticDashboard>,
    pub completion: VersionedState<CompletionView>,
    pub hover: VersionedState<HoverView>,
    pub signature: VersionedState<SignatureView>,
    pub go_to: VersionedState<LocationChooserView>,
    pub references: VersionedState<LocationChooserView>,
    pub rename: VersionedState<RenamePreviewView>,
    pub code_actions: VersionedState<CodeActionView>,
    pub inlay_hints: VersionedState<InlayHintsView>,
    pub symbols: VersionedState<SymbolsView>,
    pub formatting: VersionedState<FormattingFeedbackView>,
    pub server_status: LanguageServerStatus,
}

impl LanguageModel {
    #[must_use]
    pub fn new(document_version: u64) -> Self {
        Self {
            document_version,
            syntax: VersionedState::new(document_version),
            semantic: VersionedState::new(document_version),
            diagnostics: VersionedState::new(document_version),
            completion: VersionedState::new(document_version),
            hover: VersionedState::new(document_version),
            signature: VersionedState::new(document_version),
            go_to: VersionedState::new(document_version),
            references: VersionedState::new(document_version),
            rename: VersionedState::new(document_version),
            code_actions: VersionedState::new(document_version),
            inlay_hints: VersionedState::new(document_version),
            symbols: VersionedState::new(document_version),
            formatting: VersionedState::new(document_version),
            server_status: LanguageServerStatus::Stopped,
        }
    }

    pub fn set_document_version(&mut self, version: u64) {
        self.document_version = version;
        self.syntax.set_active_version(version);
        self.semantic.set_active_version(version);
        self.diagnostics.set_active_version(version);
        self.completion.set_active_version(version);
        self.hover.set_active_version(version);
        self.signature.set_active_version(version);
        self.go_to.set_active_version(version);
        self.references.set_active_version(version);
        self.rename.set_active_version(version);
        self.code_actions.set_active_version(version);
        self.inlay_hints.set_active_version(version);
        self.symbols.set_active_version(version);
        self.formatting.set_active_version(version);
    }

    #[must_use]
    pub fn apply_result(&mut self, result: LanguageResult) -> bool {
        match result {
            LanguageResult::Syntax(result) => self.syntax.apply(result),
            LanguageResult::Semantic(result) => self.semantic.apply(result),
            LanguageResult::Diagnostics(result) => self.diagnostics.apply(result),
            LanguageResult::Completion(result) => self.completion.apply(result),
            LanguageResult::Hover(result) => self.hover.apply(result),
            LanguageResult::Signature(result) => self.signature.apply(result),
            LanguageResult::GoTo(result) => self.go_to.apply(result),
            LanguageResult::References(result) => self.references.apply(result),
            LanguageResult::Rename(result) => self.rename.apply(result),
            LanguageResult::CodeActions(result) => self.code_actions.apply(result),
            LanguageResult::InlayHints(result) => self.inlay_hints.apply(result),
            LanguageResult::Symbols(result) => self.symbols.apply(result),
            LanguageResult::Formatting(result) => self.formatting.apply(result),
            LanguageResult::ServerStatus(status) => {
                self.server_status = status;
                true
            }
        }
    }
}

#[must_use]
pub fn render_panel(panel: &PanelState, columns: u16, rows: u16) -> Framebuffer {
    let mut frame = Framebuffer::new(columns, rows);
    render_line(&mut frame, 0, &panel.title, None);
    let mut row = 1_u16;
    for (index, panel_row) in panel.rows.iter().enumerate() {
        if row >= rows {
            break;
        }
        let selected = panel.selected == Some(index);
        render_line(
            &mut frame,
            row,
            &panel_row.cells,
            Some((selected, panel_row.severity)),
        );
        row = row.saturating_add(1);
    }
    if let Some(footer) = &panel.footer
        && row < rows
    {
        render_line(&mut frame, row, footer, None);
    }
    frame
}

#[must_use]
pub fn render_diagnostic_dashboard(
    dashboard: &DiagnosticDashboard,
    columns: u16,
    rows: u16,
) -> Framebuffer {
    let mut frame = Framebuffer::new(columns, rows);
    let header = PanelState::new(text_cells(&[
        "Diagnostics",
        "|",
        "summary",
        "|",
        "problems",
        "|",
        "overview",
    ]))
    .with_rows(vec![dashboard.status_row()])
    .with_footer(overview_row(
        "Gutter",
        &dashboard.gutter,
        "Ruler",
        &dashboard.ruler,
    ));
    render_line(&mut frame, 0, &header.title, None);
    if rows > 1 {
        render_line(&mut frame, 1, &header.rows[0].cells, None);
    }
    if rows > 2 {
        render_line(&mut frame, 2, header.footer.as_deref().unwrap_or(&[]), None);
    }
    for (offset, row) in dashboard.problems.groups.iter().enumerate() {
        let line = match u16::try_from(offset) {
            Ok(offset) => offset + 3,
            Err(_) => break,
        };
        if line >= rows {
            break;
        }
        let mut cells = vec![Glyph::severity("•", row.severity).panel()];
        cells.extend(text_cells(&[&row.path.to_string_lossy()]));
        if let Some(record) = row.items.first() {
            cells.extend(text_cells(&["->"]));
            cells.extend(text_cells(&[&record.diagnostic.message]));
        }
        render_line(
            &mut frame,
            line,
            &cells,
            Some((
                dashboard.problems.selected_group == Some(offset),
                Some(row.severity),
            )),
        );
    }
    frame
}

#[must_use]
pub fn render_completion_view(view: &CompletionView, columns: u16, rows: u16) -> Framebuffer {
    let mut frame = Framebuffer::new(columns, rows);
    render_split_view(&mut frame, &view.list, &view.details);
    frame
}

#[must_use]
pub fn render_hover_view(view: &HoverView, columns: u16, rows: u16) -> Framebuffer {
    render_panel(&view.card, columns, rows)
}

#[must_use]
pub fn render_signature_view(view: &SignatureView, columns: u16, rows: u16) -> Framebuffer {
    render_panel(&view.card, columns, rows)
}

#[must_use]
pub fn render_location_chooser(view: &LocationChooserView, columns: u16, rows: u16) -> Framebuffer {
    let panel_rows = view
        .choices
        .iter()
        .enumerate()
        .map(|(index, choice)| {
            let mut cells = choice.label.clone();
            cells.extend(text_cells(&["->"]));
            cells.extend(text_cells(&[&choice.path.to_string_lossy()]));
            let row = PanelRow::new(cells);
            if view.selected == Some(index) {
                row.severity(DiagnosticSeverity::Information)
            } else {
                row
            }
        })
        .collect();
    let panel = PanelState {
        title: view.title.clone(),
        rows: panel_rows,
        selected: view.selected,
        footer: None,
    };
    render_panel(&panel, columns, rows)
}

#[must_use]
pub fn render_rename_preview(view: &RenamePreviewView, columns: u16, rows: u16) -> Framebuffer {
    let mut frame = Framebuffer::new(columns, rows);
    render_line(&mut frame, 0, &view.title, None);
    if rows > 1 {
        render_line(&mut frame, 1, &view.before.title, None);
    }
    if rows > 2 {
        render_line(&mut frame, 2, &view.after.title, None);
    }
    if rows > 3 {
        render_line(&mut frame, 3, &view.conflicts.title, None);
    }
    frame
}

#[must_use]
pub fn render_code_actions(view: &CodeActionView, columns: u16, rows: u16) -> Framebuffer {
    render_panel(&view.list, columns, rows)
}

#[must_use]
pub fn render_inlay_hints(view: &InlayHintsView, columns: u16, rows: u16) -> Framebuffer {
    render_panel(&view.list, columns, rows)
}

#[must_use]
pub fn render_symbols(view: &SymbolsView, columns: u16, rows: u16) -> Framebuffer {
    render_panel(&view.list, columns, rows)
}

#[must_use]
pub fn render_formatting_feedback(
    view: &FormattingFeedbackView,
    columns: u16,
    rows: u16,
) -> Framebuffer {
    render_panel(&view.card, columns, rows)
}

#[must_use]
pub fn render_language_server(view: &LanguageServerView, columns: u16, rows: u16) -> Framebuffer {
    render_panel(&view.card, columns, rows)
}

/// Dump the framebuffer into a deterministic text format for snapshot assertions.
///
/// # Panics
///
/// Panics if the framebuffer reports a cell outside its declared size. The renderer only calls
/// this on in-bounds frames, so that would indicate a bug in the framebuffer implementation.
#[must_use]
pub fn dump_framebuffer(frame: &Framebuffer) -> String {
    let (columns, rows) = frame.size();
    let mut output = String::new();
    let mut wrote_line = false;
    for row in 0..rows {
        let mut last_significant = None;
        let mut selected_row = false;
        for column in (0..columns).rev() {
            let cell = frame
                .get(column, row)
                .expect("frame coordinates are bounded by frame.size()");
            if cell.continuation || is_significant(cell) {
                last_significant = Some(column);
                if cell.background == StyleRole::Selection {
                    selected_row = true;
                }
                break;
            }
        }
        let Some(last_significant) = last_significant else {
            continue;
        };
        if wrote_line {
            output.push('\n');
        }
        wrote_line = true;
        output.push(if selected_row { '>' } else { ' ' });
        for column in 0..columns {
            if column > last_significant {
                break;
            }
            if column > 0 {
                output.push('|');
            }
            let cell = frame
                .get(column, row)
                .expect("frame coordinates are bounded by frame.size()");
            if cell.continuation {
                output.push('↳');
            } else if !is_significant(cell) {
                output.push('·');
            } else {
                output.push_str(&cell.symbol);
            }
        }
    }
    output
}

fn render_split_view(frame: &mut Framebuffer, list: &PanelState, details: &PanelState) {
    let (columns, rows) = frame.size();
    let split = columns / 2;
    for row in 0..rows {
        let _ = frame.set(
            split,
            row,
            Cell {
                symbol: "│".to_owned(),
                foreground: StyleRole::Panel,
                background: StyleRole::Panel,
                bold: false,
                continuation: false,
            },
        );
    }

    let mut row = 0_u16;
    render_line_at(frame, row, 0, &list.title, None);
    row = row.saturating_add(1);
    for (index, entry) in list.rows.iter().enumerate() {
        if row >= rows {
            break;
        }
        render_line_at(
            frame,
            row,
            0,
            &entry.cells,
            Some((list.selected == Some(index), entry.severity)),
        );
        row = row.saturating_add(1);
    }

    let mut row = 0_u16;
    render_line_at(frame, row, split.saturating_add(1), &details.title, None);
    row = row.saturating_add(1);
    for (index, entry) in details.rows.iter().enumerate() {
        if row >= rows {
            break;
        }
        render_line_at(
            frame,
            row,
            split.saturating_add(1),
            &entry.cells,
            Some((details.selected == Some(index), entry.severity)),
        );
        row = row.saturating_add(1);
    }
}

fn render_line(
    frame: &mut Framebuffer,
    row: u16,
    cells: &[Glyph],
    row_state: Option<(bool, Option<DiagnosticSeverity>)>,
) {
    render_line_at(frame, row, 0, cells, row_state);
}

fn render_line_at(
    frame: &mut Framebuffer,
    row: u16,
    start_column: u16,
    cells: &[Glyph],
    row_state: Option<(bool, Option<DiagnosticSeverity>)>,
) {
    let (columns, rows) = frame.size();
    if row >= rows {
        return;
    }
    if start_column >= columns {
        return;
    }
    let mut column = start_column;
    for glyph in cells {
        if column >= columns {
            break;
        }
        let mut cell = glyph.to_cell();
        if let Some((selected, severity)) = row_state {
            if selected {
                cell.background = StyleRole::Selection;
            } else if matches!(cell.background, StyleRole::EditorBackground) {
                cell.background = StyleRole::Panel;
            }
            if let Some(severity) = severity {
                cell.foreground = severity_role(severity);
            }
        }
        if frame.set(column, row, cell).is_err() {
            break;
        }
        column = advance_column(frame, column, row);
    }
}

fn advance_column(frame: &Framebuffer, column: u16, row: u16) -> u16 {
    let (columns, rows) = frame.size();
    if row >= rows {
        return column;
    }
    let mut next = column.saturating_add(1);
    while next < columns {
        let Ok(cell) = frame.get(next, row) else {
            break;
        };
        if cell.continuation {
            next = next.saturating_add(1);
        } else {
            break;
        }
    }
    next
}

fn overview_row(
    left_label: &str,
    left_markers: &[SeverityMarker],
    right_label: &str,
    right_markers: &[SeverityMarker],
) -> Vec<Glyph> {
    let mut cells = Vec::new();
    cells.extend(text_cells(&[left_label]));
    for marker in left_markers {
        cells.extend(number_cells(marker.count, marker.severity));
    }
    cells.extend(text_cells(&[right_label]));
    for marker in right_markers {
        cells.extend(number_cells(marker.count, marker.severity));
    }
    cells
}

fn text_cells(parts: &[&str]) -> Vec<Glyph> {
    let mut cells = Vec::new();
    for part in parts {
        cells.extend(
            part.chars()
                .map(|character| Glyph::plain(character.to_string()).panel()),
        );
    }
    cells
}

fn number_cells(value: usize, severity: DiagnosticSeverity) -> Vec<Glyph> {
    value
        .to_string()
        .chars()
        .map(|character| Glyph::severity(character.to_string(), severity).panel())
        .collect()
}

fn severity_role(severity: DiagnosticSeverity) -> StyleRole {
    match severity {
        DiagnosticSeverity::Error => StyleRole::Error,
        DiagnosticSeverity::Warning => StyleRole::Warning,
        DiagnosticSeverity::Information => StyleRole::Information,
        DiagnosticSeverity::Hint => StyleRole::Hint,
    }
}

fn severity_rank(severity: DiagnosticSeverity) -> u8 {
    match severity {
        DiagnosticSeverity::Error => 0,
        DiagnosticSeverity::Warning => 1,
        DiagnosticSeverity::Information => 2,
        DiagnosticSeverity::Hint => 3,
    }
}

fn is_significant(cell: &Cell) -> bool {
    cell.symbol != " "
        || cell.foreground != StyleRole::EditorText
        || cell.background != StyleRole::EditorBackground
        || cell.bold
}

fn problem_sort_key(record: &DiagnosticRecord) -> (u8, String, String, usize) {
    (
        severity_rank(record.diagnostic.severity),
        record
            .workspace
            .map_or_else(String::new, |workspace| workspace.0.to_string()),
        record.path.to_string_lossy().to_string(),
        record.diagnostic.range.start.0,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(name: &str) -> String {
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let repository = manifest_dir
            .parent()
            .and_then(|path| path.parent())
            .expect("crate should be nested two levels below the worktree root");
        let path = repository
            .join("tests")
            .join("fixtures")
            .join("app-ui")
            .join("language")
            .join(name);
        std::fs::read_to_string(path).expect("fixture should exist")
    }

    fn snapshot(name: &str) -> String {
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let repository = manifest_dir
            .parent()
            .and_then(|path| path.parent())
            .expect("crate should be nested two levels below the worktree root");
        let path = repository
            .join("tests")
            .join("snapshots")
            .join("ui-language")
            .join(name);
        std::fs::read_to_string(path)
            .expect("snapshot should exist")
            .trim_end()
            .to_owned()
    }

    fn glyphs(values: &[&str]) -> Vec<Glyph> {
        text_cells(values)
    }

    fn grapheme_cells(values: &[&str]) -> Vec<Glyph> {
        values
            .iter()
            .map(|value| Glyph::plain(*value).panel())
            .collect()
    }

    fn diagnostic_record(
        workspace: Option<WorkspaceId>,
        path: &str,
        message: &str,
        severity: DiagnosticSeverity,
        version: u64,
        start: usize,
    ) -> DiagnosticRecord {
        DiagnosticRecord {
            workspace,
            path: PathBuf::from(path),
            diagnostic: Diagnostic {
                document: DocumentId(version),
                range: TextRange::new(
                    CharacterOffset(start),
                    CharacterOffset(start.saturating_add(1)),
                )
                .expect("valid range"),
                severity,
                message: message.to_owned(),
                source: Some("lsp".to_owned()),
                version: Some(version),
            },
            related: vec![RelatedLocation {
                label: text_cells(&["related", "界"]),
                path: PathBuf::from("src/related.rs"),
                position: LogicalPosition {
                    line: 2,
                    character: 4,
                },
            }],
        }
    }

    #[test]
    fn stale_results_are_suppressed_for_every_versioned_surface() {
        let mut model = LanguageModel::new(7);
        assert!(
            !model.apply_result(LanguageResult::Syntax(Versioned::new(
                6,
                SpanSet {
                    document: DocumentId(1),
                    version: 6,
                    spans: vec![StyledSpan {
                        range: TextRange::new(CharacterOffset(0), CharacterOffset(1))
                            .expect("valid range"),
                        style: TokenStyle::SyntaxKeyword,
                    }],
                },
            )))
        );
        assert!(model.syntax.current().is_none());

        assert!(
            !model.apply_result(LanguageResult::Diagnostics(Versioned::new(
                6,
                DiagnosticDashboard::from_records(vec![diagnostic_record(
                    Some(WorkspaceId(1)),
                    "src/main.rs",
                    "stale",
                    DiagnosticSeverity::Error,
                    6,
                    0,
                )]),
            )))
        );
        assert!(model.diagnostics.current().is_none());

        assert!(
            !model.apply_result(LanguageResult::Completion(Versioned::new(
                6,
                CompletionView {
                    list: PanelState::new(glyphs(&["completion"])),
                    details: PanelState::new(glyphs(&["details"])),
                },
            )))
        );
        assert!(model.completion.current().is_none());
    }

    #[test]
    fn problems_group_by_workspace_and_file() {
        let dashboard = DiagnosticDashboard::from_records(vec![
            diagnostic_record(
                Some(WorkspaceId(1)),
                "src/main.rs",
                "unused import",
                DiagnosticSeverity::Warning,
                7,
                1,
            ),
            diagnostic_record(
                Some(WorkspaceId(1)),
                "src/main.rs",
                "missing semicolon",
                DiagnosticSeverity::Error,
                7,
                3,
            ),
            diagnostic_record(
                Some(WorkspaceId(2)),
                "tests/ユニコード.rs",
                "wide glyph",
                DiagnosticSeverity::Information,
                7,
                5,
            ),
            diagnostic_record(
                Some(WorkspaceId(2)),
                "tests/ユニコード.rs",
                "hint",
                DiagnosticSeverity::Hint,
                7,
                8,
            ),
        ]);
        assert_eq!(dashboard.problems.groups.len(), 2);
        assert_eq!(
            dashboard.problems.groups[0].severity,
            DiagnosticSeverity::Error
        );
        assert_eq!(dashboard.summary.error, 1);
        assert_eq!(dashboard.summary.warning, 1);
        assert_eq!(dashboard.summary.information, 1);
        assert_eq!(dashboard.summary.hint, 1);
    }

    #[test]
    fn diagnostic_dashboard_snapshot_covers_all_severities() {
        let dashboard = DiagnosticDashboard::from_records(vec![
            diagnostic_record(
                Some(WorkspaceId(1)),
                "src/main.rs",
                fixture("diagnostics_error.txt").trim_end(),
                DiagnosticSeverity::Error,
                7,
                0,
            ),
            diagnostic_record(
                Some(WorkspaceId(1)),
                "src/lib.rs",
                fixture("diagnostics_warning.txt").trim_end(),
                DiagnosticSeverity::Warning,
                7,
                1,
            ),
            diagnostic_record(
                Some(WorkspaceId(2)),
                "tests/ユニコード.rs",
                fixture("diagnostics_information.txt").trim_end(),
                DiagnosticSeverity::Information,
                7,
                2,
            ),
            diagnostic_record(
                Some(WorkspaceId(2)),
                "tests/ユニコード.rs",
                fixture("diagnostics_hint.txt").trim_end(),
                DiagnosticSeverity::Hint,
                7,
                3,
            ),
        ]);
        let frame = render_diagnostic_dashboard(&dashboard, 64, 8);
        assert_eq!(
            dump_framebuffer(&frame),
            snapshot("diagnostic_dashboard.txt")
        );
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn completion_hover_signature_navigation_and_actions_snapshots() {
        let mut completion_row = text_cells(&["fmt"]);
        completion_row.extend(text_cells(&["format_document"]));
        completion_row.extend(text_cells(&["->"]));
        completion_row.extend(text_cells(&["applies workspace formatting"]));
        let mut hover_row = text_cells(&["hover"]);
        hover_row.extend(text_cells(&["界"]));
        hover_row.extend(text_cells(&["->"]));
        hover_row.extend(text_cells(&["details"]));
        let completion = CompletionView {
            list: PanelState::new(text_cells(&["Completion popup"]))
                .with_rows(vec![
                    PanelRow::new(completion_row),
                    PanelRow::new(hover_row),
                ])
                .with_selected(Some(0)),
            details: PanelState::new(text_cells(&["Details"])).with_rows(vec![
                PanelRow::new(text_cells(&["details supports unicode"])),
                PanelRow::new(text_cells(&["resolve on-demand"])),
            ]),
        };
        let hover = HoverView {
            card: PanelState::new(text_cells(&["Hover"])).with_rows(vec![PanelRow::new(
                text_cells(&["signature 界🙂 documented"]),
            )]),
        };
        let signature = SignatureView {
            card: PanelState::new(text_cells(&["Signature"]))
                .with_rows(vec![PanelRow::new(text_cells(&["fn select (value: T)"]))]),
        };
        let locations = LocationChooserView {
            title: text_cells(&["Go to chooser"]),
            choices: vec![
                LocationChoice {
                    label: text_cells(&["Definition 1"]),
                    document: DocumentId(7),
                    path: PathBuf::from("src/main.rs"),
                    position: LogicalPosition {
                        line: 10,
                        character: 2,
                    },
                },
                LocationChoice {
                    label: text_cells(&["Definition 2 界"]),
                    document: DocumentId(7),
                    path: PathBuf::from("src/lib.rs"),
                    position: LogicalPosition {
                        line: 20,
                        character: 5,
                    },
                },
            ],
            selected: Some(1),
        };
        let references = LocationChooserView {
            title: text_cells(&["References"]),
            choices: vec![LocationChoice {
                label: text_cells(&["references 3"]),
                document: DocumentId(7),
                path: PathBuf::from("tests/ユニコード.rs"),
                position: LogicalPosition {
                    line: 5,
                    character: 8,
                },
            }],
            selected: Some(0),
        };
        let rename = RenamePreviewView {
            title: text_cells(&["Rename preview"]),
            before: PanelState::new(text_cells(&["before"]))
                .with_rows(vec![PanelRow::new(text_cells(&["old_name"]))]),
            after: PanelState::new(text_cells(&["after"]))
                .with_rows(vec![PanelRow::new(text_cells(&["new_name"]))]),
            conflicts: PanelState::new(text_cells(&["conflicts"]))
                .with_rows(vec![PanelRow::new(text_cells(&["src/conflict.rs rename"]))]),
        };
        let code_actions = CodeActionView {
            list: PanelState::new(text_cells(&["Code actions"]))
                .with_rows(vec![
                    PanelRow::new(text_cells(&["Fix import"])),
                    PanelRow::new(text_cells(&["Apply missing semicolon"])),
                ])
                .with_selected(Some(0)),
        };
        let inlay_hints = InlayHintsView {
            list: PanelState::new(text_cells(&["Inlay hints"]))
                .with_rows(vec![PanelRow::new(text_cells(&["param value 界"]))]),
        };
        let symbols = SymbolsView {
            list: PanelState::new(text_cells(&["Symbols"]))
                .with_rows(vec![
                    PanelRow::new(text_cells(&["struct Document"])),
                    PanelRow::new(text_cells(&["fn render"])),
                ])
                .with_selected(Some(1)),
        };
        let formatting = FormattingFeedbackView {
            card: PanelState::new(text_cells(&["Formatting"]))
                .with_rows(vec![PanelRow::new(text_cells(&["applied 3 edits"]))]),
        };
        let server = LanguageServerView {
            card: PanelState::new(text_cells(&["LSP status"])).with_rows(vec![
                PanelRow::new(text_cells(&["Unavailable"])),
                PanelRow::new(text_cells(&["Crashed"])),
                PanelRow::new(text_cells(&["Restarting"])),
            ]),
        };

        assert_eq!(
            dump_framebuffer(&render_completion_view(&completion, 56, 8)),
            snapshot("completion_view.txt")
        );
        assert_eq!(
            dump_framebuffer(&render_hover_view(&hover, 40, 4)),
            snapshot("hover_view.txt")
        );
        assert_eq!(
            dump_framebuffer(&render_signature_view(&signature, 40, 4)),
            snapshot("signature_view.txt")
        );
        assert_eq!(
            dump_framebuffer(&render_location_chooser(&locations, 56, 5)),
            snapshot("goto_view.txt")
        );
        assert_eq!(
            dump_framebuffer(&render_location_chooser(&references, 56, 4)),
            snapshot("references_view.txt")
        );
        assert_eq!(
            dump_framebuffer(&render_rename_preview(&rename, 56, 4)),
            snapshot("rename_view.txt")
        );
        assert_eq!(
            dump_framebuffer(&render_code_actions(&code_actions, 56, 4)),
            snapshot("code_actions_view.txt")
        );
        assert_eq!(
            dump_framebuffer(&render_inlay_hints(&inlay_hints, 56, 4)),
            snapshot("inlay_hints_view.txt")
        );
        assert_eq!(
            dump_framebuffer(&render_symbols(&symbols, 56, 4)),
            snapshot("symbols_view.txt")
        );
        assert_eq!(
            dump_framebuffer(&render_formatting_feedback(&formatting, 40, 4)),
            snapshot("formatting_view.txt")
        );
        assert_eq!(
            dump_framebuffer(&render_language_server(&server, 40, 4)),
            snapshot("server_status_view.txt")
        );
    }

    #[test]
    fn unicode_grapheme_ranges_survive_frame_rendering() {
        let panel = PanelState::new(text_cells(&["Unicode"])).with_rows(vec![PanelRow::new(
            grapheme_cells(&["e\u{301}", "界", "🙂", "α"]),
        )]);
        let frame = render_panel(&panel, 12, 3);
        assert_eq!(dump_framebuffer(&frame), snapshot("unicode_frame.txt"));
    }
}
