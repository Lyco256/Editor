//! Unicode-safe virtual framebuffer storage.

use editor_types::StyleRole;
use thiserror::Error;
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

/// One display cell. A wide grapheme owns its first cell and marks following cells as continuations.
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

/// A fixed-size row-major virtual terminal frame.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Framebuffer {
    columns: u16,
    rows: u16,
    cells: Vec<Cell>,
}

/// A rejected framebuffer operation.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum FramebufferError {
    #[error("cell ({column}, {row}) is outside framebuffer {columns}x{rows}")]
    OutOfBounds {
        column: u16,
        row: u16,
        columns: u16,
        rows: u16,
    },
    #[error("a framebuffer cell must contain exactly one grapheme cluster")]
    InvalidGrapheme,
    #[error("zero-width grapheme clusters cannot own a framebuffer cell")]
    ZeroWidthGrapheme,
    #[error(
        "grapheme of width {width} does not fit at column {column} in a {columns}-column frame"
    )]
    GraphemeDoesNotFit {
        column: u16,
        columns: u16,
        width: usize,
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

    /// Replaces one displayed grapheme and repairs any overlapping wide-cell ownership.
    ///
    /// # Errors
    ///
    /// Returns a typed bounds or grapheme error. The frame is unchanged when validation fails.
    pub fn set(&mut self, column: u16, row: u16, mut cell: Cell) -> Result<(), FramebufferError> {
        self.index(column, row)?;
        let mut graphemes = cell.symbol.graphemes(true);
        let Some(grapheme) = graphemes.next() else {
            return Err(FramebufferError::InvalidGrapheme);
        };
        if graphemes.next().is_some() {
            return Err(FramebufferError::InvalidGrapheme);
        }
        let width = grapheme.width();
        if width == 0 {
            return Err(FramebufferError::ZeroWidthGrapheme);
        }
        if usize::from(column) + width > usize::from(self.columns) {
            return Err(FramebufferError::GraphemeDoesNotFit {
                column,
                columns: self.columns,
                width,
            });
        }
        let width_u16 = u16::try_from(width).map_err(|_| FramebufferError::GraphemeDoesNotFit {
            column,
            columns: self.columns,
            width,
        })?;

        for target in column..column + width_u16 {
            self.clear_grapheme_at(target, row);
        }
        self.clear_grapheme_at(column, row);

        cell.continuation = false;
        let owner = self.index(column, row)?;
        self.cells[owner] = cell.clone();
        for offset in 1..width {
            let continuation_column = column
                + u16::try_from(offset).map_err(|_| FramebufferError::GraphemeDoesNotFit {
                    column,
                    columns: self.columns,
                    width,
                })?;
            let continuation = self.index(continuation_column, row)?;
            self.cells[continuation] = Cell {
                symbol: String::new(),
                continuation: true,
                ..cell.clone()
            };
        }
        Ok(())
    }

    /// Clears the whole grapheme occupying a coordinate.
    ///
    /// # Errors
    ///
    /// Returns [`FramebufferError::OutOfBounds`] when the coordinate is outside this frame.
    pub fn clear(&mut self, column: u16, row: u16) -> Result<(), FramebufferError> {
        self.index(column, row)?;
        self.clear_grapheme_at(column, row);
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

    pub(crate) fn cells(&self) -> &[Cell] {
        &self.cells
    }

    fn clear_grapheme_at(&mut self, column: u16, row: u16) {
        let mut owner_column = column;
        while owner_column > 0
            && self.cells[usize::from(row) * usize::from(self.columns) + usize::from(owner_column)]
                .continuation
        {
            owner_column -= 1;
        }
        let row_start = usize::from(row) * usize::from(self.columns);
        let owner_index = row_start + usize::from(owner_column);
        self.cells[owner_index] = Cell::default();
        let mut next = owner_column.saturating_add(1);
        while next < self.columns && self.cells[row_start + usize::from(next)].continuation {
            self.cells[row_start + usize::from(next)] = Cell::default();
            next += 1;
        }
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

#[cfg(test)]
mod tests {
    use super::{Cell, Framebuffer, FramebufferError};

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

    #[test]
    fn wide_grapheme_marks_and_repairs_continuation() {
        let mut frame = Framebuffer::new(4, 1);
        frame
            .set(
                1,
                0,
                Cell {
                    symbol: "界".to_owned(),
                    ..Cell::default()
                },
            )
            .expect("wide glyph fits");
        assert!(frame.get(2, 0).expect("continuation exists").continuation);
        frame
            .set(
                2,
                0,
                Cell {
                    symbol: "x".to_owned(),
                    ..Cell::default()
                },
            )
            .expect("replacement is valid");
        assert_eq!(frame.get(1, 0), Ok(&Cell::default()));
        assert_eq!(frame.get(2, 0).expect("replacement exists").symbol, "x");
    }

    #[test]
    fn combining_sequence_is_one_safe_cell() {
        let mut frame = Framebuffer::new(1, 1);
        frame
            .set(
                0,
                0,
                Cell {
                    symbol: "e\u{301}".to_owned(),
                    ..Cell::default()
                },
            )
            .expect("combined grapheme is valid");
        assert_eq!(frame.get(0, 0).expect("cell exists").symbol, "e\u{301}");
    }

    #[test]
    fn invalid_symbol_does_not_mutate_frame() {
        let mut frame = Framebuffer::new(1, 1);
        let before = frame.clone();
        let result = frame.set(
            0,
            0,
            Cell {
                symbol: "ab".to_owned(),
                ..Cell::default()
            },
        );
        assert_eq!(result, Err(FramebufferError::InvalidGrapheme));
        assert_eq!(frame, before);
    }
}
