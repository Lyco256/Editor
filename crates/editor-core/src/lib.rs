//! Unicode text editing domain boundary.

use editor_types::DocumentId;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DocumentDescriptor {
    pub id: DocumentId,
    pub version: u64,
    pub large_file_mode: bool,
}
