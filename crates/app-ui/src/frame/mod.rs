//! Shared frame construction contracts.

use editor_types::StyleRole;
use terminal_backend::{Cell, Framebuffer};

#[must_use]
pub fn empty_frame(columns: u16, rows: u16) -> Framebuffer {
    Framebuffer::new(columns, rows)
}

#[must_use]
pub fn status_cell(symbol: impl Into<String>) -> Cell {
    Cell {
        symbol: symbol.into(),
        foreground: StyleRole::EditorText,
        background: StyleRole::StatusBar,
        bold: false,
        continuation: false,
    }
}
