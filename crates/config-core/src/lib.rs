//! Configuration, encoding, `EditorConfig`, and recovery services.

pub mod editorconfig;
pub mod encoding;
pub mod recovery;
pub mod settings;

pub use editorconfig::{DocumentSettings, EditorConfigError, load_editorconfig};
pub use encoding::{
    DecodePolicy, DecodedText, EncodingError, EncodingKind, LineEndings, decode, detect_encoding,
    encode, inspect_line_endings, normalize_line_endings,
};
pub use recovery::{
    CURRENT_SESSION_FORMAT, CursorState, EditorSession, RecoveryError, RecoveryLoad, RecoveryStore,
    RecoveryWarning, SelectionState, SessionState, SplitAxis, SplitLayout,
};
pub use settings::{
    AutoClosingPairs, EditorSettings, Keybinding, LineEndingPreference, LoadSettingsResult,
    RgbColor, SettingsError, SettingsIssue, SettingsLayer, SettingsStack, Theme, ThemeStyle,
    WordWrap, parse_jsonc_settings,
};

/// Settings that control the large-file safety boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LargeFileSettings {
    /// Size at which whole-document semantic services are disabled.
    pub threshold_bytes: u64,
}

impl Default for LargeFileSettings {
    fn default() -> Self {
        Self {
            threshold_bytes: 32 * 1024 * 1024,
        }
    }
}
