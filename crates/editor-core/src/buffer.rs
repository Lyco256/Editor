use std::{fmt, sync::Arc};

use ropey::Rope;
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

use crate::{
    CharacterOffset, Edit, EditorError, LogicalPosition, Result, Selection, SelectionSet,
    TextRange, Transaction, TransactionBuilder,
};

pub const DEFAULT_LARGE_FILE_THRESHOLD: usize = 32 * 1024 * 1024;

/// Whether whole-document semantic services may run for this document.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SemanticServicePolicy {
    Enabled,
    SuppressedLargeFile,
}

/// Result metadata for a transaction application.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AppliedTransaction {
    pub changed: bool,
    pub version: u64,
}

/// An immutable, thread-safe read view detached from future buffer mutations.
#[derive(Debug, Clone)]
pub struct TextSnapshot {
    text: Arc<str>,
    version: u64,
}

impl TextSnapshot {
    /// Creates an immutable snapshot for compatibility projections (without edit history).
    #[must_use]
    pub fn from_text(text: impl Into<Arc<str>>) -> Self {
        Self {
            text: text.into(),
            version: 0,
        }
    }
    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }

    #[must_use]
    pub const fn version(&self) -> u64 {
        self.version
    }

    #[must_use]
    pub fn len_chars(&self) -> usize {
        self.text.chars().count()
    }

    #[must_use]
    pub fn len_bytes(&self) -> usize {
        self.text.len()
    }

    #[must_use]
    pub fn line_count(&self) -> usize {
        self.text
            .chars()
            .filter(|character| *character == '\n')
            .count()
            + 1
    }

    pub fn text_in_range(&self, range: TextRange) -> Result<&str> {
        validate_range_in_text(&self.text, range)?;
        let start = char_to_byte(&self.text, range.start.0);
        let end = char_to_byte(&self.text, range.end.0);
        Ok(&self.text[start..end])
    }

    pub fn line_text(&self, line: u32) -> Result<&str> {
        let (start, content_end, _) = line_bounds(&self.text, line)
            .ok_or(EditorError::InvalidPosition { line, character: 0 })?;
        Ok(&self.text[char_to_byte(&self.text, start)..char_to_byte(&self.text, content_end)])
    }

    pub fn offset_to_position(&self, offset: CharacterOffset) -> Result<LogicalPosition> {
        offset_to_position_in_text(&self.text, offset)
    }

    pub fn position_to_offset(&self, position: LogicalPosition) -> Result<CharacterOffset> {
        position_to_offset_in_text(&self.text, position)
    }

    pub fn display_column(&self, offset: CharacterOffset, tab_width: usize) -> Result<usize> {
        display_column_in_text(&self.text, offset, tab_width)
    }

    pub fn offset_for_display_column(
        &self,
        line: u32,
        column: usize,
        tab_width: usize,
    ) -> Result<CharacterOffset> {
        offset_for_display_column_in_text(&self.text, line, column, tab_width)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AutoPairMarker {
    pub open_start: usize,
    pub open: String,
    pub close_start: usize,
    pub close: String,
}

impl AutoPairMarker {
    fn end(&self) -> usize {
        self.close_start + self.close.chars().count()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct HistoryEntry {
    forward: Transaction,
    inverse: Transaction,
    selections_before: SelectionSet,
    selections_after: SelectionSet,
    markers_before: Vec<AutoPairMarker>,
    markers_after: Vec<AutoPairMarker>,
    state_before: u64,
    state_after: u64,
}

/// Mutable Unicode text and all transaction-scoped editor state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextBuffer {
    rope: Rope,
    selections: SelectionSet,
    version: u64,
    current_state: u64,
    saved_state: u64,
    next_state: u64,
    undo: Vec<HistoryEntry>,
    redo: Vec<HistoryEntry>,
    auto_pairs: Vec<AutoPairMarker>,
    large_file_threshold: usize,
    preferred_display_columns: Option<Vec<usize>>,
}

impl Default for TextBuffer {
    fn default() -> Self {
        Self::new("")
    }
}

impl TextBuffer {
    #[must_use]
    pub fn new(text: &str) -> Self {
        Self::with_large_file_threshold(text, DEFAULT_LARGE_FILE_THRESHOLD)
    }

    #[must_use]
    pub fn with_large_file_threshold(text: &str, threshold: usize) -> Self {
        Self {
            rope: Rope::from_str(text),
            selections: SelectionSet::default(),
            version: 0,
            current_state: 0,
            saved_state: 0,
            next_state: 1,
            undo: Vec::new(),
            redo: Vec::new(),
            auto_pairs: Vec::new(),
            large_file_threshold: threshold,
            preferred_display_columns: None,
        }
    }

    #[must_use]
    pub fn snapshot(&self) -> TextSnapshot {
        TextSnapshot {
            text: Arc::from(self.rope.to_string()),
            version: self.version,
        }
    }

    #[must_use]
    pub fn len_chars(&self) -> usize {
        self.rope.len_chars()
    }

    #[must_use]
    pub fn len_bytes(&self) -> usize {
        self.rope.len_bytes()
    }

    #[must_use]
    pub fn line_count(&self) -> usize {
        self.rope.len_lines()
    }

    /// Returns the next grapheme boundary at or after `offset`.
    ///
    /// Keeping this operation on the buffer ensures editor commands never split a
    /// user-perceived character such as an emoji sequence or combining mark.
    #[must_use]
    pub fn next_grapheme_offset(&self, offset: CharacterOffset) -> CharacterOffset {
        let text = self.rope.to_string();
        next_grapheme_offset(&text, offset)
    }

    #[must_use]
    pub const fn version(&self) -> u64 {
        self.version
    }

    #[must_use]
    pub fn is_dirty(&self) -> bool {
        self.current_state != self.saved_state
    }

    pub fn mark_saved(&mut self) {
        self.saved_state = self.current_state;
    }

    /// Marks restored recovery contents as dirty without inventing an edit transaction.
    ///
    /// Recovery records contain the user's already-edited text, so replaying a synthetic edit
    /// would pollute undo history. This marker keeps dirty-quit protection active until the user
    /// explicitly saves the recovered buffer.
    pub fn mark_recovered_dirty(&mut self) {
        self.saved_state = if self.current_state == 0 {
            1
        } else {
            self.current_state - 1
        };
    }

    #[must_use]
    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    #[must_use]
    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    #[must_use]
    pub fn selections(&self) -> &SelectionSet {
        &self.selections
    }

    pub fn set_selections(&mut self, selections: SelectionSet) -> Result<()> {
        validate_selections_in_text(&self.rope.to_string(), &selections)?;
        self.selections = selections;
        self.preferred_display_columns = None;
        Ok(())
    }

    pub(crate) fn auto_pairs(&self) -> &[AutoPairMarker] {
        &self.auto_pairs
    }

    #[must_use]
    pub const fn large_file_threshold(&self) -> usize {
        self.large_file_threshold
    }

    pub fn set_large_file_threshold(&mut self, threshold: usize) {
        self.large_file_threshold = threshold;
    }

    #[must_use]
    pub fn is_large_file(&self) -> bool {
        self.len_bytes() >= self.large_file_threshold
    }

    #[must_use]
    pub fn semantic_service_policy(&self) -> SemanticServicePolicy {
        if self.is_large_file() {
            SemanticServicePolicy::SuppressedLargeFile
        } else {
            SemanticServicePolicy::Enabled
        }
    }

    pub fn offset_to_position(&self, offset: CharacterOffset) -> Result<LogicalPosition> {
        offset_to_position_in_text(&self.rope.to_string(), offset)
    }

    pub fn position_to_offset(&self, position: LogicalPosition) -> Result<CharacterOffset> {
        position_to_offset_in_text(&self.rope.to_string(), position)
    }

    pub fn display_column(&self, offset: CharacterOffset, tab_width: usize) -> Result<usize> {
        display_column_in_text(&self.rope.to_string(), offset, tab_width)
    }

    pub fn offset_for_display_column(
        &self,
        line: u32,
        column: usize,
        tab_width: usize,
    ) -> Result<CharacterOffset> {
        offset_for_display_column_in_text(&self.rope.to_string(), line, column, tab_width)
    }

    pub fn insert(&mut self, at: CharacterOffset, text: &str) -> Result<AppliedTransaction> {
        self.apply_transaction(Transaction::new(vec![Edit::insert(at, text)])?)
    }

    pub fn delete(&mut self, range: TextRange) -> Result<AppliedTransaction> {
        self.apply_transaction(Transaction::new(vec![Edit::delete(range)])?)
    }

    pub fn replace(&mut self, range: TextRange, text: &str) -> Result<AppliedTransaction> {
        self.apply_transaction(Transaction::new(vec![Edit::replace(range, text)])?)
    }

    pub fn apply_transaction(&mut self, transaction: Transaction) -> Result<AppliedTransaction> {
        self.apply_transaction_internal(transaction, None)
    }

    pub(crate) fn apply_transaction_with_markers(
        &mut self,
        transaction: Transaction,
        additional_markers: Vec<AutoPairMarker>,
    ) -> Result<AppliedTransaction> {
        self.apply_transaction_internal(transaction, Some(additional_markers))
    }

    fn apply_transaction_internal(
        &mut self,
        mut transaction: Transaction,
        additional_markers: Option<Vec<AutoPairMarker>>,
    ) -> Result<AppliedTransaction> {
        self.preferred_display_columns = None;
        let original = self.rope.to_string();
        validate_transaction_in_text(&original, &transaction)?;
        transaction.edits.retain(|edit| {
            let start = char_to_byte(&original, edit.range.start.0);
            let end = char_to_byte(&original, edit.range.end.0);
            original[start..end] != edit.replacement
        });

        if transaction.edits.is_empty() {
            if let Some(selections) = transaction.selection_after {
                validate_selections_in_text(&original, &selections)?;
                self.selections = selections;
            }
            return Ok(AppliedTransaction {
                changed: false,
                version: self.version,
            });
        }

        let final_text = apply_edits_to_string(&original, &transaction.edits);
        let selections_before = self.selections.clone();
        let selections_after = if let Some(selections) = &transaction.selection_after {
            validate_selections_in_text(&final_text, selections)?;
            selections.clone()
        } else {
            transform_selections(&self.selections, &transaction.edits)?
        };
        validate_selections_in_text(&final_text, &selections_after)?;

        let inverse = inverse_transaction(&original, &transaction.edits)?;
        let markers_before = self.auto_pairs.clone();
        let mut markers_after = transform_markers(&markers_before, &transaction.edits);
        if let Some(mut markers) = additional_markers {
            markers_after.append(&mut markers);
            markers_after.sort_by_key(|marker| marker.open_start);
        }

        apply_edits_to_rope(&mut self.rope, &transaction.edits);
        self.selections = selections_after.clone();
        self.auto_pairs.clone_from(&markers_after);
        let state_before = self.current_state;
        let state_after = self.next_state;
        self.next_state = self.next_state.saturating_add(1);
        self.current_state = state_after;
        self.version = self.version.saturating_add(1);
        self.undo.push(HistoryEntry {
            forward: transaction,
            inverse,
            selections_before,
            selections_after,
            markers_before,
            markers_after,
            state_before,
            state_after,
        });
        self.redo.clear();
        Ok(AppliedTransaction {
            changed: true,
            version: self.version,
        })
    }

    pub fn undo(&mut self) -> Result<bool> {
        let Some(entry) = self.undo.pop() else {
            return Ok(false);
        };
        let current = self.rope.to_string();
        validate_transaction_in_text(&current, &entry.inverse)?;
        apply_edits_to_rope(&mut self.rope, &entry.inverse.edits);
        self.selections = entry.selections_before.clone();
        self.auto_pairs.clone_from(&entry.markers_before);
        self.current_state = entry.state_before;
        self.version = self.version.saturating_add(1);
        self.redo.push(entry);
        Ok(true)
    }

    pub fn redo(&mut self) -> Result<bool> {
        let Some(entry) = self.redo.pop() else {
            return Ok(false);
        };
        let current = self.rope.to_string();
        validate_transaction_in_text(&current, &entry.forward)?;
        apply_edits_to_rope(&mut self.rope, &entry.forward.edits);
        self.selections = entry.selections_after.clone();
        self.auto_pairs.clone_from(&entry.markers_after);
        self.current_state = entry.state_after;
        self.version = self.version.saturating_add(1);
        self.undo.push(entry);
        Ok(true)
    }

    pub fn move_left(&mut self, extend: bool) -> Result<()> {
        self.preferred_display_columns = None;
        let text = self.rope.to_string();
        self.selections = self.selections.mapped(|selection| {
            let active = if !extend && !selection.is_cursor() {
                selection.range().start
            } else {
                previous_grapheme_offset(&text, selection.active)
            };
            selection.with_active(active, extend)
        })?;
        Ok(())
    }

    /// Moves left to the beginning of the previous word boundary.
    pub fn move_word_left(&mut self, extend: bool) -> Result<()> {
        self.preferred_display_columns = None;
        let text = self.rope.to_string();
        self.selections = self.selections.mapped(|selection| {
            let active = previous_word_offset(&text, selection.active);
            selection.with_active(active, extend)
        })?;
        Ok(())
    }

    pub fn move_right(&mut self, extend: bool) -> Result<()> {
        self.preferred_display_columns = None;
        let text = self.rope.to_string();
        self.selections = self.selections.mapped(|selection| {
            let active = if !extend && !selection.is_cursor() {
                selection.range().end
            } else {
                next_grapheme_offset(&text, selection.active)
            };
            selection.with_active(active, extend)
        })?;
        Ok(())
    }

    /// Moves right to the beginning of the next word boundary.
    pub fn move_word_right(&mut self, extend: bool) -> Result<()> {
        self.preferred_display_columns = None;
        let text = self.rope.to_string();
        self.selections = self.selections.mapped(|selection| {
            let active = next_word_offset(&text, selection.active);
            selection.with_active(active, extend)
        })?;
        Ok(())
    }

    /// Returns the word-like range containing a character offset using the core fallback policy.
    pub fn word_range_at(&self, offset: CharacterOffset) -> Result<TextRange> {
        let chars: Vec<char> = self.rope.to_string().chars().collect();
        if offset.0 > chars.len() {
            return Err(EditorError::OffsetOutOfBounds {
                offset: offset.0,
                length: chars.len(),
            });
        }
        let index = offset.0.min(chars.len());
        if chars.is_empty() {
            return Ok(TextRange {
                start: CharacterOffset(0),
                end: CharacterOffset(0),
            });
        }
        let probe = if index == chars.len() {
            index.saturating_sub(1)
        } else {
            index
        };
        let class = is_word_character(chars[probe]);
        let mut start = probe;
        while start > 0 && is_word_character(chars[start - 1]) == class {
            start -= 1;
        }
        let mut end = probe.saturating_add(1);
        while end < chars.len() && is_word_character(chars[end]) == class {
            end += 1;
        }
        Ok(TextRange {
            start: CharacterOffset(start),
            end: CharacterOffset(end),
        })
    }

    /// Moves every caret to the beginning of its logical line.
    pub fn move_home(&mut self, extend: bool) -> Result<()> {
        self.preferred_display_columns = None;
        let text = self.rope.to_string();
        let mut moved = Vec::with_capacity(self.selections.len());
        for selection in self.selections.selections() {
            let position = offset_to_position_in_text(&text, selection.active)?;
            let (start, content_end, _) =
                line_bounds(&text, position.line).ok_or(crate::EditorError::InvalidPosition {
                    line: position.line,
                    character: position.character,
                })?;
            let line_text = &text[char_to_byte(&text, start)..char_to_byte(&text, content_end)];
            let first_non_whitespace = line_text
                .char_indices()
                .find(|(_, character)| !character.is_whitespace())
                .map_or(content_end, |(offset, _)| {
                    start + line_text[..offset].chars().count()
                });
            let active = if selection.active.0 == first_non_whitespace {
                CharacterOffset(start)
            } else {
                CharacterOffset(first_non_whitespace)
            };
            moved.push(selection.with_active(active, extend));
        }
        self.selections = SelectionSet::new(moved, self.selections.primary_index())?;
        Ok(())
    }

    /// Moves every caret to the end of its logical line.
    pub fn move_end(&mut self, extend: bool) -> Result<()> {
        self.preferred_display_columns = None;
        let text = self.rope.to_string();
        let mut moved = Vec::with_capacity(self.selections.len());
        for selection in self.selections.selections() {
            let position = offset_to_position_in_text(&text, selection.active)?;
            let (_, end, _) =
                line_bounds(&text, position.line).ok_or(crate::EditorError::InvalidPosition {
                    line: position.line,
                    character: position.character,
                })?;
            moved.push(selection.with_active(CharacterOffset(end), extend));
        }
        self.selections = SelectionSet::new(moved, self.selections.primary_index())?;
        Ok(())
    }

    /// Moves every active endpoint to the beginning of the document.
    pub fn move_document_start(&mut self, extend: bool) -> Result<()> {
        self.preferred_display_columns = None;
        self.selections = self
            .selections
            .mapped(|selection| selection.with_active(CharacterOffset(0), extend))?;
        Ok(())
    }

    /// Moves every active endpoint to the end of the document.
    pub fn move_document_end(&mut self, extend: bool) -> Result<()> {
        self.preferred_display_columns = None;
        let end = CharacterOffset(self.rope.len_chars());
        self.selections = self
            .selections
            .mapped(|selection| selection.with_active(end, extend))?;
        Ok(())
    }

    /// Moves by a caller-supplied visible page while retaining preferred display columns.
    pub fn move_page(
        &mut self,
        page_lines: u32,
        direction: i32,
        extend: bool,
        tab_width: usize,
    ) -> Result<()> {
        let lines = i32::try_from(page_lines).unwrap_or(i32::MAX);
        self.move_vertical(lines.saturating_mul(direction), extend, tab_width)
    }

    /// Deletes the complete word to the left of each selection as one transaction.
    pub fn delete_word_left(&mut self) -> Result<AppliedTransaction> {
        self.delete_word(false)
    }

    /// Deletes the complete word to the right of each selection as one transaction.
    pub fn delete_word_right(&mut self) -> Result<AppliedTransaction> {
        self.delete_word(true)
    }

    fn delete_word(&mut self, right: bool) -> Result<AppliedTransaction> {
        let text = self.rope.to_string();
        let selections = self.selections.clone();
        let mut edits = Vec::new();
        let mut resulting = Vec::with_capacity(selections.len());
        for selection in selections.selections() {
            let range = if selection.is_cursor() {
                if right {
                    TextRange {
                        start: selection.active,
                        end: next_word_offset(&text, selection.active),
                    }
                } else {
                    TextRange {
                        start: previous_word_offset(&text, selection.active),
                        end: selection.active,
                    }
                }
            } else {
                selection.range()
            };
            resulting.push(Selection::cursor(range.start));
            if range.start != range.end {
                edits.push(Edit::delete(range));
            }
        }
        let resulting = SelectionSet::new(resulting, selections.primary_index())?;
        let transaction = TransactionBuilder::new()
            .extend(edits)
            .selection_after(resulting)
            .build()?;
        self.apply_transaction(transaction)
    }

    /// Selects the whole document, retaining a single primary selection.
    pub fn select_all(&mut self) -> Result<()> {
        let end = CharacterOffset(self.rope.len_chars());
        self.set_selections(SelectionSet::single(Selection::new(
            CharacterOffset(0),
            end,
        )))
    }

    /// Selects the logical line containing the primary active endpoint.
    pub fn select_line(&mut self) -> Result<()> {
        let snapshot = self.snapshot();
        let line = snapshot
            .offset_to_position(self.selections.primary().active)?
            .line;
        let (start, _, newline_end) = line_bounds(&snapshot.text, line)
            .ok_or(EditorError::InvalidPosition { line, character: 0 })?;
        self.set_selections(SelectionSet::single(Selection::new(
            CharacterOffset(start),
            CharacterOffset(newline_end),
        )))
    }

    pub fn move_vertical(&mut self, line_delta: i32, extend: bool, tab_width: usize) -> Result<()> {
        let text = self.rope.to_string();
        let preferred_columns = if let Some(columns) = self
            .preferred_display_columns
            .clone()
            .filter(|columns| columns.len() == self.selections.len())
        {
            columns
        } else {
            let mut columns = Vec::with_capacity(self.selections.len());
            for selection in self.selections.selections() {
                columns.push(display_column_in_text(&text, selection.active, tab_width)?);
            }
            columns
        };
        let mut moved = Vec::with_capacity(self.selections.len());
        for (index, selection) in self.selections.selections().iter().enumerate() {
            let position = offset_to_position_in_text(&text, selection.active)?;
            let last_line = line_bounds(&text, u32::MAX).map_or_else(
                || text.chars().filter(|character| *character == '\n').count(),
                |_| 0,
            );
            let target = i64::from(position.line)
                .saturating_add(i64::from(line_delta))
                .clamp(0, i64::try_from(last_line).unwrap_or(i64::MAX));
            let line = u32::try_from(target).unwrap_or(u32::MAX);
            let preferred = preferred_columns.get(index).copied().unwrap_or(0);
            let active = offset_for_display_column_in_text(&text, line, preferred, tab_width)?;
            moved.push(selection.with_active(active, extend));
        }
        self.selections = SelectionSet::new(moved, self.selections.primary_index())?;
        self.preferred_display_columns = Some(preferred_columns);
        Ok(())
    }

    /// Adds a cursor at a character offset without changing the primary selection.
    pub fn add_cursor_at(&mut self, offset: CharacterOffset) -> Result<()> {
        let selection = Selection::cursor(offset);
        self.set_selections(self.selections.with_cursor(selection)?)
    }

    /// Adds a vertically aligned cursor above the primary cursor.
    pub fn add_cursor_above(&mut self, tab_width: usize) -> Result<()> {
        self.add_cursor_vertical(-1, tab_width)
    }

    /// Adds a vertically aligned cursor below the primary cursor.
    pub fn add_cursor_below(&mut self, tab_width: usize) -> Result<()> {
        self.add_cursor_vertical(1, tab_width)
    }

    fn add_cursor_vertical(&mut self, delta: i32, tab_width: usize) -> Result<()> {
        let snapshot = self.snapshot();
        let primary = self.selections.primary();
        let position = snapshot.offset_to_position(primary.active)?;
        let column = snapshot.display_column(primary.active, tab_width)?;
        let target_line = i64::from(position.line).saturating_add(i64::from(delta));
        if target_line < 0
            || target_line >= i64::try_from(snapshot.line_count()).unwrap_or(i64::MAX)
        {
            return Ok(());
        }
        let offset = snapshot.offset_for_display_column(
            u32::try_from(target_line).unwrap_or(u32::MAX),
            column,
            tab_width,
        )?;
        self.add_cursor_at(offset)
    }

    /// Removes the last cursor while preserving the primary cursor when possible.
    pub fn remove_last_cursor(&mut self) -> Result<()> {
        self.set_selections(self.selections.without_last_cursor()?)
    }

    /// Collapses every selection to its active endpoint.
    pub fn collapse_selections(&mut self) -> Result<()> {
        self.set_selections(self.selections.collapse()?)
    }

    /// Selects the next occurrence of `query`, wrapping at the end of the document.
    pub fn select_next_occurrence(
        &mut self,
        query: &str,
        options: crate::FindOptions,
        skip: bool,
    ) -> Result<bool> {
        let matches = self.find(query, options)?;
        if matches.is_empty() {
            return Ok(false);
        }
        let primary = self.selections.primary().range();
        let Some(next) = matches
            .iter()
            .find(|matched| matched.range.start > primary.end)
            .or_else(|| matches.first())
            .copied()
        else {
            return Ok(false);
        };
        if skip {
            return Ok(true);
        }
        self.set_selections(
            self.selections
                .with_cursor(Selection::new(next.range.start, next.range.end))?,
        )?;
        Ok(true)
    }

    /// Selects every occurrence of `query` as one normalized multi-selection set.
    pub fn select_all_occurrences(
        &mut self,
        query: &str,
        options: crate::FindOptions,
    ) -> Result<usize> {
        let matches = self.find(query, options)?;
        if matches.is_empty() {
            return Ok(0);
        }
        let primary = self.selections.primary();
        let mut selections = matches
            .iter()
            .map(|matched| Selection::new(matched.range.start, matched.range.end))
            .collect::<Vec<_>>();
        let primary_index = selections
            .iter()
            .position(|selection| *selection == primary)
            .unwrap_or(0);
        self.set_selections(SelectionSet::new(
            std::mem::take(&mut selections),
            primary_index,
        )?)?;
        Ok(matches.len())
    }
}

impl fmt::Display for TextBuffer {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.rope.to_string())
    }
}

fn validate_transaction_in_text(text: &str, transaction: &Transaction) -> Result<()> {
    let mut previous: Option<&Edit> = None;
    for edit in &transaction.edits {
        validate_range_in_text(text, edit.range)?;
        if let Some(left) = previous {
            let overlap = edit.range.start < left.range.end;
            let collision = edit.range.start == left.range.start
                && (edit.range.start == edit.range.end || left.range.start == left.range.end);
            if overlap || collision {
                return Err(EditorError::OverlappingEdits {
                    offset: edit.range.start.0,
                });
            }
        }
        previous = Some(edit);
    }
    Ok(())
}

fn validate_range_in_text(text: &str, range: TextRange) -> Result<()> {
    let length = text.chars().count();
    if range.start > range.end || range.end.0 > length {
        return Err(EditorError::InvalidRange {
            start: range.start.0,
            end: range.end.0,
            length,
        });
    }
    validate_caret_boundary(text, range.start)?;
    validate_caret_boundary(text, range.end)
}

fn validate_selections_in_text(text: &str, selections: &SelectionSet) -> Result<()> {
    for selection in selections.selections() {
        validate_range_in_text(text, selection.range())?;
    }
    Ok(())
}

fn validate_caret_boundary(text: &str, offset: CharacterOffset) -> Result<()> {
    let length = text.chars().count();
    if offset.0 > length {
        return Err(EditorError::OffsetOutOfBounds {
            offset: offset.0,
            length,
        });
    }
    if offset.0 > 0 && offset.0 < length {
        let before = text.chars().nth(offset.0 - 1);
        let after = text.chars().nth(offset.0);
        if before == Some('\r') && after == Some('\n') {
            return Err(EditorError::InvalidCaretBoundary { offset: offset.0 });
        }
    }
    Ok(())
}

fn inverse_transaction(original: &str, edits: &[Edit]) -> Result<Transaction> {
    let mut delta: isize = 0;
    let mut inverse = Vec::with_capacity(edits.len());
    for edit in edits {
        let start = char_to_byte(original, edit.range.start.0);
        let end = char_to_byte(original, edit.range.end.0);
        let removed = original[start..end].to_owned();
        let final_start = edit.range.start.0.saturating_add_signed(delta);
        let inserted_len = edit.replacement.chars().count();
        inverse.push(Edit::replace(
            TextRange {
                start: CharacterOffset(final_start),
                end: CharacterOffset(final_start + inserted_len),
            },
            removed,
        ));
        let removed_len = edit.range.end.0 - edit.range.start.0;
        delta = delta.saturating_add(
            isize::try_from(inserted_len).unwrap_or(isize::MAX)
                - isize::try_from(removed_len).unwrap_or(isize::MAX),
        );
    }
    TransactionBuilder::new().extend(inverse).build()
}

fn apply_edits_to_rope(rope: &mut Rope, edits: &[Edit]) {
    for edit in edits.iter().rev() {
        rope.remove(edit.range.start.0..edit.range.end.0);
        if !edit.replacement.is_empty() {
            rope.insert(edit.range.start.0, &edit.replacement);
        }
    }
}

fn apply_edits_to_string(text: &str, edits: &[Edit]) -> String {
    let mut result = text.to_owned();
    for edit in edits.iter().rev() {
        let start = char_to_byte(&result, edit.range.start.0);
        let end = char_to_byte(&result, edit.range.end.0);
        result.replace_range(start..end, &edit.replacement);
    }
    result
}

fn transform_selections(selections: &SelectionSet, edits: &[Edit]) -> Result<SelectionSet> {
    selections.mapped(|selection| Selection {
        anchor: transform_offset(selection.anchor, edits),
        active: transform_offset(selection.active, edits),
    })
}

fn transform_offset(offset: CharacterOffset, edits: &[Edit]) -> CharacterOffset {
    let mut delta: isize = 0;
    for edit in edits {
        let inserted = edit.replacement.chars().count();
        let removed = edit.range.end.0 - edit.range.start.0;
        if offset < edit.range.start {
            break;
        }
        if offset <= edit.range.end {
            return CharacterOffset(edit.range.start.0.saturating_add_signed(delta) + inserted);
        }
        delta = delta.saturating_add(
            isize::try_from(inserted).unwrap_or(isize::MAX)
                - isize::try_from(removed).unwrap_or(isize::MAX),
        );
    }
    CharacterOffset(offset.0.saturating_add_signed(delta))
}

fn transform_markers(markers: &[AutoPairMarker], edits: &[Edit]) -> Vec<AutoPairMarker> {
    markers
        .iter()
        .filter_map(|marker| {
            let mut delta: isize = 0;
            for edit in edits {
                let inserted = edit.replacement.chars().count();
                let removed = edit.range.end.0 - edit.range.start.0;
                if edit.range.end.0 <= marker.open_start {
                    delta = delta.saturating_add(
                        isize::try_from(inserted).unwrap_or(isize::MAX)
                            - isize::try_from(removed).unwrap_or(isize::MAX),
                    );
                } else if edit.range.start.0 < marker.end()
                    || (edit.range.start == edit.range.end
                        && edit.range.start.0 > marker.open_start
                        && edit.range.start.0 < marker.end())
                {
                    return None;
                }
            }
            let mut shifted = marker.clone();
            shifted.open_start = shifted.open_start.saturating_add_signed(delta);
            shifted.close_start = shifted.close_start.saturating_add_signed(delta);
            Some(shifted)
        })
        .collect()
}

pub(crate) fn previous_grapheme_offset(text: &str, offset: CharacterOffset) -> CharacterOffset {
    if offset.0 == 0 {
        return offset;
    }
    let byte = char_to_byte(text, offset.0);
    let previous_byte = text[..byte]
        .grapheme_indices(true)
        .next_back()
        .map_or(0, |(index, _)| index);
    CharacterOffset(text[..previous_byte].chars().count())
}

pub(crate) fn next_grapheme_offset(text: &str, offset: CharacterOffset) -> CharacterOffset {
    let byte = char_to_byte(text, offset.0);
    let next = text[byte..]
        .graphemes(true)
        .next()
        .map_or(offset.0, |grapheme| offset.0 + grapheme.chars().count());
    CharacterOffset(next)
}

pub(crate) fn char_to_byte(text: &str, char_offset: usize) -> usize {
    text.char_indices()
        .nth(char_offset)
        .map_or(text.len(), |(byte, _)| byte)
}

fn is_word_character(character: char) -> bool {
    character.is_alphanumeric() || character == '_'
}

fn previous_word_offset(text: &str, offset: CharacterOffset) -> CharacterOffset {
    let chars: Vec<char> = text.chars().collect();
    let mut index = offset.0.min(chars.len());
    while index > 0 && chars[index - 1].is_whitespace() {
        index -= 1;
    }
    let class = index.checked_sub(1).map(|at| is_word_character(chars[at]));
    while index > 0 && index.checked_sub(1).map(|at| is_word_character(chars[at])) == class {
        index -= 1;
    }
    CharacterOffset(index)
}

fn next_word_offset(text: &str, offset: CharacterOffset) -> CharacterOffset {
    let chars: Vec<char> = text.chars().collect();
    let mut index = offset.0.min(chars.len());
    if index < chars.len() {
        let class = is_word_character(chars[index]);
        while index < chars.len() && is_word_character(chars[index]) == class {
            index += 1;
        }
        while index < chars.len() && chars[index].is_whitespace() {
            index += 1;
        }
    }
    CharacterOffset(index)
}

fn line_bounds(text: &str, requested_line: u32) -> Option<(usize, usize, usize)> {
    let requested = usize::try_from(requested_line).ok()?;
    let mut line = 0;
    let mut start = 0;
    let chars: Vec<char> = text.chars().collect();
    for (index, character) in chars.iter().enumerate() {
        if *character == '\n' {
            if line == requested {
                let content_end = if index > start && chars[index - 1] == '\r' {
                    index - 1
                } else {
                    index
                };
                return Some((start, content_end, index + 1));
            }
            line += 1;
            start = index + 1;
        }
    }
    (line == requested).then_some((start, chars.len(), chars.len()))
}

fn offset_to_position_in_text(text: &str, offset: CharacterOffset) -> Result<LogicalPosition> {
    validate_caret_boundary(text, offset)?;
    let mut line = 0_u32;
    let mut character = 0_u32;
    for current in text.chars().take(offset.0) {
        if current == '\n' {
            line = line.saturating_add(1);
            character = 0;
        } else if current != '\r' || text.chars().nth(offset.0) != Some('\n') {
            character = character.saturating_add(1);
        }
    }
    Ok(LogicalPosition { line, character })
}

fn position_to_offset_in_text(text: &str, position: LogicalPosition) -> Result<CharacterOffset> {
    let (start, content_end, _) =
        line_bounds(text, position.line).ok_or(EditorError::InvalidPosition {
            line: position.line,
            character: position.character,
        })?;
    let character =
        usize::try_from(position.character).map_err(|_| EditorError::InvalidPosition {
            line: position.line,
            character: position.character,
        })?;
    if start + character > content_end {
        return Err(EditorError::InvalidPosition {
            line: position.line,
            character: position.character,
        });
    }
    Ok(CharacterOffset(start + character))
}

fn display_column_in_text(text: &str, offset: CharacterOffset, tab_width: usize) -> Result<usize> {
    let position = offset_to_position_in_text(text, offset)?;
    let (start, _, _) = line_bounds(text, position.line).ok_or(EditorError::InvalidPosition {
        line: position.line,
        character: position.character,
    })?;
    let prefix = &text[char_to_byte(text, start)..char_to_byte(text, offset.0)];
    let mut column = 0;
    for grapheme in prefix.graphemes(true) {
        if grapheme == "\t" {
            let width = tab_width.max(1);
            column += width - (column % width);
        } else {
            column += UnicodeWidthStr::width(grapheme);
        }
    }
    Ok(column)
}

fn offset_for_display_column_in_text(
    text: &str,
    line: u32,
    target: usize,
    tab_width: usize,
) -> Result<CharacterOffset> {
    let (start, content_end, _) =
        line_bounds(text, line).ok_or(EditorError::InvalidPosition { line, character: 0 })?;
    let line_text = &text[char_to_byte(text, start)..char_to_byte(text, content_end)];
    let mut column = 0;
    let mut chars = 0;
    for grapheme in line_text.graphemes(true) {
        let width = if grapheme == "\t" {
            let tab = tab_width.max(1);
            tab - (column % tab)
        } else {
            UnicodeWidthStr::width(grapheme)
        };
        if column + width > target {
            break;
        }
        column += width;
        chars += grapheme.chars().count();
    }
    Ok(CharacterOffset(start + chars))
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;
    use rstest::rstest;

    use super::*;

    #[rstest]
    #[case("a\nb", LogicalPosition { line: 1, character: 1 }, CharacterOffset(3))]
    #[case("a\r\nb", LogicalPosition { line: 1, character: 1 }, CharacterOffset(4))]
    #[case("界🙂\n", LogicalPosition { line: 0, character: 2 }, CharacterOffset(2))]
    fn logical_positions_round_trip(
        #[case] text: &str,
        #[case] position: LogicalPosition,
        #[case] offset: CharacterOffset,
    ) {
        let buffer = TextBuffer::new(text);
        assert_eq!(buffer.position_to_offset(position), Ok(offset));
        assert_eq!(buffer.offset_to_position(offset), Ok(position));
    }

    #[test]
    fn crlf_split_is_rejected() {
        let mut buffer = TextBuffer::new("a\r\nb");
        let error =
            buffer.set_selections(SelectionSet::single(Selection::cursor(CharacterOffset(2))));
        assert_eq!(error, Err(EditorError::InvalidCaretBoundary { offset: 2 }));
    }

    #[test]
    fn wide_and_combining_graphemes_move_as_units() {
        let mut buffer = TextBuffer::new("e\u{301}界🙂");
        buffer
            .set_selections(SelectionSet::single(Selection::cursor(CharacterOffset(4))))
            .expect("valid cursor");
        buffer.move_left(false).expect("movement succeeds");
        assert_eq!(buffer.selections().primary().active, CharacterOffset(3));
        buffer.move_left(false).expect("movement succeeds");
        assert_eq!(buffer.selections().primary().active, CharacterOffset(2));
        buffer.move_left(false).expect("movement succeeds");
        assert_eq!(buffer.selections().primary().active, CharacterOffset(0));
        assert_eq!(buffer.display_column(CharacterOffset(3), 4), Ok(3));
    }

    #[test]
    fn crlf_fixture_round_trips_positions_and_offsets() {
        let text = "a\r\n界🙂\r\n";
        let buffer = TextBuffer::new(text);
        assert_eq!(buffer.line_count(), 3);
        assert_eq!(
            buffer.offset_to_position(CharacterOffset(0)),
            Ok(LogicalPosition {
                line: 0,
                character: 0
            })
        );
        assert_eq!(
            buffer.position_to_offset(LogicalPosition {
                line: 1,
                character: 1
            }),
            Ok(CharacterOffset(4))
        );
        assert_eq!(
            buffer.offset_to_position(CharacterOffset(5)),
            Ok(LogicalPosition {
                line: 1,
                character: 2
            })
        );
    }

    #[test]
    fn simultaneous_edits_use_original_coordinates() {
        let mut buffer = TextBuffer::new("abcdef");
        let transaction = Transaction::new(vec![
            Edit::replace(
                TextRange::new(CharacterOffset(1), CharacterOffset(3)).unwrap(),
                "X",
            ),
            Edit::replace(
                TextRange::new(CharacterOffset(4), CharacterOffset(6)).unwrap(),
                "Y",
            ),
        ])
        .expect("non-overlapping");
        buffer.apply_transaction(transaction).expect("valid edits");
        assert_eq!(buffer.to_string(), "aXdY");
        assert_eq!(buffer.version(), 1);
    }

    #[test]
    fn overlapping_edits_are_typed_errors() {
        let result = Transaction::new(vec![
            Edit::delete(TextRange::new(CharacterOffset(1), CharacterOffset(4)).unwrap()),
            Edit::delete(TextRange::new(CharacterOffset(3), CharacterOffset(5)).unwrap()),
        ]);
        assert_eq!(result, Err(EditorError::OverlappingEdits { offset: 3 }));
    }

    #[test]
    fn undo_redo_restore_text_selections_dirty_and_versions() {
        let mut buffer = TextBuffer::new("hello");
        buffer
            .set_selections(SelectionSet::single(Selection::cursor(CharacterOffset(5))))
            .expect("valid selection");
        let after = SelectionSet::single(Selection::cursor(CharacterOffset(6)));
        let transaction = Transaction::new(vec![Edit::insert(CharacterOffset(5), "!")])
            .expect("transaction")
            .with_selection_after(after.clone());
        buffer.apply_transaction(transaction).expect("apply");
        assert!(buffer.is_dirty());
        assert_eq!(buffer.selections(), &after);
        assert_eq!(buffer.version(), 1);
        assert!(buffer.undo().expect("undo"));
        assert_eq!(buffer.to_string(), "hello");
        assert_eq!(buffer.selections().primary().active, CharacterOffset(5));
        assert!(!buffer.is_dirty());
        assert_eq!(buffer.version(), 2);
        assert!(buffer.redo().expect("redo"));
        assert_eq!(buffer.to_string(), "hello!");
        assert_eq!(buffer.selections(), &after);
        assert_eq!(buffer.version(), 3);
    }

    #[test]
    fn multi_cursor_vertical_and_occurrence_selection_are_unicode_safe() {
        let mut buffer = TextBuffer::new("écho\nécho\nécho");
        buffer
            .set_selections(SelectionSet::single(Selection::cursor(CharacterOffset(1))))
            .expect("selection");
        buffer.add_cursor_below(4).expect("below cursor");
        assert_eq!(buffer.selections().len(), 2);
        let count = buffer
            .select_all_occurrences("écho", crate::FindOptions::default())
            .expect("occurrences");
        assert_eq!(count, 3);
        buffer.collapse_selections().expect("collapse");
        assert!(
            buffer
                .selections()
                .selections()
                .iter()
                .all(|selection| selection.is_cursor())
        );
    }

    #[test]
    fn vertical_navigation_retains_preferred_display_column_across_blank_lines() {
        let mut buffer = TextBuffer::new("abcde\n\nabcde");
        buffer
            .set_selections(SelectionSet::single(Selection::cursor(CharacterOffset(5))))
            .expect("selection");
        buffer.move_vertical(1, false, 4).expect("down to blank");
        assert_eq!(buffer.selections().primary().active, CharacterOffset(6));
        buffer
            .move_vertical(1, false, 4)
            .expect("down to long line");
        assert_eq!(buffer.selections().primary().active, CharacterOffset(12));
    }

    #[test]
    fn vertical_navigation_keeps_each_multi_cursor_column() {
        let mut buffer = TextBuffer::new("ab\ncdef\n\nuvwxyz");
        buffer
            .set_selections(
                SelectionSet::new(
                    vec![
                        Selection::cursor(CharacterOffset(1)),
                        Selection::cursor(CharacterOffset(4)),
                    ],
                    0,
                )
                .expect("selections"),
            )
            .expect("selection");
        buffer.move_vertical(1, false, 4).expect("down to blank");
        buffer
            .move_vertical(1, false, 4)
            .expect("down to short line");
        let offsets = buffer
            .selections()
            .selections()
            .iter()
            .map(|selection| selection.active.0)
            .collect::<Vec<_>>();
        assert_eq!(offsets, vec![8, 10]);
    }

    #[test]
    fn large_file_policy_tracks_edits() {
        let mut buffer = TextBuffer::with_large_file_threshold("1234", 5);
        assert_eq!(
            buffer.semantic_service_policy(),
            SemanticServicePolicy::Enabled
        );
        buffer.insert(CharacterOffset(4), "5").expect("insert");
        assert_eq!(
            buffer.semantic_service_policy(),
            SemanticServicePolicy::SuppressedLargeFile
        );
    }

    #[test]
    fn navigation_commands_cover_document_word_page_and_selection() {
        let mut buffer = TextBuffer::new("one two\nthree");
        buffer
            .set_selections(SelectionSet::single(Selection::cursor(CharacterOffset(7))))
            .expect("selection");
        buffer.move_document_start(false).expect("document start");
        assert_eq!(buffer.selections().primary().active, CharacterOffset(0));
        buffer.move_document_end(false).expect("document end");
        assert_eq!(buffer.selections().primary().active, CharacterOffset(13));
        buffer
            .set_selections(SelectionSet::single(Selection::cursor(CharacterOffset(4))))
            .expect("selection");
        buffer.move_word_left(false).expect("word left");
        assert_eq!(buffer.selections().primary().active, CharacterOffset(0));
        buffer
            .set_selections(SelectionSet::single(Selection::cursor(CharacterOffset(4))))
            .expect("selection");
        buffer.move_page(1, 1, false, 4).expect("page");
        assert_eq!(buffer.selections().primary().active, CharacterOffset(12));
        buffer.select_line().expect("line selection");
        assert_eq!(
            buffer
                .snapshot()
                .text_in_range(buffer.selections().primary().range())
                .expect("range"),
            "three"
        );
        buffer.select_all().expect("all selection");
        assert_eq!(
            buffer.selections().primary().range().end,
            CharacterOffset(13)
        );
    }

    #[test]
    fn smart_home_and_word_delete_are_safe() {
        let mut buffer = TextBuffer::new("  alpha beta");
        buffer
            .set_selections(SelectionSet::single(Selection::cursor(CharacterOffset(8))))
            .expect("selection");
        buffer.move_home(false).expect("smart home");
        assert_eq!(buffer.selections().primary().active, CharacterOffset(2));
        buffer.move_home(false).expect("absolute home");
        assert_eq!(buffer.selections().primary().active, CharacterOffset(0));
        buffer
            .set_selections(SelectionSet::single(Selection::cursor(CharacterOffset(8))))
            .expect("selection");
        buffer.delete_word_left().expect("word delete");
        assert_eq!(buffer.to_string(), "  beta");
    }

    proptest! {
        #[test]
        fn arbitrary_insert_delete_sequences_preserve_valid_text(
            initial in ".{0,80}",
            operations in prop::collection::vec((any::<bool>(), 0usize..100, ".{0,8}"), 0..40),
        ) {
            let mut buffer = TextBuffer::new(&initial);
            for (insert, raw, value) in operations {
                let length = buffer.len_chars();
                let at = raw % (length + 1);
                if insert {
                    buffer.insert(CharacterOffset(at), &value).expect("bounded insert");
                } else if length > 0 {
                    let end = (at + value.chars().count()).min(length);
                    if at <= end {
                        buffer.delete(TextRange::new(CharacterOffset(at), CharacterOffset(end)).unwrap())
                            .expect("bounded delete");
                    }
                }
                let text = buffer.to_string();
                prop_assert_eq!(text.chars().count(), buffer.len_chars());
            }
        }

        #[test]
        fn transaction_undo_redo_is_exact(
            initial in ".{0,80}",
            replacement in ".{0,30}",
            raw_start in 0usize..100,
            raw_end in 0usize..100,
        ) {
            let mut buffer = TextBuffer::new(&initial);
            let length = buffer.len_chars();
            let start = raw_start.min(raw_end) % (length + 1);
            let end = (raw_start.max(raw_end) % (length + 1)).max(start);
            let before_selection = SelectionSet::single(Selection::cursor(CharacterOffset(start)));
            buffer.set_selections(before_selection.clone()).expect("valid selection");
            let after_offset = start + replacement.chars().count();
            let after_selection = SelectionSet::single(Selection::cursor(CharacterOffset(after_offset)));
            let transaction = Transaction::new(vec![Edit::replace(
                TextRange::new(CharacterOffset(start), CharacterOffset(end)).unwrap(),
                replacement,
            )]).expect("transaction").with_selection_after(after_selection.clone());
            let changed = buffer.apply_transaction(transaction).expect("apply").changed;
            let edited = buffer.to_string();
            if changed {
                prop_assert!(buffer.undo().expect("undo"));
                prop_assert_eq!(buffer.to_string(), initial);
                prop_assert_eq!(buffer.selections(), &before_selection);
                prop_assert!(buffer.redo().expect("redo"));
                prop_assert_eq!(buffer.to_string(), edited);
                prop_assert_eq!(buffer.selections(), &after_selection);
            }
        }
    }
}
