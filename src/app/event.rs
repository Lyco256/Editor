//! Completion events returned by background services.

use editor_types::{OutputMessage, RequestId};

use super::effect::ExternalProcessKind;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
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
