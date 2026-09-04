//! Typed settings, JSONC parsing, and deterministic five-layer merging.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

/// Whether long lines wrap in the editor viewport.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum WordWrap {
    #[default]
    Off,
    On,
    WordWrapColumn,
    Bounded,
}

/// Automatic closing-pair behavior.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AutoClosingPairs {
    Never,
    #[default]
    LanguageDefined,
    BeforeWhitespace,
    Always,
}

/// Preferred line ending for newly created documents.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LineEndingPreference {
    #[default]
    Preserve,
    Lf,
    Crlf,
}

/// A structured keybinding entry. Commands are identifiers, not executable strings.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Keybinding {
    pub key: String,
    pub command: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub when: Option<String>,
    #[serde(flatten)]
    pub unknown: BTreeMap<String, Value>,
}

/// RGB color independent of terminal palette depth.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct RgbColor {
    pub red: u8,
    pub green: u8,
    pub blue: u8,
}

/// Style assigned to one logical theme role.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThemeStyle {
    pub foreground: RgbColor,
    pub background: RgbColor,
    #[serde(default)]
    pub bold: bool,
    #[serde(default)]
    pub italic: bool,
    #[serde(default)]
    pub underline: bool,
}

/// Static theme data keyed by logical semantic role name.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Theme {
    pub name: String,
    pub roles: BTreeMap<String, ThemeStyle>,
    #[serde(flatten)]
    pub unknown: BTreeMap<String, Value>,
}

/// A partial settings layer. `None` means the layer does not override that value.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SettingsLayer {
    pub line_numbers: Option<bool>,
    pub tab_size: Option<u8>,
    pub insert_spaces: Option<bool>,
    pub word_wrap: Option<WordWrap>,
    pub auto_closing_pairs: Option<AutoClosingPairs>,
    pub format_on_save: Option<bool>,
    pub format_on_paste: Option<bool>,
    pub theme: Option<String>,
    pub search_excludes: Option<Vec<String>>,
    pub files_excludes: Option<Vec<String>>,
    pub large_file_threshold: Option<u64>,
    pub encoding_fallback: Option<String>,
    pub line_ending: Option<LineEndingPreference>,
    pub language_server_commands: Option<BTreeMap<String, Vec<String>>>,
    pub external_formatter_commands: Option<BTreeMap<String, Vec<String>>>,
    pub keybindings: Option<Vec<Keybinding>>,
    #[serde(flatten)]
    pub unknown: BTreeMap<String, Value>,
}

/// Fully resolved editor settings.
#[derive(Debug, Clone, PartialEq)]
#[allow(clippy::struct_excessive_bools)]
pub struct EditorSettings {
    pub line_numbers: bool,
    pub tab_size: u8,
    pub insert_spaces: bool,
    pub word_wrap: WordWrap,
    pub auto_closing_pairs: AutoClosingPairs,
    pub format_on_save: bool,
    pub format_on_paste: bool,
    pub theme: String,
    pub search_excludes: Vec<String>,
    pub files_excludes: Vec<String>,
    pub large_file_threshold: u64,
    pub encoding_fallback: String,
    pub line_ending: LineEndingPreference,
    pub language_server_commands: BTreeMap<String, Vec<String>>,
    pub external_formatter_commands: BTreeMap<String, Vec<String>>,
    pub keybindings: Vec<Keybinding>,
    pub unknown: BTreeMap<String, Value>,
}

impl Default for EditorSettings {
    fn default() -> Self {
        Self {
            line_numbers: true,
            tab_size: 4,
            insert_spaces: true,
            word_wrap: WordWrap::Off,
            auto_closing_pairs: AutoClosingPairs::LanguageDefined,
            format_on_save: false,
            format_on_paste: false,
            theme: "Editor Dark".to_owned(),
            search_excludes: vec!["**/.git/**".to_owned()],
            files_excludes: Vec::new(),
            large_file_threshold: 32 * 1024 * 1024,
            encoding_fallback: "windows-1252".to_owned(),
            line_ending: LineEndingPreference::Preserve,
            language_server_commands: BTreeMap::new(),
            external_formatter_commands: BTreeMap::new(),
            keybindings: Vec::new(),
            unknown: BTreeMap::new(),
        }
    }
}

impl EditorSettings {
    fn apply(&mut self, layer: &SettingsLayer) {
        macro_rules! replace_copy {
            ($field:ident) => {
                if let Some(value) = layer.$field {
                    self.$field = value;
                }
            };
        }
        macro_rules! replace_clone {
            ($field:ident) => {
                if let Some(value) = &layer.$field {
                    self.$field.clone_from(value);
                }
            };
        }
        replace_copy!(line_numbers);
        replace_copy!(tab_size);
        replace_copy!(insert_spaces);
        replace_copy!(word_wrap);
        replace_copy!(auto_closing_pairs);
        replace_copy!(format_on_save);
        replace_copy!(format_on_paste);
        replace_clone!(theme);
        replace_clone!(search_excludes);
        replace_clone!(files_excludes);
        replace_copy!(large_file_threshold);
        replace_clone!(encoding_fallback);
        replace_copy!(line_ending);
        replace_clone!(language_server_commands);
        replace_clone!(external_formatter_commands);
        replace_clone!(keybindings);
        self.unknown.extend(layer.unknown.clone());
    }
}

/// The fixed five configuration layers, from persistent settings to session overrides.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SettingsStack {
    pub defaults: SettingsLayer,
    pub user: SettingsLayer,
    pub workspace: SettingsLayer,
    pub folder: SettingsLayer,
    pub session: SettingsLayer,
}

impl SettingsStack {
    /// Merges settings in the required low-to-high precedence order.
    #[must_use]
    pub fn resolve(&self) -> EditorSettings {
        let mut resolved = EditorSettings::default();
        for layer in [
            &self.defaults,
            &self.user,
            &self.workspace,
            &self.folder,
            &self.session,
        ] {
            resolved.apply(layer);
        }
        resolved
    }
}

/// A settings source that could not be loaded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SettingsIssue {
    pub path: PathBuf,
    pub message: String,
}

/// Settings loaded with non-fatal, surfaced issues.
#[derive(Debug, Clone, PartialEq)]
pub struct LoadSettingsResult {
    pub layer: SettingsLayer,
    pub issues: Vec<SettingsIssue>,
}

/// Typed configuration parsing and I/O errors.
#[derive(Debug, Error)]
pub enum SettingsError {
    #[error("could not read settings at {path}: {source}")]
    Read {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("invalid JSONC settings: {0}")]
    InvalidJson(#[from] serde_json::Error),
    #[error("could not serialize settings: {0}")]
    Serialize(serde_json::Error),
}

/// Parses JSON with comments and trailing commas without evaluating configuration.
///
/// # Errors
///
/// Returns [`SettingsError::InvalidJson`] when the normalized JSON still fails to parse.
pub fn parse_jsonc_settings(input: &str) -> Result<SettingsLayer, SettingsError> {
    let without_comments = strip_jsonc_comments(input);
    let normalized = strip_trailing_commas(&without_comments);
    Ok(serde_json::from_str(&normalized)?)
}

/// Reads a layer. Malformed or unreadable settings fall back to an empty layer and report an issue.
#[must_use]
pub fn load_settings_or_default(path: &Path) -> LoadSettingsResult {
    match std::fs::read_to_string(path) {
        Ok(contents) => match parse_jsonc_settings(&contents) {
            Ok(layer) => LoadSettingsResult {
                layer,
                issues: Vec::new(),
            },
            Err(error) => LoadSettingsResult {
                layer: SettingsLayer::default(),
                issues: vec![SettingsIssue {
                    path: path.to_path_buf(),
                    message: error.to_string(),
                }],
            },
        },
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => LoadSettingsResult {
            layer: SettingsLayer::default(),
            issues: Vec::new(),
        },
        Err(source) => LoadSettingsResult {
            layer: SettingsLayer::default(),
            issues: vec![SettingsIssue {
                path: path.to_path_buf(),
                message: SettingsError::Read {
                    path: path.to_path_buf(),
                    source,
                }
                .to_string(),
            }],
        },
    }
}

/// Serializes a layer only when a caller explicitly chooses to rewrite it.
///
/// # Errors
///
/// Returns [`SettingsError::Serialize`] when the layer cannot be rendered as JSON.
pub fn serialize_settings(layer: &SettingsLayer) -> Result<String, SettingsError> {
    serde_json::to_string_pretty(layer).map_err(SettingsError::Serialize)
}

fn strip_jsonc_comments(input: &str) -> String {
    let mut output = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    let mut in_string = false;
    let mut escaped = false;
    while let Some(ch) = chars.next() {
        if in_string {
            output.push(ch);
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                in_string = false;
            }
            continue;
        }
        if ch == '"' {
            in_string = true;
            output.push(ch);
        } else if ch == '/' && chars.peek() == Some(&'/') {
            chars.next();
            for comment in chars.by_ref() {
                if comment == '\n' {
                    output.push('\n');
                    break;
                }
            }
        } else if ch == '/' && chars.peek() == Some(&'*') {
            chars.next();
            let mut previous = '\0';
            for comment in chars.by_ref() {
                if comment == '\n' {
                    output.push('\n');
                }
                if previous == '*' && comment == '/' {
                    break;
                }
                previous = comment;
            }
        } else {
            output.push(ch);
        }
    }
    output
}

fn strip_trailing_commas(input: &str) -> String {
    let mut output = String::with_capacity(input.len());
    let chars: Vec<char> = input.chars().collect();
    let mut in_string = false;
    let mut escaped = false;
    for (index, ch) in chars.iter().copied().enumerate() {
        if in_string {
            output.push(ch);
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                in_string = false;
            }
            continue;
        }
        if ch == '"' {
            in_string = true;
            output.push(ch);
        } else if ch == ',' {
            let next = chars[index + 1..]
                .iter()
                .find(|candidate| !candidate.is_whitespace());
            if !matches!(next, Some('}' | ']')) {
                output.push(ch);
            }
        } else {
            output.push(ch);
        }
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn five_layers_have_expected_precedence() {
        let stack = SettingsStack {
            defaults: SettingsLayer {
                tab_size: Some(2),
                ..SettingsLayer::default()
            },
            user: SettingsLayer {
                tab_size: Some(3),
                ..SettingsLayer::default()
            },
            workspace: SettingsLayer {
                tab_size: Some(4),
                ..SettingsLayer::default()
            },
            folder: SettingsLayer {
                tab_size: Some(5),
                ..SettingsLayer::default()
            },
            session: SettingsLayer {
                tab_size: Some(6),
                ..SettingsLayer::default()
            },
        };
        assert_eq!(stack.resolve().tab_size, 6);
    }

    #[test]
    fn jsonc_allows_comments_trailing_commas_and_preserves_unknown() {
        let input = r#"{
            // compatible comment
            "tabSize": 2,
            "future.setting": { "enabled": true, },
            "theme": "https://example.invalid/a//b",
        }"#;
        let layer = parse_jsonc_settings(input).expect("valid JSONC");
        assert_eq!(layer.tab_size, Some(2));
        assert_eq!(layer.unknown["future.setting"]["enabled"], true);
        let rewritten = serialize_settings(&layer).expect("serializes");
        let reparsed = parse_jsonc_settings(&rewritten).expect("rewritten settings parse");
        assert_eq!(reparsed.unknown, layer.unknown);
    }

    #[test]
    fn malformed_settings_fall_back_and_surface_error() {
        let directory = tempfile::tempdir().expect("temp directory");
        let path = directory.path().join("settings.json");
        std::fs::write(&path, "{ invalid").expect("write malformed settings");
        let result = load_settings_or_default(&path);
        assert_eq!(result.layer, SettingsLayer::default());
        assert_eq!(result.issues.len(), 1);
    }

    #[test]
    fn commands_remain_structured_arguments() {
        let layer = parse_jsonc_settings(
            r#"{"languageServerCommands":{"rust":["rust-analyzer","--stdio"]}}"#,
        )
        .expect("settings parse");
        assert_eq!(
            layer.language_server_commands.expect("commands")["rust"],
            ["rust-analyzer", "--stdio"]
        );
    }
}
