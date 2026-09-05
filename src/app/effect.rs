//! Typed asynchronous work requested by state updates.

use editor_types::RequestId;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExternalProcessKind {
    LanguageServer,
    Formatter,
    Git,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessSpec {
    pub executable: String,
    pub arguments: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Effect {
    SaveDocument {
        path: PathBuf,
        text: String,
        encoding: workspace_core::EncodingKind,
        with_bom: bool,
        line_endings: workspace_core::LineEndings,
    },
    SaveDocumentAs {
        path: PathBuf,
        text: String,
        encoding: workspace_core::EncodingKind,
        with_bom: bool,
        line_endings: workspace_core::LineEndings,
    },
    ExternalProcess {
        request: RequestId,
        kind: ExternalProcessKind,
        spec: ProcessSpec,
    },
    RefreshGitStatus {
        request: RequestId,
        root: PathBuf,
    },
    RefreshExplorer {
        request: RequestId,
        roots: Vec<PathBuf>,
    },
    ClipboardWrite {
        request: RequestId,
        text: String,
        cut: bool,
    },
    ClipboardRead {
        request: RequestId,
    },
    FormatDocument {
        request: RequestId,
        text: String,
        spec: ProcessSpec,
        timeout_ms: u64,
    },
    ApplyReplacementPlan {
        request: RequestId,
        plan: workspace_core::ReplacementPlan,
    },
    SearchWorkspace {
        session_id: u64,
        options: workspace_core::SearchOptions,
    },
    CancelSearch {
        session_id: u64,
    },
    LspRequest {
        request: RequestId,
        version: u64,
        spec: ProcessSpec,
        method: String,
        params: serde_json::Value,
    },
    RefreshSyntax {
        request: RequestId,
        document: editor_types::DocumentId,
        version: u64,
        path: Option<PathBuf>,
        text: String,
        large_file: bool,
    },
    Render,
}

impl Effect {
    #[must_use]
    pub const fn requires_trusted_workspace(&self) -> bool {
        matches!(
            self,
            Self::ExternalProcess { .. }
                | Self::RefreshGitStatus { .. }
                | Self::FormatDocument { .. }
                | Self::LspRequest { .. }
        )
    }
}
