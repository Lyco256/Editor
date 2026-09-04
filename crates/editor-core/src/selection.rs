use crate::{CharacterOffset, EditorError, Result, TextRange};

/// A conventional anchor/active selection. Equal endpoints represent a cursor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Selection {
    pub anchor: CharacterOffset,
    pub active: CharacterOffset,
}

impl Selection {
    #[must_use]
    pub const fn cursor(offset: CharacterOffset) -> Self {
        Self {
            anchor: offset,
            active: offset,
        }
    }

    #[must_use]
    pub const fn new(anchor: CharacterOffset, active: CharacterOffset) -> Self {
        Self { anchor, active }
    }

    #[must_use]
    pub fn is_cursor(self) -> bool {
        self.anchor == self.active
    }

    #[must_use]
    pub fn range(self) -> TextRange {
        if self.anchor <= self.active {
            TextRange {
                start: self.anchor,
                end: self.active,
            }
        } else {
            TextRange {
                start: self.active,
                end: self.anchor,
            }
        }
    }

    #[must_use]
    pub const fn cursor_offset(self) -> CharacterOffset {
        self.active
    }

    pub(crate) fn with_active(self, active: CharacterOffset, extend: bool) -> Self {
        if extend {
            Self { active, ..self }
        } else {
            Self::cursor(active)
        }
    }
}

/// One or more selections with a distinguished primary selection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectionSet {
    selections: Vec<Selection>,
    primary: usize,
}

impl Default for SelectionSet {
    fn default() -> Self {
        Self::single(Selection::cursor(CharacterOffset(0)))
    }
}

impl SelectionSet {
    #[must_use]
    pub fn single(selection: Selection) -> Self {
        Self {
            selections: vec![selection],
            primary: 0,
        }
    }

    pub fn new(selections: Vec<Selection>, primary: usize) -> Result<Self> {
        if selections.is_empty() {
            return Err(EditorError::EmptySelectionSet);
        }
        if primary >= selections.len() {
            return Err(EditorError::InvalidPrimarySelection {
                primary,
                length: selections.len(),
            });
        }
        let primary_selection = selections[primary];
        let mut selections = selections;
        selections.sort_by_key(|selection| {
            let range = selection.range();
            (range.start, range.end, selection.anchor, selection.active)
        });
        selections.dedup();
        let primary = selections
            .iter()
            .position(|selection| *selection == primary_selection)
            .unwrap_or(0);
        Ok(Self {
            selections,
            primary,
        })
    }

    #[must_use]
    pub fn selections(&self) -> &[Selection] {
        &self.selections
    }

    #[must_use]
    pub fn primary(&self) -> Selection {
        self.selections[self.primary]
    }

    #[must_use]
    pub const fn primary_index(&self) -> usize {
        self.primary
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.selections.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.selections.is_empty()
    }

    pub(crate) fn mapped(&self, mut mapper: impl FnMut(Selection) -> Selection) -> Result<Self> {
        Self::new(
            self.selections.iter().copied().map(&mut mapper).collect(),
            self.primary,
        )
    }
}
