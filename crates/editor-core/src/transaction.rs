use crate::{CharacterOffset, EditorError, Result, SelectionSet, TextRange};

/// A replacement expressed in character offsets against the transaction's input text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Edit {
    pub range: TextRange,
    pub replacement: String,
}

impl Edit {
    #[must_use]
    pub fn insert(at: CharacterOffset, text: impl Into<String>) -> Self {
        Self {
            range: TextRange { start: at, end: at },
            replacement: text.into(),
        }
    }

    #[must_use]
    pub fn delete(range: TextRange) -> Self {
        Self {
            range,
            replacement: String::new(),
        }
    }

    #[must_use]
    pub fn replace(range: TextRange, text: impl Into<String>) -> Self {
        Self {
            range,
            replacement: text.into(),
        }
    }
}

/// A deterministic group of simultaneous edits and an optional resulting selection set.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Transaction {
    pub(crate) edits: Vec<Edit>,
    pub(crate) selection_after: Option<SelectionSet>,
}

impl Transaction {
    pub fn new(edits: Vec<Edit>) -> Result<Self> {
        TransactionBuilder::new().extend(edits).build()
    }

    #[must_use]
    pub fn edits(&self) -> &[Edit] {
        &self.edits
    }

    #[must_use]
    pub fn selection_after(&self) -> Option<&SelectionSet> {
        self.selection_after.as_ref()
    }

    #[must_use]
    pub fn with_selection_after(mut self, selections: SelectionSet) -> Self {
        self.selection_after = Some(selections);
        self
    }
}

/// Builder that sorts edits and can explicitly coalesce identical duplicate edits.
#[derive(Debug, Default)]
pub struct TransactionBuilder {
    edits: Vec<Edit>,
    selection_after: Option<SelectionSet>,
    normalize_identical: bool,
}

impl TransactionBuilder {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            edits: Vec::new(),
            selection_after: None,
            normalize_identical: false,
        }
    }

    #[must_use]
    pub fn edit(mut self, edit: Edit) -> Self {
        self.edits.push(edit);
        self
    }

    #[must_use]
    pub fn extend(mut self, edits: impl IntoIterator<Item = Edit>) -> Self {
        self.edits.extend(edits);
        self
    }

    #[must_use]
    pub fn selection_after(mut self, selections: SelectionSet) -> Self {
        self.selection_after = Some(selections);
        self
    }

    /// Coalesce exact duplicate edits. Other overlaps remain errors.
    #[must_use]
    pub const fn normalize_identical_edits(mut self) -> Self {
        self.normalize_identical = true;
        self
    }

    pub fn build(mut self) -> Result<Transaction> {
        self.edits.sort_by(|left, right| {
            (left.range.start, left.range.end, &left.replacement).cmp(&(
                right.range.start,
                right.range.end,
                &right.replacement,
            ))
        });
        if self.normalize_identical {
            self.edits.dedup();
        }
        for pair in self.edits.windows(2) {
            let left = &pair[0];
            let right = &pair[1];
            let ranges_overlap = right.range.start < left.range.end;
            let insertions_collide = right.range.start == left.range.start
                && (right.range.start == right.range.end || left.range.start == left.range.end);
            if ranges_overlap || insertions_collide {
                return Err(EditorError::OverlappingEdits {
                    offset: right.range.start.0,
                });
            }
        }
        Ok(Transaction {
            edits: self.edits,
            selection_after: self.selection_after,
        })
    }
}
