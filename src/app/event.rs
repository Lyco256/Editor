//! Completion events returned by background services.

use editor_types::{GitStatusSummary, OutputMessage, RequestId};
use std::path::PathBuf;

use super::effect::ExternalProcessKind;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    DocumentSaved {
        path: PathBuf,
    },
    DocumentSavedAs {
        path: PathBuf,
    },
    DocumentSaveFailed {
        path: PathBuf,
        message: OutputMessage,
    },
    GitStatusUpdated {
        request: RequestId,
        summary: GitStatusSummary,
    },
    ExplorerUpdated {
        request: RequestId,
        entries: Vec<ExplorerEntryData>,
    },
    ClipboardWritten {
        request: RequestId,
        cut: bool,
    },
    ClipboardRead {
        request: RequestId,
        text: String,
    },
    ClipboardFailed {
        request: RequestId,
        message: OutputMessage,
    },
    DocumentFormatted {
        request: RequestId,
        replacement: String,
    },
    DocumentFormatFailed {
        request: RequestId,
        message: OutputMessage,
    },
    LanguageDiagnostics {
        request: RequestId,
        params: lsp_client::protocol::PublishDiagnosticsParams,
    },
    ReplacementApplied {
        request: RequestId,
        report: workspace_core::ReplacementReport,
    },
    ReplacementFailed {
        request: RequestId,
        message: OutputMessage,
    },
    SyntaxUpdated {
        request: RequestId,
        update: syntax_engine::SyntaxUpdate,
    },
    SearchStarted {
        session_id: u64,
        query: String,
        options: app_ui::workspace::SearchOptionsView,
    },
    SearchResult {
        session_id: u64,
        result: app_ui::workspace::SearchResult,
    },
    SearchFinished {
        session_id: u64,
    },
    SearchCancelled {
        session_id: u64,
    },
    SearchFailed {
        session_id: u64,
        message: OutputMessage,
    },
    EffectCompleted(RequestId),
    EffectFailed {
        request: RequestId,
        message: OutputMessage,
    },
    ExternalProcessBlocked {
        request: RequestId,
        kind: ExternalProcessKind,
    },
    Output(OutputMessage),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExplorerEntryData {
    pub path: PathBuf,
    pub depth: u8,
}
