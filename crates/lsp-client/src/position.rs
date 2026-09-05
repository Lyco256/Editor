#![allow(clippy::missing_errors_doc)]

use std::sync::Arc;

use editor_types::LogicalPosition;
use thiserror::Error;

use crate::protocol::{self, Position, PositionEncoding, ProtocolError};

#[derive(Debug, Clone, Copy)]
pub struct PositionMapper<'a> {
    snapshot: &'a TextSnapshot,
    encoding: PositionEncoding,
}

impl<'a> PositionMapper<'a> {
    #[must_use]
    pub fn new(snapshot: &'a TextSnapshot, encoding: PositionEncoding) -> Self {
        Self { snapshot, encoding }
    }

    pub fn to_lsp(&self, position: LogicalPosition) -> Result<Position, PositionError> {
        protocol::editor_position_to_lsp(self.snapshot.text(), position, self.encoding)
            .map_err(PositionError::from)
    }

    pub fn from_lsp(&self, position: Position) -> Result<LogicalPosition, PositionError> {
        protocol::lsp_position_to_editor(self.snapshot.text(), position, self.encoding)
            .map_err(PositionError::from)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextSnapshot {
    text: Arc<str>,
}

impl TextSnapshot {
    #[must_use]
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: Arc::from(text.into()),
        }
    }

    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }

    #[must_use]
    pub fn len_chars(&self) -> usize {
        self.text.chars().count()
    }
}

#[derive(Debug, Error)]
pub enum PositionError {
    #[error(transparent)]
    Protocol(#[from] ProtocolError),
}

impl From<PositionError> for ProtocolError {
    fn from(error: PositionError) -> Self {
        match error {
            PositionError::Protocol(error) => error,
        }
    }
}
