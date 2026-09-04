//! Incremental syntax service boundary.

use editor_types::DocumentId;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParseTicket {
    pub document: DocumentId,
    pub version: u64,
}
