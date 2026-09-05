//! Completion events returned by background services.

use editor_types::{GitStatusSummary, OutputMessage, RequestId};
use std::path::PathBuf;

use super::effect::ExternalProcessKind;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    DocumentSaved {
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
