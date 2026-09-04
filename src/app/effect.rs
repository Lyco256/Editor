//! Typed asynchronous work requested by state updates.

use editor_types::RequestId;

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
    ExternalProcess {
        request: RequestId,
        kind: ExternalProcessKind,
        spec: ProcessSpec,
    },
    Render,
}

impl Effect {
    #[must_use]
    pub const fn requires_trusted_workspace(&self) -> bool {
        matches!(self, Self::ExternalProcess { .. })
    }
}
