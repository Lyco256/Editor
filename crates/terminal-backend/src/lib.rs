//! Terminal lifecycle and virtual-framebuffer boundary.

use editor_types::{StyleRole, TerminalCapabilities};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cell {
    pub symbol: String,
    pub foreground: StyleRole,
    pub background: StyleRole,
    pub bold: bool,
    pub continuation: bool,
}

impl Default for Cell {
    fn default() -> Self {
        Self {
            symbol: " ".to_owned(),
            foreground: StyleRole::EditorText,
            background: StyleRole::EditorBackground,
            bold: false,
            continuation: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Framebuffer {
    columns: u16,
    rows: u16,
    cells: Vec<Cell>,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum FramebufferError {
    #[error("cell ({column}, {row}) is outside framebuffer {columns}x{rows}")]
    OutOfBounds {
        column: u16,
        row: u16,
        columns: u16,
        rows: u16,
    },
}

impl Framebuffer {
    #[must_use]
    pub fn new(columns: u16, rows: u16) -> Self {
        let len = usize::from(columns) * usize::from(rows);
        Self {
            columns,
            rows,
            cells: vec![Cell::default(); len],
        }
    }

    #[must_use]
    pub const fn size(&self) -> (u16, u16) {
        (self.columns, self.rows)
    }

    /// Replaces one framebuffer cell.
    ///
    /// # Errors
    ///
    /// Returns [`FramebufferError::OutOfBounds`] when the coordinate is outside this frame.
    pub fn set(&mut self, column: u16, row: u16, cell: Cell) -> Result<(), FramebufferError> {
        let index = self.index(column, row)?;
        self.cells[index] = cell;
        Ok(())
    }

    /// Returns one framebuffer cell.
    ///
    /// # Errors
    ///
    /// Returns [`FramebufferError::OutOfBounds`] when the coordinate is outside this frame.
    pub fn get(&self, column: u16, row: u16) -> Result<&Cell, FramebufferError> {
        self.index(column, row).map(|index| &self.cells[index])
    }

    fn index(&self, column: u16, row: u16) -> Result<usize, FramebufferError> {
        if column >= self.columns || row >= self.rows {
            return Err(FramebufferError::OutOfBounds {
                column,
                row,
                columns: self.columns,
                rows: self.rows,
            });
        }
        Ok(usize::from(row) * usize::from(self.columns) + usize::from(column))
    }
}

pub trait TerminalAdapter {
    type Error: std::error::Error + Send + Sync + 'static;

    fn capabilities(&self) -> TerminalCapabilities;
    /// Activates terminal modes owned by the adapter.
    ///
    /// # Errors
    ///
    /// Returns the adapter error when terminal modes cannot be activated safely.
    fn enter(&mut self) -> Result<(), Self::Error>;
    /// Presents the next framebuffer.
    ///
    /// # Errors
    ///
    /// Returns the adapter error when frame output fails.
    fn render(&mut self, frame: &Framebuffer) -> Result<(), Self::Error>;
    /// Restores all terminal modes changed by [`Self::enter`].
    ///
    /// # Errors
    ///
    /// Returns the adapter error when restoration is incomplete.
    fn restore(&mut self) -> Result<(), Self::Error>;
}

#[cfg(test)]
mod tests {
    use super::{Cell, Framebuffer};

    #[test]
    fn cell_can_be_replaced() {
        let mut frame = Framebuffer::new(2, 1);
        let cell = Cell {
            symbol: "x".to_owned(),
            ..Cell::default()
        };
        frame.set(1, 0, cell.clone()).expect("valid cell");
        assert_eq!(frame.get(1, 0), Ok(&cell));
    }
}
