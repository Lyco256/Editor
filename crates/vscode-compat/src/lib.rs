//! Static VS Code data compatibility boundary.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContributionKind {
    Theme,
    Snippet,
    Language,
    LanguageConfiguration,
    Unsupported,
}
