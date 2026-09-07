use crate::buffer::{AutoPairMarker, char_to_byte, previous_grapheme_offset};
use crate::{
    AppliedTransaction, CharacterOffset, Edit, EditorError, Result, Selection, SelectionSet,
    TextBuffer, TextRange, TransactionBuilder,
};

/// Configurable indentation unit used by Enter and indentation commands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IndentStyle {
    Spaces(usize),
    Tabs,
}

impl IndentStyle {
    #[must_use]
    pub fn unit(self) -> String {
        match self {
            Self::Spaces(width) => " ".repeat(width.max(1)),
            Self::Tabs => "\t".to_owned(),
        }
    }
}

/// Language-configurable opening and closing delimiter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PairConfig {
    pub open: String,
    pub close: String,
    pub overtype: bool,
}

/// Regex-driven indentation hints imported from a language configuration.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct IndentationRules {
    pub increase_indent_pattern: Option<String>,
    pub decrease_indent_pattern: Option<String>,
    pub indent_next_line_pattern: Option<String>,
    pub unindented_line_pattern: Option<String>,
}

/// Regex predicates and insertion hints for language-configured Enter behavior.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct EnterRule {
    pub before_text: Option<String>,
    pub after_text: Option<String>,
    pub previous_line_text: Option<String>,
    pub append_text: Option<String>,
    pub remove_text: Option<String>,
    pub indent_action: Option<String>,
}

impl PairConfig {
    pub fn new(open: impl Into<String>, close: impl Into<String>) -> Result<Self> {
        let open = open.into();
        let close = close.into();
        if open.is_empty() || close.is_empty() {
            return Err(EditorError::InvalidPairConfiguration);
        }
        Ok(Self {
            open,
            close,
            overtype: true,
        })
    }

    #[must_use]
    pub fn with_overtype(mut self, overtype: bool) -> Self {
        self.overtype = overtype;
        self
    }

    #[must_use]
    pub fn common_defaults() -> Vec<Self> {
        [('{', '}'), ('[', ']'), ('(', ')'), ('\'', '\''), ('"', '"')]
            .into_iter()
            .map(|(open, close)| Self {
                open: open.to_string(),
                close: close.to_string(),
                overtype: true,
            })
            .collect()
    }
}

/// Observable outcome of a smart editing command.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SmartEditOutcome {
    pub text_changed: bool,
    pub selection_changed: bool,
    pub version: u64,
}

impl From<AppliedTransaction> for SmartEditOutcome {
    fn from(applied: AppliedTransaction) -> Self {
        Self {
            text_changed: applied.changed,
            selection_changed: true,
            version: applied.version,
        }
    }
}

impl TextBuffer {
    /// Inserts text at every selection, applying configured surround, pair and overtype rules.
    pub fn smart_insert(&mut self, input: &str, pairs: &[PairConfig]) -> Result<SmartEditOutcome> {
        if input.is_empty() {
            return Ok(SmartEditOutcome {
                text_changed: false,
                selection_changed: false,
                version: self.version(),
            });
        }
        let text = self.to_string();
        let selections = self.selections().clone();
        let mut edits = Vec::with_capacity(selections.len());
        let mut resulting = Vec::with_capacity(selections.len());
        let mut markers = Vec::new();
        let mut delta: isize = 0;

        for selection in selections.selections() {
            let range = selection.range();
            let final_start = range.start.0.saturating_add_signed(delta);
            if !selection.is_cursor() {
                if let Some(pair) = pairs.iter().find(|pair| pair.open == input) {
                    let start_byte = char_to_byte(&text, range.start.0);
                    let end_byte = char_to_byte(&text, range.end.0);
                    let selected = &text[start_byte..end_byte];
                    let replacement = format!("{}{}{}", pair.open, selected, pair.close);
                    edits.push(Edit::replace(range, replacement));
                    let inner_start = final_start + pair.open.chars().count();
                    let inner_end = inner_start + selected.chars().count();
                    resulting.push(if selection.anchor <= selection.active {
                        Selection::new(CharacterOffset(inner_start), CharacterOffset(inner_end))
                    } else {
                        Selection::new(CharacterOffset(inner_end), CharacterOffset(inner_start))
                    });
                    delta = delta.saturating_add(
                        isize::try_from(pair.open.chars().count() + pair.close.chars().count())
                            .unwrap_or(isize::MAX),
                    );
                    continue;
                }
            } else if let Some(pair) = pairs
                .iter()
                .find(|pair| pair.close == input && pair.overtype)
            {
                let at = range.start.0;
                let is_tracked = self
                    .auto_pairs()
                    .iter()
                    .any(|marker| marker.close_start == at && marker.close == pair.close);
                let byte = char_to_byte(&text, at);
                if is_tracked && text[byte..].starts_with(&pair.close) {
                    resulting.push(Selection::cursor(CharacterOffset(
                        final_start + pair.close.chars().count(),
                    )));
                    continue;
                }
            }

            if selection.is_cursor() {
                if let Some(pair) = pairs.iter().find(|pair| pair.open == input) {
                    let replacement = format!("{}{}", pair.open, pair.close);
                    edits.push(Edit::insert(range.start, replacement));
                    let open_len = pair.open.chars().count();
                    let close_start = final_start + open_len;
                    resulting.push(Selection::cursor(CharacterOffset(close_start)));
                    markers.push(AutoPairMarker {
                        open_start: final_start,
                        open: pair.open.clone(),
                        close_start,
                        close: pair.close.clone(),
                    });
                    delta = delta.saturating_add(
                        isize::try_from(open_len + pair.close.chars().count())
                            .unwrap_or(isize::MAX),
                    );
                    continue;
                }
            }

            edits.push(Edit::replace(range, input));
            let inserted = input.chars().count();
            resulting.push(Selection::cursor(CharacterOffset(final_start + inserted)));
            delta = delta.saturating_add(
                isize::try_from(inserted).unwrap_or(isize::MAX)
                    - isize::try_from(range.end.0 - range.start.0).unwrap_or(isize::MAX),
            );
        }

        let resulting = SelectionSet::new(resulting, selections.primary_index())?;
        if edits.is_empty() {
            let changed = &resulting != self.selections();
            self.set_selections(resulting)?;
            return Ok(SmartEditOutcome {
                text_changed: false,
                selection_changed: changed,
                version: self.version(),
            });
        }
        let transaction = TransactionBuilder::new()
            .extend(edits)
            .normalize_identical_edits()
            .selection_after(resulting)
            .build()?;
        self.apply_transaction_with_markers(transaction, markers)
            .map(Into::into)
    }

    /// Deletes selections or the preceding grapheme, including tracked empty auto-pairs.
    pub fn smart_backspace(&mut self) -> Result<SmartEditOutcome> {
        let text = self.to_string();
        let selections = self.selections().clone();
        let mut edits = Vec::with_capacity(selections.len());
        let mut resulting = Vec::with_capacity(selections.len());
        let mut delta: isize = 0;

        for selection in selections.selections() {
            let range = selection.range();
            let deletion = if !selection.is_cursor() {
                range
            } else if let Some(marker) = self.auto_pairs().iter().find(|marker| {
                marker.open_start + marker.open.chars().count() == range.start.0
                    && marker.close_start == range.start.0
            }) {
                TextRange {
                    start: CharacterOffset(marker.open_start),
                    end: CharacterOffset(marker.close_start + marker.close.chars().count()),
                }
            } else {
                TextRange {
                    start: previous_grapheme_offset(&text, range.start),
                    end: range.start,
                }
            };
            if deletion.start == deletion.end {
                resulting.push(*selection);
                continue;
            }
            let final_start = deletion.start.0.saturating_add_signed(delta);
            edits.push(Edit::delete(deletion));
            resulting.push(Selection::cursor(CharacterOffset(final_start)));
            delta = delta.saturating_sub(
                isize::try_from(deletion.end.0 - deletion.start.0).unwrap_or(isize::MAX),
            );
        }
        if edits.is_empty() {
            return Ok(SmartEditOutcome {
                text_changed: false,
                selection_changed: false,
                version: self.version(),
            });
        }
        let resulting = SelectionSet::new(resulting, selections.primary_index())?;
        let transaction = TransactionBuilder::new()
            .extend(edits)
            .normalize_identical_edits()
            .selection_after(resulting)
            .build()?;
        self.apply_transaction(transaction).map(Into::into)
    }

    /// Inserts a newline with current indentation, expanding when between a configured pair.
    pub fn smart_enter(
        &mut self,
        pairs: &[PairConfig],
        indent_style: IndentStyle,
    ) -> Result<SmartEditOutcome> {
        self.smart_enter_with_rules(pairs, indent_style, None)
    }

    /// Inserts a newline while applying optional language-provided indentation regexes.
    pub fn smart_enter_with_rules(
        &mut self,
        pairs: &[PairConfig],
        indent_style: IndentStyle,
        rules: Option<&IndentationRules>,
    ) -> Result<SmartEditOutcome> {
        self.smart_enter_with_rules_and_actions(pairs, indent_style, rules, &[])
    }

    /// Inserts a newline with indentation rules and optional language Enter actions.
    #[allow(clippy::too_many_lines)]
    pub fn smart_enter_with_rules_and_actions(
        &mut self,
        pairs: &[PairConfig],
        indent_style: IndentStyle,
        rules: Option<&IndentationRules>,
        on_enter_rules: &[EnterRule],
    ) -> Result<SmartEditOutcome> {
        let compiled_rules = rules.map(CompiledIndentationRules::compile).transpose()?;
        let compiled_enter_rules = on_enter_rules
            .iter()
            .map(CompiledEnterRule::compile)
            .collect::<Result<Vec<_>>>()?;
        let text = self.to_string();
        let newline = preferred_newline(&text);
        let selections = self.selections().clone();
        let mut edits = Vec::with_capacity(selections.len());
        let mut resulting = Vec::with_capacity(selections.len());
        let mut delta: isize = 0;

        for selection in selections.selections() {
            let range = selection.range();
            let start_byte = char_to_byte(&text, range.start.0);
            let line_start_byte = text[..start_byte].rfind('\n').map_or(0, |index| index + 1);
            let line_prefix = &text[line_start_byte..start_byte];
            let base_indent: String = line_prefix
                .chars()
                .take_while(|character| *character == ' ' || *character == '\t')
                .collect();
            let before = &text[..start_byte];
            let after = &text[char_to_byte(&text, range.end.0)..];
            let after_line = &after[..after.find('\n').unwrap_or(after.len())];
            let previous_line = before.strip_suffix(line_prefix).and_then(|prefix| {
                prefix
                    .rsplit_once('\n')
                    .map_or(Some(prefix), |(_, line)| Some(line))
            });
            let enter_action = compiled_enter_rules
                .iter()
                .find(|rule| rule.matches(line_prefix, after_line, previous_line));
            let mut edit_range = range;
            if let Some(rule) = enter_action {
                if let Some(remove_text) = &rule.remove_text {
                    let remove_len = remove_text.chars().count();
                    if before.ends_with(remove_text) && range.start.0 >= remove_len {
                        edit_range.start =
                            CharacterOffset(range.start.0.saturating_sub(remove_len));
                    }
                }
            }
            let pair_between = selection.is_cursor()
                && pairs
                    .iter()
                    .any(|pair| before.ends_with(&pair.open) && after.starts_with(&pair.close));
            let (replacement, cursor_delta) = if pair_between {
                let inner = format!("{base_indent}{}", indent_style.unit());
                let replacement = format!("{newline}{inner}{newline}{base_indent}");
                let cursor_delta = newline.chars().count() + inner.chars().count();
                (replacement, cursor_delta)
            } else {
                let extra_indent = pairs.iter().any(|pair| before.ends_with(&pair.open));
                let mut indent = base_indent.clone();
                let line_matches = |pattern: &Option<regex::Regex>| {
                    pattern
                        .as_ref()
                        .is_some_and(|pattern| pattern.is_match(line_prefix))
                };
                if compiled_rules
                    .as_ref()
                    .is_some_and(|rules| line_matches(&rules.unindented_line_pattern))
                {
                    indent.clear();
                } else {
                    let should_decrease = compiled_rules
                        .as_ref()
                        .is_some_and(|rules| line_matches(&rules.decrease_indent_pattern));
                    if should_decrease {
                        remove_indent_unit(&mut indent, indent_style);
                    }
                    let should_increase = extra_indent
                        || compiled_rules.as_ref().is_some_and(|rules| {
                            line_matches(&rules.increase_indent_pattern)
                                || line_matches(&rules.indent_next_line_pattern)
                        });
                    if should_increase {
                        indent.push_str(&indent_style.unit());
                    }
                }
                let mut replacement = format!("{newline}{indent}");
                if let Some(rule) = enter_action {
                    if let Some(append_text) = &rule.append_text {
                        replacement.push_str(append_text);
                    }
                    if rule.indent_action.as_deref() == Some("indent") {
                        replacement.push_str(&indent_style.unit());
                    }
                }
                let cursor_delta = replacement.chars().count();
                (replacement, cursor_delta)
            };
            let final_start = edit_range.start.0.saturating_add_signed(delta);
            let replacement_len = replacement.chars().count();
            edits.push(Edit::replace(edit_range, replacement));
            resulting.push(Selection::cursor(CharacterOffset(
                final_start + cursor_delta,
            )));
            delta = delta.saturating_add(
                isize::try_from(replacement_len).unwrap_or(isize::MAX)
                    - isize::try_from(edit_range.end.0 - edit_range.start.0).unwrap_or(isize::MAX),
            );
        }
        let resulting = SelectionSet::new(resulting, selections.primary_index())?;
        let transaction = TransactionBuilder::new()
            .extend(edits)
            .normalize_identical_edits()
            .selection_after(resulting)
            .build()?;
        self.apply_transaction(transaction).map(Into::into)
    }

    /// Adds one indentation unit at the beginning of every touched line.
    pub fn indent_selections(&mut self, style: IndentStyle) -> Result<AppliedTransaction> {
        let snapshot = self.snapshot();
        let mut lines: Vec<u32> = self
            .selections()
            .selections()
            .iter()
            .flat_map(|selection| {
                let start = snapshot.offset_to_position(selection.range().start);
                let end = snapshot.offset_to_position(selection.range().end);
                match (start, end) {
                    (Ok(start), Ok(end)) => (start.line..=end.line).collect(),
                    _ => Vec::new(),
                }
            })
            .collect();
        lines.sort_unstable();
        lines.dedup();
        let edits = lines
            .into_iter()
            .map(|line| {
                snapshot
                    .position_to_offset(crate::LogicalPosition { line, character: 0 })
                    .map(|offset| Edit::insert(offset, style.unit()))
            })
            .collect::<Result<Vec<_>>>()?;
        self.apply_transaction(TransactionBuilder::new().extend(edits).build()?)
    }

    /// Removes one indentation unit from every touched logical line as one transaction.
    pub fn outdent_selections(&mut self, style: IndentStyle) -> Result<AppliedTransaction> {
        let snapshot = self.snapshot();
        let mut lines: Vec<u32> = self
            .selections()
            .selections()
            .iter()
            .flat_map(|selection| {
                let start = snapshot.offset_to_position(selection.range().start);
                let end = snapshot.offset_to_position(selection.range().end);
                match (start, end) {
                    (Ok(start), Ok(end)) => (start.line..=end.line).collect(),
                    _ => Vec::new(),
                }
            })
            .collect();
        lines.sort_unstable();
        lines.dedup();
        let unit_width = match style {
            IndentStyle::Spaces(width) => width.max(1),
            IndentStyle::Tabs => 1,
        };
        let mut edits = Vec::new();
        for line in lines {
            let text = snapshot.line_text(line)?;
            let remove = if text.starts_with('\t') {
                1
            } else {
                text.chars()
                    .take(unit_width)
                    .take_while(|character| *character == ' ')
                    .count()
            };
            if remove > 0 {
                let start =
                    snapshot.position_to_offset(crate::LogicalPosition { line, character: 0 })?;
                let end = crate::CharacterOffset(start.0.saturating_add(remove));
                edits.push(Edit::delete(crate::TextRange { start, end }));
            }
        }
        self.apply_transaction(TransactionBuilder::new().extend(edits).build()?)
    }
}

#[derive(Debug)]
#[allow(clippy::struct_field_names)]
struct CompiledIndentationRules {
    increase_indent_pattern: Option<regex::Regex>,
    decrease_indent_pattern: Option<regex::Regex>,
    indent_next_line_pattern: Option<regex::Regex>,
    unindented_line_pattern: Option<regex::Regex>,
}

impl CompiledIndentationRules {
    fn compile(rules: &IndentationRules) -> Result<Self> {
        let compile = |pattern: &Option<String>| {
            pattern
                .as_deref()
                .map(regex::Regex::new)
                .transpose()
                .map_err(|error| crate::EditorError::InvalidRegex {
                    message: error.to_string(),
                })
        };
        Ok(Self {
            increase_indent_pattern: compile(&rules.increase_indent_pattern)?,
            decrease_indent_pattern: compile(&rules.decrease_indent_pattern)?,
            indent_next_line_pattern: compile(&rules.indent_next_line_pattern)?,
            unindented_line_pattern: compile(&rules.unindented_line_pattern)?,
        })
    }
}

#[derive(Debug)]
struct CompiledEnterRule {
    before_text: Option<regex::Regex>,
    after_text: Option<regex::Regex>,
    previous_line_text: Option<regex::Regex>,
    append_text: Option<String>,
    remove_text: Option<String>,
    indent_action: Option<String>,
}

impl CompiledEnterRule {
    fn compile(rule: &EnterRule) -> Result<Self> {
        let compile = |pattern: &Option<String>| {
            pattern
                .as_deref()
                .map(regex::Regex::new)
                .transpose()
                .map_err(|error| crate::EditorError::InvalidRegex {
                    message: error.to_string(),
                })
        };
        Ok(Self {
            before_text: compile(&rule.before_text)?,
            after_text: compile(&rule.after_text)?,
            previous_line_text: compile(&rule.previous_line_text)?,
            append_text: rule.append_text.clone(),
            remove_text: rule.remove_text.clone(),
            indent_action: rule.indent_action.clone(),
        })
    }

    fn matches(&self, before: &str, after: &str, previous: Option<&str>) -> bool {
        self.before_text
            .as_ref()
            .is_none_or(|pattern| pattern.is_match(before))
            && self
                .after_text
                .as_ref()
                .is_none_or(|pattern| pattern.is_match(after))
            && self
                .previous_line_text
                .as_ref()
                .is_none_or(|pattern| previous.is_some_and(|line| pattern.is_match(line)))
    }
}

fn remove_indent_unit(indent: &mut String, style: IndentStyle) {
    match style {
        IndentStyle::Spaces(width) => {
            let remove = width.max(1).min(indent.len());
            if indent[..remove].chars().all(|character| character == ' ') {
                indent.drain(..remove);
            }
        }
        IndentStyle::Tabs => {
            if indent.starts_with('\t') {
                indent.remove(0);
            }
        }
    }
}

fn preferred_newline(text: &str) -> &'static str {
    if text.contains("\r\n") && !text.replace("\r\n", "").contains('\n') {
        "\r\n"
    } else {
        "\n"
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;

    fn braces() -> Vec<PairConfig> {
        vec![PairConfig::new("{", "}").expect("valid pair")]
    }

    #[test]
    fn insert_overtype_and_delete_auto_pair() {
        let mut buffer = TextBuffer::new("");
        buffer.smart_insert("{", &braces()).expect("insert pair");
        assert_eq!(buffer.to_string(), "{}");
        assert_eq!(buffer.selections().primary().active, CharacterOffset(1));
        buffer.smart_insert("}", &braces()).expect("overtype");
        assert_eq!(buffer.to_string(), "{}");
        assert_eq!(buffer.version(), 1);
        buffer.move_left(false).expect("move between pair");
        buffer.smart_backspace().expect("paired delete");
        assert_eq!(buffer.to_string(), "");
    }

    #[test]
    fn combining_grapheme_is_deleted_together() {
        let sample = include_str!("../../../tests/fixtures/editor-core/combining_grapheme.txt")
            .trim_end_matches(['\r', '\n']);
        let mut buffer = TextBuffer::new(sample);
        buffer
            .set_selections(SelectionSet::single(Selection::cursor(CharacterOffset(2))))
            .expect("selection");
        buffer.smart_backspace().expect("backspace");
        assert_eq!(buffer.to_string(), "");
    }

    #[rstest]
    #[case("{", "}")]
    #[case("[", "]")]
    #[case("begin", "end")]
    fn enter_expands_configured_pairs(#[case] open: &str, #[case] close: &str) {
        let pair = PairConfig::new(open, close).expect("valid pair");
        let mut buffer = TextBuffer::new(&format!("  {open}{close}"));
        let cursor = 2 + open.chars().count();
        buffer
            .set_selections(SelectionSet::single(Selection::cursor(CharacterOffset(
                cursor,
            ))))
            .expect("selection");
        buffer
            .smart_enter(&[pair], IndentStyle::Spaces(2))
            .expect("enter");
        assert_eq!(buffer.to_string(), format!("  {open}\n    \n  {close}"));
    }

    #[test]
    fn crlf_enter_preserves_document_newline_convention() {
        let mut buffer = TextBuffer::new("{}\r\n");
        buffer
            .set_selections(SelectionSet::single(Selection::cursor(CharacterOffset(1))))
            .expect("selection");
        buffer
            .smart_enter(&braces(), IndentStyle::Tabs)
            .expect("enter");
        assert_eq!(buffer.to_string(), "{\r\n\t\r\n}\r\n");
    }

    #[test]
    fn language_indentation_rules_adjust_newline_indent() {
        let mut buffer = TextBuffer::new("if ready {\n    next");
        buffer
            .set_selections(SelectionSet::single(Selection::cursor(CharacterOffset(19))))
            .expect("selection");
        let rules = IndentationRules {
            increase_indent_pattern: Some(r"\{$".to_owned()),
            decrease_indent_pattern: Some(r"^\s*}".to_owned()),
            ..IndentationRules::default()
        };
        buffer
            .smart_enter_with_rules(&[], IndentStyle::Spaces(4), Some(&rules))
            .expect("enter");
        assert_eq!(buffer.to_string(), "if ready {\n    next\n    ");
    }

    #[test]
    fn language_on_enter_rule_appends_text() {
        let mut buffer = TextBuffer::new("/// docs");
        buffer
            .set_selections(SelectionSet::single(Selection::cursor(CharacterOffset(8))))
            .expect("selection");
        let rules = [EnterRule {
            before_text: Some(r"^///.*$".to_owned()),
            append_text: Some("/// ".to_owned()),
            ..EnterRule::default()
        }];
        buffer
            .smart_enter_with_rules_and_actions(&[], IndentStyle::Spaces(4), None, &rules)
            .expect("enter");
        assert_eq!(buffer.to_string(), "/// docs\n/// ");
    }

    #[test]
    fn outdent_removes_only_configured_indentation() {
        let mut buffer = TextBuffer::new("    one\n  two\ntext");
        buffer
            .set_selections(SelectionSet::single(Selection::new(
                CharacterOffset(0),
                CharacterOffset(12),
            )))
            .expect("selection");
        buffer
            .outdent_selections(IndentStyle::Spaces(2))
            .expect("outdent");
        assert_eq!(buffer.to_string(), "  one\ntwo\ntext");
    }
}
