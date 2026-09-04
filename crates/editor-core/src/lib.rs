//! Unicode-safe, transaction-oriented text editing primitives for Editor.
//!
//! [`TextBuffer`] is the only mutable text owner. The backing rope is deliberately
//! private; consumers that need a stable read view use [`TextSnapshot`].
#![allow(clippy::missing_errors_doc)]

mod buffer;
mod fold;
mod search;
mod selection;
mod smart;
mod transaction;

pub use buffer::{
    AppliedTransaction, DEFAULT_LARGE_FILE_THRESHOLD, SemanticServicePolicy, TextBuffer,
    TextSnapshot,
};
pub use editor_types::{CharacterOffset, LogicalPosition, TextRange};
pub use fold::{FoldRegion, FoldSet};
pub use search::{FindMatch, FindOptions, SearchKind};
pub use selection::{Selection, SelectionSet};
pub use smart::{IndentStyle, PairConfig, SmartEditOutcome};
pub use transaction::{Edit, Transaction, TransactionBuilder};

/// Immutable document metadata shared with asynchronous syntax and language services.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DocumentDescriptor {
    pub id: editor_types::DocumentId,
    pub version: u64,
    pub large_file_mode: bool,
}

/// Typed failures caused by invalid input to the editing engine.
#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq)]
pub enum EditorError {
    #[error("character offset {offset} is outside the document (length {length})")]
    OffsetOutOfBounds { offset: usize, length: usize },
    #[error("range {start}..{end} is invalid for document length {length}")]
    InvalidRange {
        start: usize,
        end: usize,
        length: usize,
    },
    #[error("offset {offset} splits a CRLF sequence and is not a valid caret boundary")]
    InvalidCaretBoundary { offset: usize },
    #[error("logical position {line}:{character} is outside the document")]
    InvalidPosition { line: u32, character: u32 },
    #[error("transaction edits overlap at character offset {offset}")]
    OverlappingEdits { offset: usize },
    #[error("a selection set must contain at least one selection")]
    EmptySelectionSet,
    #[error("primary selection index {primary} is outside a set of length {length}")]
    InvalidPrimarySelection { primary: usize, length: usize },
    #[error("invalid regular expression: {message}")]
    InvalidRegex { message: String },
    #[error("smart pair delimiters must both be non-empty")]
    InvalidPairConfiguration,
    #[error("fold region {start_line}..{end_line} must span at least two logical lines")]
    InvalidFoldRegion { start_line: u32, end_line: u32 },
}

pub type Result<T> = std::result::Result<T, EditorError>;
