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
    /// Executes a previously planned filesystem operation after explicit UI confirmation.
    FileOperation {
        request: RequestId,
        plan: workspace_core::FileOperationPlan,
    },
    ExternalProcess {
        request: RequestId,
        kind: ExternalProcessKind,
        spec: ProcessSpec,
    },
    /// Stops the persistent language-server session when trust is revoked.
    StopLanguageServer {
        request: RequestId,
    },
    GitHunk {
        request: RequestId,
        root: PathBuf,
        hunk: vcs_git::GitDiffHunk,
        reverse: bool,
    },
    GitDiscard {
        request: RequestId,
        root: PathBuf,
        plan: vcs_git::GitDiscardPlan,
        confirmed: bool,
    },
    RefreshGitStatus {
        request: RequestId,
        root: PathBuf,
    },
    RefreshExplorer {
        request: RequestId,
        roots: Vec<PathBuf>,
        expanded: Vec<PathBuf>,
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
    /// Sends a JSON-RPC notification through the persistent language-server session.
    LspNotification {
        request: RequestId,
        method: String,
        params: serde_json::Value,
    },
    /// Responds to a server-originated JSON-RPC request after root policy has handled it.
    LspServerResponse {
        request: RequestId,
        id: lsp_client::protocol::RequestId,
        result: Option<serde_json::Value>,
        error: Option<lsp_client::protocol::JsonRpcErrorObject>,
    },
    /// Applies a server-originated `WorkspaceEdit` on a background worker.
    LspWorkspaceEdit {
        request: RequestId,
        id: lsp_client::protocol::RequestId,
        edit: serde_json::Value,
        roots: Vec<PathBuf>,
        encoding: lsp_client::protocol::PositionEncoding,
    },
    /// Applies a client-originated workspace edit that includes unopened documents.
    LspApplyWorkspaceEdit {
        request: RequestId,
        edit: serde_json::Value,
        roots: Vec<PathBuf>,
        encoding: lsp_client::protocol::PositionEncoding,
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
                | Self::GitHunk { .. }
                | Self::GitDiscard { .. }
                | Self::RefreshGitStatus { .. }
                | Self::FormatDocument { .. }
                | Self::LspRequest { .. }
                | Self::LspNotification { .. }
                | Self::LspWorkspaceEdit { .. }
                | Self::LspApplyWorkspaceEdit { .. }
        )
    }
}
