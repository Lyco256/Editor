//! Language-intelligence view boundary.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LanguagePopup {
    Completion,
    Hover,
    Signature,
    CodeAction,
}
