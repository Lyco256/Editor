//! Protocol-neutral types shared across Editor subsystems.

use serde::{Deserialize, Serialize};
use std::fmt;

macro_rules! numeric_id {
    ($name:ident) => {
        #[derive(
            Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
        )]
        pub struct $name(pub u64);
    };
}

numeric_id!(DocumentId);
numeric_id!(WorkspaceId);
numeric_id!(FileId);
numeric_id!(RequestId);
numeric_id!(CancellationId);

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct CommandId(String);

impl CommandId {
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for CommandId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct CharacterOffset(pub usize);

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LogicalPosition {
    pub line: u32,
    pub character: u32,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScreenCell {
    pub row: u16,
    pub column: u16,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TextRange {
    pub start: CharacterOffset,
    pub end: CharacterOffset,
}

impl TextRange {
    #[must_use]
    pub fn new(start: CharacterOffset, end: CharacterOffset) -> Option<Self> {
        (start <= end).then_some(Self { start, end })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Modifier {
    Control,
    Alt,
    Shift,
    Meta,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Modifiers(u8);

impl Modifiers {
    const CONTROL: u8 = 1;
    const ALT: u8 = 1 << 1;
    const SHIFT: u8 = 1 << 2;
    const META: u8 = 1 << 3;

    #[must_use]
    pub fn from_modifiers(modifiers: impl IntoIterator<Item = Modifier>) -> Self {
        let mut value = 0;
        for modifier in modifiers {
            value |= match modifier {
                Modifier::Control => Self::CONTROL,
                Modifier::Alt => Self::ALT,
                Modifier::Shift => Self::SHIFT,
                Modifier::Meta => Self::META,
            };
        }
        Self(value)
    }

    #[must_use]
    pub const fn contains(self, modifier: Modifier) -> bool {
        let bit = match modifier {
            Modifier::Control => Self::CONTROL,
            Modifier::Alt => Self::ALT,
            Modifier::Shift => Self::SHIFT,
            Modifier::Meta => Self::META,
        };
        self.0 & bit != 0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum KeyCode {
    Character(char),
    Enter,
    Escape,
    Backspace,
    Delete,
    Tab,
    Up,
    Down,
    Left,
    Right,
    Home,
    End,
    PageUp,
    PageDown,
    Function(u8),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeyEvent {
    pub code: KeyCode,
    pub modifiers: Modifiers,
    pub repeat: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MouseButton {
    Left,
    Middle,
    Right,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MouseAction {
    Down(MouseButton),
    Up(MouseButton),
    Drag(MouseButton),
    Move,
    ScrollLines(i16),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct MouseEvent {
    pub position: ScreenCell,
    pub action: MouseAction,
    pub modifiers: Modifiers,
    #[serde(default = "default_click_count")]
    pub click_count: u8,
}

fn default_click_count() -> u8 {
    1
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum InputEvent {
    Key(KeyEvent),
    Mouse(MouseEvent),
    Paste(String),
    Resize { columns: u16, rows: u16 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DiagnosticSeverity {
    Error,
    Warning,
    Information,
    Hint,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Diagnostic {
    pub document: DocumentId,
    pub range: TextRange,
    pub severity: DiagnosticSeverity,
    pub message: String,
    pub source: Option<String>,
    pub version: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum StyleRole {
    EditorText,
    EditorBackground,
    Selection,
    CurrentLine,
    LineNumber,
    Gutter,
    StatusBar,
    Panel,
    Error,
    Warning,
    Information,
    Hint,
    GitAdded,
    GitModified,
    GitDeleted,
    SearchMatch,
    SyntaxKeyword,
    SyntaxString,
    SyntaxComment,
    SemanticType,
    CurrentLineBackground,
    SelectionForeground,
    SelectionBackground,
    SecondaryCursorForeground,
    SecondaryCursorBackground,
    LineNumberActive,
    SplitSeparator,
    MenuForeground,
    MenuBackground,
    MenuSelectedForeground,
    MenuSelectedBackground,
    ExplorerForeground,
    ExplorerBackground,
    ExplorerDirectory,
    ExplorerSelectedForeground,
    ExplorerSelectedBackground,
    TabForeground,
    TabBackground,
    TabActiveForeground,
    TabActiveBackground,
    TabPreviewForeground,
    TabPinnedForeground,
    PanelForeground,
    PanelBackground,
    PanelTitle,
    InputForeground,
    InputBackground,
    StatusForeground,
    StatusBackground,
    Border,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ColorDepth {
    TrueColor,
    Color256,
    Color16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum UnderlineStyle {
    Curly,
    Straight,
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TerminalFeature {
    EnhancedKeyboard,
    Mouse,
    BracketedPaste,
    AlternateScreen,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TerminalFeatures(u8);

impl TerminalFeatures {
    const ENHANCED_KEYBOARD: u8 = 1;
    const MOUSE: u8 = 1 << 1;
    const BRACKETED_PASTE: u8 = 1 << 2;
    const ALTERNATE_SCREEN: u8 = 1 << 3;

    #[must_use]
    pub fn from_features(features: impl IntoIterator<Item = TerminalFeature>) -> Self {
        let mut value = 0;
        for feature in features {
            value |= match feature {
                TerminalFeature::EnhancedKeyboard => Self::ENHANCED_KEYBOARD,
                TerminalFeature::Mouse => Self::MOUSE,
                TerminalFeature::BracketedPaste => Self::BRACKETED_PASTE,
                TerminalFeature::AlternateScreen => Self::ALTERNATE_SCREEN,
            };
        }
        Self(value)
    }

    #[must_use]
    pub const fn contains(self, feature: TerminalFeature) -> bool {
        let bit = match feature {
            TerminalFeature::EnhancedKeyboard => Self::ENHANCED_KEYBOARD,
            TerminalFeature::Mouse => Self::MOUSE,
            TerminalFeature::BracketedPaste => Self::BRACKETED_PASTE,
            TerminalFeature::AlternateScreen => Self::ALTERNATE_SCREEN,
        };
        self.0 & bit != 0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TerminalCapabilities {
    pub color_depth: ColorDepth,
    pub underline: UnderlineStyle,
    pub features: TerminalFeatures,
}

impl Default for TerminalCapabilities {
    fn default() -> Self {
        Self {
            color_depth: ColorDepth::Color16,
            underline: UnderlineStyle::None,
            features: TerminalFeatures::default(),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct GitStatusSummary {
    pub branch: Option<String>,
    pub staged: usize,
    pub unstaged: usize,
    pub untracked: usize,
    pub conflicts: usize,
    pub busy: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum LanguageServerStatus {
    Unavailable,
    #[default]
    Stopped,
    Starting,
    Running {
        name: String,
    },
    Crashed {
        message: String,
    },
    Restarting,
    DisabledByPolicy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OutputLevel {
    Trace,
    Information,
    Warning,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OutputMessage {
    pub subsystem: String,
    pub operation: String,
    pub level: OutputLevel,
    pub message: String,
}

#[cfg(test)]
mod tests {
    use super::{CharacterOffset, CommandId, TextRange};

    #[test]
    fn invalid_range_is_rejected() {
        assert!(TextRange::new(CharacterOffset(2), CharacterOffset(1)).is_none());
    }

    #[test]
    fn command_id_preserves_namespaced_value() {
        let id = CommandId::new("editor.save");
        assert_eq!(id.as_str(), "editor.save");
    }
}
