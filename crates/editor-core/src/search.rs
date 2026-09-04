use regex::{Regex, RegexBuilder};

use crate::{
    AppliedTransaction, CharacterOffset, Edit, EditorError, Result, TextBuffer, TextRange,
    TransactionBuilder,
};

/// Search expression interpretation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchKind {
    Literal,
    RegularExpression,
}

/// In-document search controls.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FindOptions {
    pub kind: SearchKind,
    pub case_sensitive: bool,
    pub whole_word: bool,
}

impl Default for FindOptions {
    fn default() -> Self {
        Self {
            kind: SearchKind::Literal,
            case_sensitive: true,
            whole_word: false,
        }
    }
}

/// A character-offset match suitable for selections and overview markers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FindMatch {
    pub range: TextRange,
}

impl TextBuffer {
    pub fn find(&self, query: &str, options: FindOptions) -> Result<Vec<FindMatch>> {
        if query.is_empty() {
            return Ok(Vec::new());
        }
        let text = self.to_string();
        let regex = build_regex(query, options)?;
        Ok(regex
            .find_iter(&text)
            .map(|matched| FindMatch {
                range: TextRange {
                    start: CharacterOffset(text[..matched.start()].chars().count()),
                    end: CharacterOffset(text[..matched.end()].chars().count()),
                },
            })
            .collect())
    }

    /// Replaces every current match as one undoable transaction.
    pub fn replace_all(
        &mut self,
        query: &str,
        replacement: &str,
        options: FindOptions,
    ) -> Result<AppliedTransaction> {
        if query.is_empty() {
            return Ok(AppliedTransaction {
                changed: false,
                version: self.version(),
            });
        }
        let text = self.to_string();
        let regex = build_regex(query, options)?;
        let edits = regex
            .captures_iter(&text)
            .filter_map(|captures| {
                let matched = captures.get(0)?;
                let mut expanded = String::new();
                if options.kind == SearchKind::RegularExpression {
                    captures.expand(replacement, &mut expanded);
                } else {
                    expanded.push_str(replacement);
                }
                Some(Edit::replace(
                    TextRange {
                        start: CharacterOffset(text[..matched.start()].chars().count()),
                        end: CharacterOffset(text[..matched.end()].chars().count()),
                    },
                    expanded,
                ))
            })
            .collect::<Vec<_>>();
        let transaction = TransactionBuilder::new().extend(edits).build()?;
        self.apply_transaction(transaction)
    }
}

fn build_regex(query: &str, options: FindOptions) -> Result<Regex> {
    let expression = match (options.kind, options.whole_word) {
        (SearchKind::Literal, false) => regex::escape(query),
        (SearchKind::Literal, true) => format!(r"\b{}\b", regex::escape(query)),
        (SearchKind::RegularExpression, false) => query.to_owned(),
        (SearchKind::RegularExpression, true) => format!(r"\b(?:{query})\b"),
    };
    RegexBuilder::new(&expression)
        .case_insensitive(!options.case_sensitive)
        .multi_line(true)
        .build()
        .map_err(|error| EditorError::InvalidRegex {
            message: error.to_string(),
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn literal_case_and_whole_word_options_work() {
        let buffer = TextBuffer::new("Rust rust rusty RUST");
        let matches = buffer
            .find(
                "rust",
                FindOptions {
                    case_sensitive: false,
                    whole_word: true,
                    ..FindOptions::default()
                },
            )
            .expect("search");
        assert_eq!(matches.len(), 3);
    }

    #[test]
    fn regex_capture_replace_is_one_undo_step() {
        let mut buffer = TextBuffer::new("a1 b22");
        buffer
            .replace_all(
                r"([a-z])(\d+)",
                "$2-$1",
                FindOptions {
                    kind: SearchKind::RegularExpression,
                    ..FindOptions::default()
                },
            )
            .expect("replace");
        assert_eq!(buffer.to_string(), "1-a 22-b");
        assert_eq!(buffer.version(), 1);
        assert!(buffer.undo().expect("undo"));
        assert_eq!(buffer.to_string(), "a1 b22");
    }

    #[test]
    fn invalid_regex_is_typed() {
        let buffer = TextBuffer::new("text");
        assert!(matches!(
            buffer.find(
                "(",
                FindOptions {
                    kind: SearchKind::RegularExpression,
                    ..FindOptions::default()
                }
            ),
            Err(EditorError::InvalidRegex { .. })
        ));
    }
}
