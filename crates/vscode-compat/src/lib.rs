//! Static VS Code data compatibility boundary.
#![allow(
    clippy::elidable_lifetime_names,
    clippy::missing_errors_doc,
    clippy::needless_pass_by_value,
    clippy::too_many_lines
)]
//!
//! This crate parses safe, static VS Code compatibility data without executing
//! extension code. It owns JSONC settings translation, keybinding mapping,
//! theme/snippet/language configuration parsing, local extension discovery, and
//! strict VSIX extraction.

use std::collections::BTreeMap;
use std::ffi::OsStr;
use std::fs;
use std::path::{Component, Path, PathBuf};

use editor_types::CommandId;
use serde::Deserialize;
use serde_json::{Map, Value};
use thiserror::Error;
use zip::read::ZipArchive;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContributionKind {
    Settings,
    Keybinding,
    Theme,
    Snippet,
    Language,
    LanguageConfiguration,
    Grammar,
    Unsupported,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WordWrap {
    Off,
    On,
    WordWrapColumn,
    Bounded,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AutoClosingPairs {
    Never,
    LanguageDefined,
    BeforeWhitespace,
    Always,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineEndingPreference {
    Preserve,
    Lf,
    Crlf,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Keybinding {
    pub key: String,
    pub command: String,
    pub when: Option<String>,
    pub unknown: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Default, PartialEq)]
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
    pub unknown: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompatibilityWarningKind {
    UnsupportedSetting { key: String },
    UnsupportedCommand { command: String },
    UnsupportedContribution { name: String },
    UnsupportedExecutableCode { name: String },
    UnsupportedField { field: String },
    UnsupportedArchiveEntry { entry: String },
    UnsupportedThemeInclude { reference: String },
    UnsupportedSnippetTransform { expression: String },
    UnsupportedLanguageConfigurationField { field: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompatibilityWarning {
    pub path: Option<PathBuf>,
    pub kind: CompatibilityWarningKind,
    pub contribution: ContributionKind,
    pub message: String,
}

#[derive(Debug, Error)]
pub enum CompatibilityError {
    #[error("could not read {path}: {source}")]
    Read {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("invalid JSON or JSONC at {path}: {source}")]
    InvalidJson {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },
    #[error("invalid manifest at {path}: {message}")]
    InvalidManifest { path: PathBuf, message: String },
    #[error("invalid theme at {path}: {message}")]
    InvalidTheme { path: PathBuf, message: String },
    #[error("invalid snippet file at {path}: {message}")]
    InvalidSnippetFile { path: PathBuf, message: String },
    #[error("invalid language configuration at {path}: {message}")]
    InvalidLanguageConfiguration { path: PathBuf, message: String },
    #[error("invalid VSIX entry {entry} in {archive}: {message}")]
    InvalidVsixEntry {
        archive: PathBuf,
        entry: String,
        message: String,
    },
    #[error("zip error in {archive}: {source}")]
    Zip {
        archive: PathBuf,
        #[source]
        source: zip::result::ZipError,
    },
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtensionManifest {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub display_name: Option<String>,
    #[serde(default)]
    pub publisher: Option<String>,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub main: Option<String>,
    #[serde(default)]
    pub browser: Option<String>,
    #[serde(default)]
    pub activation_events: Option<Vec<String>>,
    #[serde(default)]
    pub contributes: Option<ExtensionContributions>,
    #[serde(flatten)]
    pub unknown: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct ExtensionContributions {
    #[serde(default)]
    pub themes: Vec<ThemeContributionSpec>,
    #[serde(default)]
    pub snippets: Vec<SnippetContributionSpec>,
    #[serde(default)]
    pub languages: Vec<LanguageContributionSpec>,
    #[serde(default)]
    pub grammars: Vec<GrammarContributionSpec>,
    #[serde(flatten)]
    pub unknown: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThemeContributionSpec {
    pub label: String,
    pub path: String,
    #[serde(default)]
    pub ui_theme: Option<String>,
    #[serde(flatten)]
    pub unknown: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SnippetContributionSpec {
    pub language: Option<String>,
    pub path: String,
    #[serde(flatten)]
    pub unknown: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LanguageContributionSpec {
    pub id: String,
    #[serde(default)]
    pub aliases: Vec<String>,
    #[serde(default)]
    pub extensions: Vec<String>,
    #[serde(default)]
    pub filenames: Vec<String>,
    #[serde(default)]
    pub mimetypes: Vec<String>,
    #[serde(default)]
    pub configuration: Option<String>,
    #[serde(flatten)]
    pub unknown: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GrammarContributionSpec {
    pub language: String,
    pub scope_name: String,
    pub path: String,
    #[serde(default)]
    pub embedded_languages: BTreeMap<String, String>,
    #[serde(flatten)]
    pub unknown: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct StaticExtensionPackage {
    pub root: PathBuf,
    pub manifest: ExtensionManifest,
    pub themes: Vec<StaticTheme>,
    pub snippets: Vec<StaticSnippetFile>,
    pub languages: Vec<StaticLanguage>,
    pub warnings: Vec<CompatibilityWarning>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThemeKind {
    Dark,
    Light,
    HighContrast,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StaticTheme {
    pub name: String,
    pub kind: ThemeKind,
    pub include: Option<PathBuf>,
    pub colors: BTreeMap<String, ThemeColor>,
    pub token_colors: Vec<TokenColorRule>,
    pub semantic_token_colors: BTreeMap<String, ThemeColor>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ThemeColor {
    Hex(String),
    Style(ThemeStyleSpec),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThemeStyleSpec {
    pub foreground: Option<String>,
    pub background: Option<String>,
    pub font_style: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TokenColorRule {
    pub scopes: Vec<String>,
    pub settings: ThemeStyleSpec,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StaticSnippetFile {
    pub path: PathBuf,
    pub language: Option<String>,
    pub snippets: Vec<SnippetDefinition>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SnippetDefinition {
    pub name: String,
    pub prefix: Vec<String>,
    pub description: Option<String>,
    pub scope: Option<String>,
    pub body: SnippetBody,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SnippetBody {
    pub raw: String,
    pub parts: Vec<SnippetPart>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SnippetPart {
    Text(String),
    TabStop(u32),
    Placeholder {
        index: u32,
        default: Vec<SnippetPart>,
    },
    Variable {
        name: String,
        default: Vec<SnippetPart>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StaticLanguage {
    pub id: String,
    pub aliases: Vec<String>,
    pub extensions: Vec<String>,
    pub filenames: Vec<String>,
    pub mimetypes: Vec<String>,
    pub configuration: Option<LanguageConfiguration>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LanguageConfiguration {
    pub comments: Option<LanguageComments>,
    pub brackets: Vec<BracketPair>,
    pub auto_closing_pairs: Vec<BracketPair>,
    pub surrounding_pairs: Vec<BracketPair>,
    pub indentation_rules: Option<IndentationRules>,
    pub word_pattern: Option<String>,
    pub on_enter_rules: Vec<OnEnterRule>,
    pub folding_markers: Option<FoldingMarkers>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LanguageComments {
    pub line_comment: Option<String>,
    pub block_comment: Option<BracketPair>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BracketPair {
    pub open: String,
    pub close: String,
    pub not_in: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndentationRules {
    pub increase_indent_pattern: Option<String>,
    pub decrease_indent_pattern: Option<String>,
    pub indent_next_line_pattern: Option<String>,
    pub unindented_line_pattern: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OnEnterRule {
    pub before_text: Option<String>,
    pub after_text: Option<String>,
    pub previous_line_text: Option<String>,
    pub action: Option<OnEnterAction>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OnEnterAction {
    pub indent_action: Option<String>,
    pub append_text: Option<String>,
    pub remove_text: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FoldingMarkers {
    pub start: Option<String>,
    pub end: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ParsedSettings {
    pub layer: SettingsLayer,
    pub warnings: Vec<CompatibilityWarning>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ParsedKeybindings {
    pub keybindings: Vec<Keybinding>,
    pub warnings: Vec<CompatibilityWarning>,
}

pub fn load_vscode_settings_jsonc(input: &str) -> Result<ParsedSettings, CompatibilityError> {
    let value = parse_jsonc_value(input, Path::new("<settings>"))?;
    let object = value
        .as_object()
        .ok_or_else(|| CompatibilityError::InvalidManifest {
            path: PathBuf::from("<settings>"),
            message: "settings file must contain a JSON object".to_owned(),
        })?;
    Ok(parse_settings_object(object))
}

pub fn load_vscode_keybindings_jsonc(input: &str) -> Result<ParsedKeybindings, CompatibilityError> {
    let value = parse_jsonc_value(input, Path::new("<keybindings>"))?;
    let array = value
        .as_array()
        .ok_or_else(|| CompatibilityError::InvalidManifest {
            path: PathBuf::from("<keybindings>"),
            message: "keybindings file must contain a JSON array".to_owned(),
        })?;
    Ok(parse_keybindings_array(array))
}

pub fn load_theme_json(
    path: impl AsRef<Path>,
    input: &str,
) -> Result<(StaticTheme, Vec<CompatibilityWarning>), CompatibilityError> {
    let path = path.as_ref().to_path_buf();
    let value = parse_jsonc_value(input, &path)?;
    parse_theme_value(&path, value)
}

pub fn load_snippet_json(
    path: impl AsRef<Path>,
    input: &str,
) -> Result<(StaticSnippetFile, Vec<CompatibilityWarning>), CompatibilityError> {
    let path = path.as_ref().to_path_buf();
    let value = parse_jsonc_value(input, &path)?;
    parse_snippet_value(&path, value, None)
}

pub fn load_language_configuration_json(
    path: impl AsRef<Path>,
    input: &str,
) -> Result<(LanguageConfiguration, Vec<CompatibilityWarning>), CompatibilityError> {
    let path = path.as_ref().to_path_buf();
    let value = parse_jsonc_value(input, &path)?;
    parse_language_configuration_value(&path, value)
}

pub fn load_static_extension_directory(
    root: impl AsRef<Path>,
) -> Result<StaticExtensionPackage, CompatibilityError> {
    let root = root.as_ref().to_path_buf();
    let manifest_path = root.join("package.json");
    let manifest_source = read_to_string(&manifest_path)?;
    let manifest_value = parse_jsonc_value(&manifest_source, &manifest_path)?;
    let manifest =
        serde_json::from_value::<ExtensionManifest>(manifest_value).map_err(|source| {
            CompatibilityError::InvalidManifest {
                path: manifest_path.clone(),
                message: source.to_string(),
            }
        })?;

    let mut warnings = Vec::new();
    warnings.extend(manifest_warnings(&manifest, &manifest_path));

    let mut themes = Vec::new();
    let mut snippets = Vec::new();
    let mut languages = Vec::new();

    let contributions = manifest.contributes.clone().unwrap_or_default();

    for unsupported in contributions.unknown.keys() {
        warnings.push(CompatibilityWarning {
            path: Some(manifest_path.clone()),
            kind: CompatibilityWarningKind::UnsupportedContribution {
                name: unsupported.clone(),
            },
            contribution: ContributionKind::Unsupported,
            message: format!("unsupported contribution point `{unsupported}`"),
        });
    }

    for grammar in contributions.grammars {
        warnings.push(CompatibilityWarning {
            path: Some(manifest_path.clone()),
            kind: CompatibilityWarningKind::UnsupportedContribution {
                name: "grammars".to_owned(),
            },
            contribution: ContributionKind::Grammar,
            message: format!(
                "TextMate grammar `{}` is detected but not activated as the primary parser",
                grammar.scope_name
            ),
        });
    }

    for contribution in contributions.themes {
        let theme_path = resolve_within_root(&root, &contribution.path).map_err(|message| {
            CompatibilityError::InvalidManifest {
                path: manifest_path.clone(),
                message,
            }
        })?;
        let theme_source = read_to_string(&theme_path)?;
        let (theme, theme_warnings) = load_theme_json(&theme_path, &theme_source)?;
        warnings.extend(theme_warnings);
        themes.push(theme);
    }

    for contribution in contributions.snippets {
        let snippet_path = resolve_within_root(&root, &contribution.path).map_err(|message| {
            CompatibilityError::InvalidManifest {
                path: manifest_path.clone(),
                message,
            }
        })?;
        let snippet_source = read_to_string(&snippet_path)?;
        let (snippet, snippet_warnings) = parse_snippet_value(
            &snippet_path,
            parse_jsonc_value(&snippet_source, &snippet_path)?,
            contribution.language.clone(),
        )?;
        warnings.extend(snippet_warnings);
        snippets.push(snippet);
    }

    for contribution in contributions.languages {
        let configuration = match contribution.configuration {
            Some(reference) => {
                let configuration_path =
                    resolve_within_root(&root, &reference).map_err(|message| {
                        CompatibilityError::InvalidManifest {
                            path: manifest_path.clone(),
                            message,
                        }
                    })?;
                let configuration_source = read_to_string(&configuration_path)?;
                let (configuration, configuration_warnings) =
                    load_language_configuration_json(&configuration_path, &configuration_source)?;
                warnings.extend(configuration_warnings);
                Some(configuration)
            }
            None => None,
        };

        for unsupported in contribution.unknown.keys() {
            warnings.push(CompatibilityWarning {
                path: Some(manifest_path.clone()),
                kind: CompatibilityWarningKind::UnsupportedField {
                    field: unsupported.clone(),
                },
                contribution: ContributionKind::Language,
                message: format!(
                    "unsupported language contribution field `{unsupported}` for `{}`",
                    contribution.id
                ),
            });
        }

        languages.push(StaticLanguage {
            id: contribution.id,
            aliases: contribution.aliases,
            extensions: contribution.extensions,
            filenames: contribution.filenames,
            mimetypes: contribution.mimetypes,
            configuration,
        });
    }

    Ok(StaticExtensionPackage {
        root,
        manifest,
        themes,
        snippets,
        languages,
        warnings,
    })
}

pub fn load_static_extension_vsix(
    archive: impl AsRef<Path>,
    destination: impl AsRef<Path>,
) -> Result<StaticExtensionPackage, CompatibilityError> {
    let extracted_root = extract_vsix(archive.as_ref(), destination.as_ref())?;
    load_static_extension_directory(extracted_root)
}

pub fn extract_vsix(
    archive: impl AsRef<Path>,
    destination: impl AsRef<Path>,
) -> Result<PathBuf, CompatibilityError> {
    let archive = archive.as_ref().to_path_buf();
    let destination = destination.as_ref().to_path_buf();
    fs::create_dir_all(&destination).map_err(|source| CompatibilityError::Read {
        path: destination.clone(),
        source,
    })?;

    let file = fs::File::open(&archive).map_err(|source| CompatibilityError::Read {
        path: archive.clone(),
        source,
    })?;
    let mut zip = ZipArchive::new(file).map_err(|source| CompatibilityError::Zip {
        archive: archive.clone(),
        source,
    })?;

    let stem = archive
        .file_stem()
        .and_then(OsStr::to_str)
        .filter(|value| {
            !value.is_empty()
                && *value != "."
                && *value != ".."
                && value
                    .chars()
                    .all(|character| character.is_ascii_alphanumeric() || "._-".contains(character))
        })
        .unwrap_or("vsix");
    // Validate every archive name before creating any output.  A malformed later entry must not
    // leave a partially extracted tree behind.
    let mut validated_entries = Vec::with_capacity(zip.len());
    for index in 0..zip.len() {
        let entry = zip
            .by_index(index)
            .map_err(|source| CompatibilityError::Zip {
                archive: archive.clone(),
                source,
            })?;
        let entry_name = entry.name().replace('\\', "/");
        if entry_name.is_empty() {
            continue;
        }

        let entry_path = Path::new(&entry_name);
        let safe_relative = sanitize_zip_entry(entry_path).map_err(|message| {
            CompatibilityError::InvalidVsixEntry {
                archive: archive.clone(),
                entry: entry_name.clone(),
                message,
            }
        })?;
        validated_entries.push((index, safe_relative.clone(), entry.is_dir()));
    }

    let extraction_root = destination.join(stem);
    fs::create_dir_all(&extraction_root).map_err(|source| CompatibilityError::Read {
        path: extraction_root.clone(),
        source,
    })?;

    for (index, safe_relative, is_dir) in validated_entries {
        let mut entry = zip
            .by_index(index)
            .map_err(|source| CompatibilityError::Zip {
                archive: archive.clone(),
                source,
            })?;
        let output_path = extraction_root.join(safe_relative);

        if is_dir {
            fs::create_dir_all(&output_path).map_err(|source| CompatibilityError::Read {
                path: output_path.clone(),
                source,
            })?;
            continue;
        }

        if let Some(parent) = output_path.parent() {
            fs::create_dir_all(parent).map_err(|source| CompatibilityError::Read {
                path: parent.to_path_buf(),
                source,
            })?;
        }

        let mut output_file =
            fs::File::create(&output_path).map_err(|source| CompatibilityError::Read {
                path: output_path.clone(),
                source,
            })?;
        std::io::copy(&mut entry, &mut output_file).map_err(|source| CompatibilityError::Read {
            path: output_path,
            source,
        })?;
    }

    Ok(extraction_root)
}

fn parse_settings_object(object: &Map<String, Value>) -> ParsedSettings {
    let mut layer = SettingsLayer::default();
    let mut warnings = Vec::new();

    for (key, value) in object {
        match key.as_str() {
            "editor.lineNumbers" => {
                if let Some(flag) = value.as_bool() {
                    layer.line_numbers = Some(flag);
                } else {
                    push_setting_warning(&mut warnings, key, "expected a boolean");
                }
            }
            "editor.tabSize" => {
                if let Some(size) = value.as_u64().and_then(|raw| u8::try_from(raw).ok()) {
                    layer.tab_size = Some(size);
                } else {
                    push_setting_warning(&mut warnings, key, "expected an integer in the u8 range");
                }
            }
            "editor.insertSpaces" => {
                if let Some(flag) = value.as_bool() {
                    layer.insert_spaces = Some(flag);
                } else {
                    push_setting_warning(&mut warnings, key, "expected a boolean");
                }
            }
            "editor.wordWrap" => {
                if let Some(word_wrap) = parse_word_wrap(value) {
                    layer.word_wrap = Some(word_wrap);
                } else {
                    push_setting_warning(
                        &mut warnings,
                        key,
                        "expected one of off, on, wordWrapColumn, or bounded",
                    );
                }
            }
            "editor.autoClosingPairs" => {
                if let Some(auto_closing_pairs) = parse_auto_closing_pairs(value) {
                    layer.auto_closing_pairs = Some(auto_closing_pairs);
                } else {
                    push_setting_warning(
                        &mut warnings,
                        key,
                        "expected never, languageDefined, beforeWhitespace, or always",
                    );
                }
            }
            "editor.formatOnSave" => {
                if let Some(flag) = value.as_bool() {
                    layer.format_on_save = Some(flag);
                } else {
                    push_setting_warning(&mut warnings, key, "expected a boolean");
                }
            }
            "editor.formatOnPaste" => {
                if let Some(flag) = value.as_bool() {
                    layer.format_on_paste = Some(flag);
                } else {
                    push_setting_warning(&mut warnings, key, "expected a boolean");
                }
            }
            "workbench.colorTheme" => {
                if let Some(theme) = value.as_str() {
                    layer.theme = Some(theme.to_owned());
                } else {
                    push_setting_warning(&mut warnings, key, "expected a string");
                }
            }
            "search.exclude" => {
                if let Some(excludes) = parse_glob_object(value) {
                    layer.search_excludes = Some(excludes);
                } else {
                    push_setting_warning(&mut warnings, key, "expected an object of glob rules");
                }
            }
            "files.exclude" => {
                if let Some(excludes) = parse_glob_object(value) {
                    layer.files_excludes = Some(excludes);
                } else {
                    push_setting_warning(&mut warnings, key, "expected an object of glob rules");
                }
            }
            "files.encoding" => {
                if let Some(encoding) = value.as_str() {
                    layer.encoding_fallback = Some(encoding.to_owned());
                } else {
                    push_setting_warning(&mut warnings, key, "expected a string");
                }
            }
            "files.eol" => {
                if let Some(line_ending) = parse_line_ending(value) {
                    layer.line_ending = Some(line_ending);
                } else {
                    push_setting_warning(&mut warnings, key, "expected lf, crlf, or auto");
                }
            }
            "editor.largeFileThreshold" => {
                if let Some(threshold) = value.as_u64() {
                    layer.large_file_threshold = Some(threshold);
                } else {
                    push_setting_warning(&mut warnings, key, "expected an integer");
                }
            }
            _ => warnings.push(CompatibilityWarning {
                path: None,
                kind: CompatibilityWarningKind::UnsupportedSetting { key: key.clone() },
                contribution: ContributionKind::Settings,
                message: format!("unsupported VS Code setting `{key}`"),
            }),
        }
    }

    ParsedSettings { layer, warnings }
}

fn parse_keybindings_array(array: &[Value]) -> ParsedKeybindings {
    let mut keybindings = Vec::new();
    let mut warnings = Vec::new();

    for (index, item) in array.iter().enumerate() {
        let Some(object) = item.as_object() else {
            warnings.push(CompatibilityWarning {
                path: None,
                kind: CompatibilityWarningKind::UnsupportedField {
                    field: format!("keybinding[{index}]"),
                },
                contribution: ContributionKind::Keybinding,
                message: format!("keybinding entry {index} must be an object"),
            });
            continue;
        };

        let Some(key) = object.get("key").and_then(Value::as_str) else {
            warnings.push(CompatibilityWarning {
                path: None,
                kind: CompatibilityWarningKind::UnsupportedField {
                    field: "key".to_owned(),
                },
                contribution: ContributionKind::Keybinding,
                message: format!("keybinding entry {index} is missing a string `key`"),
            });
            continue;
        };

        let Some(command) = object.get("command").and_then(Value::as_str) else {
            warnings.push(CompatibilityWarning {
                path: None,
                kind: CompatibilityWarningKind::UnsupportedField {
                    field: "command".to_owned(),
                },
                contribution: ContributionKind::Keybinding,
                message: format!("keybinding entry {index} is missing a string `command`"),
            });
            continue;
        };

        let Some(mapped_command) = map_keybinding_command(command) else {
            warnings.push(CompatibilityWarning {
                path: None,
                kind: CompatibilityWarningKind::UnsupportedCommand {
                    command: command.to_owned(),
                },
                contribution: ContributionKind::Keybinding,
                message: format!("unsupported keybinding command `{command}`"),
            });
            continue;
        };

        if object.get("args").is_some() {
            warnings.push(CompatibilityWarning {
                path: None,
                kind: CompatibilityWarningKind::UnsupportedField {
                    field: "args".to_owned(),
                },
                contribution: ContributionKind::Keybinding,
                message: format!("keybinding `{command}` arguments are ignored"),
            });
        }

        let when = match object.get("when") {
            Some(value) if value.is_string() => value.as_str().map(ToOwned::to_owned),
            Some(_) => {
                warnings.push(CompatibilityWarning {
                    path: None,
                    kind: CompatibilityWarningKind::UnsupportedField {
                        field: "when".to_owned(),
                    },
                    contribution: ContributionKind::Keybinding,
                    message: format!("keybinding `{command}` has a non-string `when` clause"),
                });
                None
            }
            None => None,
        };

        keybindings.push(Keybinding {
            key: key.to_owned(),
            command: mapped_command.as_str().to_owned(),
            when,
            unknown: BTreeMap::new(),
        });
    }

    ParsedKeybindings {
        keybindings,
        warnings,
    }
}

fn parse_theme_value(
    path: &Path,
    value: Value,
) -> Result<(StaticTheme, Vec<CompatibilityWarning>), CompatibilityError> {
    let object = value
        .as_object()
        .ok_or_else(|| CompatibilityError::InvalidTheme {
            path: path.to_path_buf(),
            message: "theme JSON must contain an object".to_owned(),
        })?;

    let name = object
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or("Untitled Theme")
        .to_owned();
    let kind = match object.get("type").and_then(Value::as_str) {
        Some("dark") => ThemeKind::Dark,
        Some("light") => ThemeKind::Light,
        Some("hc" | "hc-dark" | "hc-light") => ThemeKind::HighContrast,
        _ => ThemeKind::Unknown,
    };

    let mut warnings = Vec::new();
    let include = match object.get("include").and_then(Value::as_str) {
        Some(reference) => {
            let referenced = path
                .parent()
                .unwrap_or_else(|| Path::new("."))
                .join(reference);
            if referenced.exists() {
                Some(referenced)
            } else {
                warnings.push(CompatibilityWarning {
                    path: Some(path.to_path_buf()),
                    kind: CompatibilityWarningKind::UnsupportedThemeInclude {
                        reference: reference.to_owned(),
                    },
                    contribution: ContributionKind::Theme,
                    message: format!("theme include `{reference}` could not be resolved"),
                });
                None
            }
        }
        None => None,
    };

    let colors = object
        .get("colors")
        .and_then(Value::as_object)
        .map(|color_map| {
            color_map
                .iter()
                .filter_map(|(key, value)| {
                    parse_theme_color(value).map(|color| (key.clone(), color))
                })
                .collect::<BTreeMap<_, _>>()
        })
        .unwrap_or_default();

    let token_colors = match object.get("tokenColors") {
        Some(Value::Array(items)) => items
            .iter()
            .filter_map(|item| parse_token_color_rule(item, path, &mut warnings))
            .collect(),
        _ => Vec::new(),
    };

    let semantic_token_colors = object
        .get("semanticTokenColors")
        .and_then(Value::as_object)
        .map(|entries| {
            entries
                .iter()
                .filter_map(|(key, value)| {
                    parse_theme_color(value).map(|color| (key.clone(), color))
                })
                .collect::<BTreeMap<_, _>>()
        })
        .unwrap_or_default();

    Ok((
        StaticTheme {
            name,
            kind,
            include,
            colors,
            token_colors,
            semantic_token_colors,
        },
        warnings,
    ))
}

fn parse_token_color_rule(
    value: &Value,
    path: &Path,
    warnings: &mut Vec<CompatibilityWarning>,
) -> Option<TokenColorRule> {
    let object = value.as_object()?;
    let scopes = match object.get("scope") {
        Some(Value::String(scope)) => vec![scope.clone()],
        Some(Value::Array(items)) => items
            .iter()
            .filter_map(Value::as_str)
            .map(ToOwned::to_owned)
            .collect(),
        Some(_) => {
            warnings.push(CompatibilityWarning {
                path: Some(path.to_path_buf()),
                kind: CompatibilityWarningKind::UnsupportedField {
                    field: "tokenColors.scope".to_owned(),
                },
                contribution: ContributionKind::Theme,
                message: "theme token color scope must be a string or array".to_owned(),
            });
            Vec::new()
        }
        None => Vec::new(),
    };

    let settings = match object.get("settings") {
        Some(Value::Object(settings)) => ThemeStyleSpec {
            foreground: settings
                .get("foreground")
                .and_then(Value::as_str)
                .map(ToOwned::to_owned),
            background: settings
                .get("background")
                .and_then(Value::as_str)
                .map(ToOwned::to_owned),
            font_style: settings
                .get("fontStyle")
                .and_then(Value::as_str)
                .map(parse_font_style)
                .unwrap_or_default(),
        },
        Some(_) => {
            warnings.push(CompatibilityWarning {
                path: Some(path.to_path_buf()),
                kind: CompatibilityWarningKind::UnsupportedField {
                    field: "tokenColors.settings".to_owned(),
                },
                contribution: ContributionKind::Theme,
                message: "theme token color settings must be an object".to_owned(),
            });
            ThemeStyleSpec {
                foreground: None,
                background: None,
                font_style: Vec::new(),
            }
        }
        None => ThemeStyleSpec {
            foreground: None,
            background: None,
            font_style: Vec::new(),
        },
    };

    Some(TokenColorRule { scopes, settings })
}

fn parse_snippet_value(
    path: &Path,
    value: Value,
    language: Option<String>,
) -> Result<(StaticSnippetFile, Vec<CompatibilityWarning>), CompatibilityError> {
    let object = value
        .as_object()
        .ok_or_else(|| CompatibilityError::InvalidSnippetFile {
            path: path.to_path_buf(),
            message: "snippet file must contain a JSON object".to_owned(),
        })?;

    let mut snippets = Vec::new();
    let mut warnings = Vec::new();

    for (name, snippet_value) in object {
        let snippet_object =
            snippet_value
                .as_object()
                .ok_or_else(|| CompatibilityError::InvalidSnippetFile {
                    path: path.to_path_buf(),
                    message: format!("snippet `{name}` must be an object"),
                })?;

        let prefix = parse_snippet_prefix(snippet_object.get("prefix"))?;
        let body_source =
            snippet_object
                .get("body")
                .ok_or_else(|| CompatibilityError::InvalidSnippetFile {
                    path: path.to_path_buf(),
                    message: format!("snippet `{name}` is missing a `body`"),
                })?;
        let raw = render_snippet_body(body_source)?;
        let parts = parse_snippet_parts(&raw, path, &mut warnings);

        if let Some(scope) = snippet_object.get("scope") {
            if !scope.is_string() {
                warnings.push(CompatibilityWarning {
                    path: Some(path.to_path_buf()),
                    kind: CompatibilityWarningKind::UnsupportedField {
                        field: "scope".to_owned(),
                    },
                    contribution: ContributionKind::Snippet,
                    message: format!("snippet `{name}` has a non-string scope"),
                });
            }
        }

        if let Some(description) = snippet_object.get("description") {
            if !description.is_string() && !description.is_array() {
                warnings.push(CompatibilityWarning {
                    path: Some(path.to_path_buf()),
                    kind: CompatibilityWarningKind::UnsupportedField {
                        field: "description".to_owned(),
                    },
                    contribution: ContributionKind::Snippet,
                    message: format!("snippet `{name}` has an unsupported description value"),
                });
            }
        }

        snippets.push(SnippetDefinition {
            name: name.clone(),
            prefix,
            description: snippet_object
                .get("description")
                .and_then(value_to_string_lossy),
            scope: snippet_object
                .get("scope")
                .and_then(Value::as_str)
                .map(ToOwned::to_owned),
            body: SnippetBody { raw, parts },
        });
    }

    Ok((
        StaticSnippetFile {
            path: path.to_path_buf(),
            language,
            snippets,
        },
        warnings,
    ))
}

fn parse_language_configuration_value(
    path: &Path,
    value: Value,
) -> Result<(LanguageConfiguration, Vec<CompatibilityWarning>), CompatibilityError> {
    let object =
        value
            .as_object()
            .ok_or_else(|| CompatibilityError::InvalidLanguageConfiguration {
                path: path.to_path_buf(),
                message: "language configuration must contain an object".to_owned(),
            })?;

    let mut warnings = Vec::new();
    let comments = object.get("comments").and_then(|value| {
        value.as_object().map(|comment_object| LanguageComments {
            line_comment: comment_object
                .get("lineComment")
                .and_then(Value::as_str)
                .map(ToOwned::to_owned),
            block_comment: comment_object
                .get("blockComment")
                .and_then(Value::as_array)
                .and_then(|items| {
                    (items.len() == 2)
                        .then(|| {
                            Some(BracketPair {
                                open: items[0].as_str()?.to_owned(),
                                close: items[1].as_str()?.to_owned(),
                                not_in: Vec::new(),
                            })
                        })
                        .flatten()
                }),
        })
    });

    let brackets = parse_bracket_pairs(object.get("brackets"), path, &mut warnings);
    let auto_closing_pairs =
        parse_bracket_pairs(object.get("autoClosingPairs"), path, &mut warnings);
    let surrounding_pairs =
        parse_bracket_pairs(object.get("surroundingPairs"), path, &mut warnings);
    let indentation_rules = object
        .get("indentationRules")
        .and_then(Value::as_object)
        .map(|rules| IndentationRules {
            increase_indent_pattern: rules
                .get("increaseIndentPattern")
                .and_then(Value::as_str)
                .map(ToOwned::to_owned),
            decrease_indent_pattern: rules
                .get("decreaseIndentPattern")
                .and_then(Value::as_str)
                .map(ToOwned::to_owned),
            indent_next_line_pattern: rules
                .get("indentNextLinePattern")
                .and_then(Value::as_str)
                .map(ToOwned::to_owned),
            unindented_line_pattern: rules
                .get("unIndentedLinePattern")
                .and_then(Value::as_str)
                .map(ToOwned::to_owned),
        });
    let word_pattern = object
        .get("wordPattern")
        .and_then(Value::as_str)
        .map(ToOwned::to_owned);

    let folding_markers = object.get("folding").and_then(|value| {
        let folding = value.as_object()?;
        Some(FoldingMarkers {
            start: folding
                .get("markers")
                .and_then(Value::as_object)
                .and_then(|markers| markers.get("start"))
                .and_then(Value::as_str)
                .map(ToOwned::to_owned),
            end: folding
                .get("markers")
                .and_then(Value::as_object)
                .and_then(|markers| markers.get("end"))
                .and_then(Value::as_str)
                .map(ToOwned::to_owned),
        })
    });

    let on_enter_rules = object
        .get("onEnterRules")
        .and_then(Value::as_array)
        .map(|rules| {
            rules
                .iter()
                .filter_map(|rule| {
                    let rule = rule.as_object()?;
                    Some(OnEnterRule {
                        before_text: rule
                            .get("beforeText")
                            .and_then(Value::as_str)
                            .map(ToOwned::to_owned),
                        after_text: rule
                            .get("afterText")
                            .and_then(Value::as_str)
                            .map(ToOwned::to_owned),
                        previous_line_text: rule
                            .get("previousLineText")
                            .and_then(Value::as_str)
                            .map(ToOwned::to_owned),
                        action: rule.get("action").and_then(Value::as_object).map(|action| {
                            OnEnterAction {
                                indent_action: action
                                    .get("indentAction")
                                    .and_then(Value::as_str)
                                    .map(ToOwned::to_owned),
                                append_text: action
                                    .get("appendText")
                                    .and_then(Value::as_str)
                                    .map(ToOwned::to_owned),
                                remove_text: action
                                    .get("removeText")
                                    .and_then(Value::as_str)
                                    .map(ToOwned::to_owned),
                            }
                        }),
                    })
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    for unsupported in object.keys().filter(|key| {
        !matches!(
            key.as_str(),
            "comments"
                | "brackets"
                | "autoClosingPairs"
                | "surroundingPairs"
                | "indentationRules"
                | "wordPattern"
                | "onEnterRules"
                | "folding"
        )
    }) {
        warnings.push(CompatibilityWarning {
            path: Some(path.to_path_buf()),
            kind: CompatibilityWarningKind::UnsupportedLanguageConfigurationField {
                field: unsupported.clone(),
            },
            contribution: ContributionKind::LanguageConfiguration,
            message: format!("unsupported language configuration field `{unsupported}`"),
        });
    }

    Ok((
        LanguageConfiguration {
            comments,
            brackets,
            auto_closing_pairs,
            surrounding_pairs,
            indentation_rules,
            word_pattern,
            on_enter_rules,
            folding_markers,
        },
        warnings,
    ))
}

fn parse_bracket_pairs(
    value: Option<&Value>,
    path: &Path,
    warnings: &mut Vec<CompatibilityWarning>,
) -> Vec<BracketPair> {
    match value {
        Some(Value::Array(items)) => items
            .iter()
            .filter_map(|item| match item {
                Value::Array(pair) if pair.len() == 2 => Some(BracketPair {
                    open: pair[0].as_str()?.to_owned(),
                    close: pair[1].as_str()?.to_owned(),
                    not_in: Vec::new(),
                }),
                Value::Object(object) => {
                    let open = object.get("open").and_then(Value::as_str)?.to_owned();
                    let close = object.get("close").and_then(Value::as_str)?.to_owned();
                    let not_in = object
                        .get("notIn")
                        .and_then(Value::as_array)
                        .map(|items| {
                            items
                                .iter()
                                .filter_map(Value::as_str)
                                .map(ToOwned::to_owned)
                                .collect::<Vec<_>>()
                        })
                        .unwrap_or_default();
                    Some(BracketPair {
                        open,
                        close,
                        not_in,
                    })
                }
                _ => {
                    warnings.push(CompatibilityWarning {
                        path: Some(path.to_path_buf()),
                        kind: CompatibilityWarningKind::UnsupportedField {
                            field: "pair".to_owned(),
                        },
                        contribution: ContributionKind::LanguageConfiguration,
                        message: "language configuration pair entry is unsupported".to_owned(),
                    });
                    None
                }
            })
            .collect(),
        Some(_) => {
            warnings.push(CompatibilityWarning {
                path: Some(path.to_path_buf()),
                kind: CompatibilityWarningKind::UnsupportedField {
                    field: "pairs".to_owned(),
                },
                contribution: ContributionKind::LanguageConfiguration,
                message: "language configuration pairs must be arrays".to_owned(),
            });
            Vec::new()
        }
        None => Vec::new(),
    }
}

fn parse_theme_color(value: &Value) -> Option<ThemeColor> {
    match value {
        Value::String(color) => Some(ThemeColor::Hex(color.clone())),
        Value::Object(object) => Some(ThemeColor::Style(ThemeStyleSpec {
            foreground: object
                .get("foreground")
                .and_then(Value::as_str)
                .map(ToOwned::to_owned),
            background: object
                .get("background")
                .and_then(Value::as_str)
                .map(ToOwned::to_owned),
            font_style: object
                .get("fontStyle")
                .and_then(Value::as_str)
                .map(parse_font_style)
                .unwrap_or_default(),
        })),
        _ => None,
    }
}

fn parse_font_style(input: &str) -> Vec<String> {
    input
        .split_whitespace()
        .filter(|part| !part.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

fn map_keybinding_command(command: &str) -> Option<CommandId> {
    let mapped = match command {
        "workbench.action.files.save" | "editor.action.save" => "editor.save",
        "workbench.action.files.saveAll" => "editor.saveAll",
        "undo" | "workbench.action.undo" => "editor.undo",
        "redo" | "workbench.action.redo" => "editor.redo",
        "actions.find" | "editor.actions.find" => "editor.find",
        "editor.action.startFindReplaceAction" => "editor.replace",
        "workbench.action.files.newUntitledFile" => "editor.newFile",
        "workbench.action.closeActiveEditor" => "editor.closeFile",
        "workbench.action.nextEditor" => "editor.nextTab",
        "workbench.action.previousEditor" => "editor.previousTab",
        "editor.action.clipboardCopyAction" => "editor.copy",
        "editor.action.clipboardCutAction" => "editor.cut",
        "editor.action.clipboardPasteAction" => "editor.paste",
        "editor.action.selectAll" => "editor.selectAll",
        "editor.action.commentLine" => "editor.toggleComment",
        "editor.action.formatDocument" => "editor.formatDocument",
        "editor.action.formatSelection" => "editor.formatSelection",
        "workbench.action.showCommands" => "editor.commandPalette",
        "workbench.action.quickOpen" => "editor.quickOpen",
        _ => return None,
    };
    Some(CommandId::new(mapped))
}

fn parse_word_wrap(value: &Value) -> Option<WordWrap> {
    let text = value.as_str()?;
    match text {
        "off" => Some(WordWrap::Off),
        "on" => Some(WordWrap::On),
        "wordWrapColumn" => Some(WordWrap::WordWrapColumn),
        "bounded" => Some(WordWrap::Bounded),
        _ => None,
    }
}

fn parse_auto_closing_pairs(value: &Value) -> Option<AutoClosingPairs> {
    match value {
        Value::Bool(false) => Some(AutoClosingPairs::Never),
        Value::Bool(true) => Some(AutoClosingPairs::LanguageDefined),
        Value::String(text) => match text.as_str() {
            "never" => Some(AutoClosingPairs::Never),
            "languageDefined" => Some(AutoClosingPairs::LanguageDefined),
            "beforeWhitespace" => Some(AutoClosingPairs::BeforeWhitespace),
            "always" => Some(AutoClosingPairs::Always),
            _ => None,
        },
        _ => None,
    }
}

fn parse_glob_object(value: &Value) -> Option<Vec<String>> {
    let object = value.as_object()?;
    let mut globs = Vec::new();
    for (glob, enabled) in object {
        if enabled.as_bool() == Some(true) {
            globs.push(glob.clone());
        }
    }
    Some(globs)
}

fn parse_line_ending(value: &Value) -> Option<LineEndingPreference> {
    match value.as_str()? {
        "lf" => Some(LineEndingPreference::Lf),
        "crlf" => Some(LineEndingPreference::Crlf),
        "auto" => Some(LineEndingPreference::Preserve),
        _ => None,
    }
}

fn parse_snippet_prefix(value: Option<&Value>) -> Result<Vec<String>, CompatibilityError> {
    match value {
        Some(Value::String(prefix)) => Ok(vec![prefix.clone()]),
        Some(Value::Array(items)) => Ok(items
            .iter()
            .filter_map(Value::as_str)
            .map(ToOwned::to_owned)
            .collect()),
        Some(_) => Err(CompatibilityError::InvalidSnippetFile {
            path: PathBuf::from("<snippet>"),
            message: "snippet prefix must be a string or array".to_owned(),
        }),
        None => Ok(Vec::new()),
    }
}

fn render_snippet_body(value: &Value) -> Result<String, CompatibilityError> {
    match value {
        Value::String(text) => Ok(text.clone()),
        Value::Array(lines) => {
            let mut rendered = Vec::new();
            for line in lines {
                let Some(text) = line.as_str() else {
                    return Err(CompatibilityError::InvalidSnippetFile {
                        path: PathBuf::from("<snippet>"),
                        message: "snippet body array entries must be strings".to_owned(),
                    });
                };
                rendered.push(text);
            }
            Ok(rendered.join("\n"))
        }
        _ => Err(CompatibilityError::InvalidSnippetFile {
            path: PathBuf::from("<snippet>"),
            message: "snippet body must be a string or array of strings".to_owned(),
        }),
    }
}

fn parse_snippet_parts(
    input: &str,
    path: &Path,
    warnings: &mut Vec<CompatibilityWarning>,
) -> Vec<SnippetPart> {
    let mut parser = SnippetParser {
        input,
        cursor: 0,
        path: path.to_path_buf(),
        warnings,
    };
    parser.parse_until(None)
}

struct SnippetParser<'a> {
    input: &'a str,
    cursor: usize,
    path: PathBuf,
    warnings: &'a mut Vec<CompatibilityWarning>,
}

impl<'a> SnippetParser<'a> {
    fn parse_until(&mut self, terminator: Option<char>) -> Vec<SnippetPart> {
        let mut parts = Vec::new();
        let mut buffer = String::new();

        while let Some(ch) = self.peek() {
            if Some(ch) == terminator {
                self.bump();
                break;
            }

            if ch == '$' {
                self.bump();
                match self.peek() {
                    Some('$') => {
                        self.bump();
                        buffer.push('$');
                    }
                    Some('{') => {
                        self.bump();
                        if !buffer.is_empty() {
                            parts.push(SnippetPart::Text(std::mem::take(&mut buffer)));
                        }
                        parts.push(self.parse_placeholder());
                    }
                    Some(digit) if digit.is_ascii_digit() => {
                        if !buffer.is_empty() {
                            parts.push(SnippetPart::Text(std::mem::take(&mut buffer)));
                        }
                        parts.push(SnippetPart::TabStop(self.parse_number()));
                    }
                    Some(letter) if letter.is_ascii_alphabetic() || letter == '_' => {
                        if !buffer.is_empty() {
                            parts.push(SnippetPart::Text(std::mem::take(&mut buffer)));
                        }
                        parts.push(self.parse_variable());
                    }
                    _ => buffer.push('$'),
                }
                continue;
            }

            buffer.push(ch);
            self.bump();
        }

        if !buffer.is_empty() {
            parts.push(SnippetPart::Text(buffer));
        }
        parts
    }

    fn parse_placeholder(&mut self) -> SnippetPart {
        let number = self.parse_number();
        if self.peek() == Some(':') {
            self.bump();
            let default = self.parse_until(Some('}'));
            return SnippetPart::Placeholder {
                index: number,
                default,
            };
        }

        if self.peek() == Some('|') {
            self.bump();
            let mut choice = String::new();
            while let Some(ch) = self.peek() {
                self.bump();
                if ch == '|' && self.peek() == Some('}') {
                    self.bump();
                    break;
                }
                choice.push(ch);
            }
            return SnippetPart::Placeholder {
                index: number,
                default: vec![SnippetPart::Text(choice)],
            };
        }

        if self.peek() == Some('/') {
            let expression = self.collect_until('}');
            self.warnings.push(CompatibilityWarning {
                path: Some(self.path.clone()),
                kind: CompatibilityWarningKind::UnsupportedSnippetTransform {
                    expression: expression.clone(),
                },
                contribution: ContributionKind::Snippet,
                message: format!(
                    "snippet placeholder transform `${{{number}/{expression}}}` is ignored"
                ),
            });
            return SnippetPart::Placeholder {
                index: number,
                default: Vec::new(),
            };
        }

        self.expect('}');
        SnippetPart::Placeholder {
            index: number,
            default: Vec::new(),
        }
    }

    fn parse_variable(&mut self) -> SnippetPart {
        let name = self.parse_identifier();
        if self.peek() == Some(':') {
            self.bump();
            let default = self.parse_until(Some('}'));
            return SnippetPart::Variable { name, default };
        }

        if self.peek() == Some('/') {
            let expression = self.collect_until('}');
            self.warnings.push(CompatibilityWarning {
                path: Some(self.path.clone()),
                kind: CompatibilityWarningKind::UnsupportedSnippetTransform {
                    expression: expression.clone(),
                },
                contribution: ContributionKind::Snippet,
                message: format!(
                    "snippet variable transform `${{{name}/{expression}}}` is ignored"
                ),
            });
            return SnippetPart::Variable {
                name,
                default: Vec::new(),
            };
        }

        self.expect('}');
        SnippetPart::Variable {
            name,
            default: Vec::new(),
        }
    }

    fn parse_number(&mut self) -> u32 {
        let mut value = String::new();
        while let Some(ch) = self.peek() {
            if ch.is_ascii_digit() {
                value.push(ch);
                self.bump();
            } else {
                break;
            }
        }
        value.parse().unwrap_or(0)
    }

    fn parse_identifier(&mut self) -> String {
        let mut value = String::new();
        while let Some(ch) = self.peek() {
            if ch.is_ascii_alphanumeric() || ch == '_' {
                value.push(ch);
                self.bump();
            } else {
                break;
            }
        }
        value
    }

    fn collect_until(&mut self, terminator: char) -> String {
        let mut value = String::new();
        while let Some(ch) = self.peek() {
            self.bump();
            if ch == terminator {
                break;
            }
            value.push(ch);
        }
        value
    }

    fn expect(&mut self, terminator: char) {
        if self.peek() == Some(terminator) {
            self.bump();
        }
    }

    fn peek(&self) -> Option<char> {
        self.input[self.cursor..].chars().next()
    }

    fn bump(&mut self) {
        if let Some(ch) = self.peek() {
            self.cursor += ch.len_utf8();
        }
    }
}

fn manifest_warnings(manifest: &ExtensionManifest, path: &Path) -> Vec<CompatibilityWarning> {
    let mut warnings = Vec::new();
    if manifest.main.is_some() {
        warnings.push(CompatibilityWarning {
            path: Some(path.to_path_buf()),
            kind: CompatibilityWarningKind::UnsupportedExecutableCode {
                name: "main".to_owned(),
            },
            contribution: ContributionKind::Unsupported,
            message: "extension JavaScript/TypeScript entrypoints are not executed".to_owned(),
        });
    }
    if manifest.browser.is_some() {
        warnings.push(CompatibilityWarning {
            path: Some(path.to_path_buf()),
            kind: CompatibilityWarningKind::UnsupportedExecutableCode {
                name: "browser".to_owned(),
            },
            contribution: ContributionKind::Unsupported,
            message: "browser extension entrypoints are not executed".to_owned(),
        });
    }
    if manifest.activation_events.is_some() {
        warnings.push(CompatibilityWarning {
            path: Some(path.to_path_buf()),
            kind: CompatibilityWarningKind::UnsupportedExecutableCode {
                name: "activationEvents".to_owned(),
            },
            contribution: ContributionKind::Unsupported,
            message:
                "activation events are ignored because executable extension code is not loaded"
                    .to_owned(),
        });
    }
    for field in manifest.unknown.keys() {
        warnings.push(CompatibilityWarning {
            path: Some(path.to_path_buf()),
            kind: CompatibilityWarningKind::UnsupportedField {
                field: field.clone(),
            },
            contribution: ContributionKind::Unsupported,
            message: format!("unsupported manifest field `{field}`"),
        });
    }
    warnings
}

fn resolve_within_root(root: &Path, reference: &str) -> Result<PathBuf, String> {
    let relative = Path::new(reference);
    if relative.is_absolute() {
        return Err(format!(
            "reference `{reference}` must be relative to {}",
            root.display()
        ));
    }

    let mut resolved = root.to_path_buf();
    let base_depth = resolved.components().count();
    for component in relative.components() {
        match component {
            Component::CurDir => {}
            Component::Normal(part) => resolved.push(part),
            Component::ParentDir => {
                if resolved.components().count() <= base_depth || !resolved.pop() {
                    return Err(format!(
                        "reference `{reference}` escapes {}",
                        root.display()
                    ));
                }
            }
            Component::Prefix(_) | Component::RootDir => {
                return Err(format!(
                    "reference `{reference}` must remain relative to {}",
                    root.display()
                ));
            }
        }
    }
    Ok(resolved)
}

fn sanitize_zip_entry(entry: &Path) -> Result<PathBuf, String> {
    let mut sanitized = PathBuf::new();
    for component in entry.components() {
        match component {
            Component::CurDir => {}
            Component::Normal(part) => sanitized.push(part),
            Component::ParentDir => {
                if !sanitized.pop() {
                    return Err(format!(
                        "zip entry `{}` escapes the destination",
                        entry.display()
                    ));
                }
            }
            Component::Prefix(_) | Component::RootDir => {
                return Err(format!("zip entry `{}` is absolute", entry.display()));
            }
        }
    }
    if sanitized.as_os_str().is_empty() {
        return Err(format!(
            "zip entry `{}` does not resolve to a file name",
            entry.display()
        ));
    }
    Ok(sanitized)
}

fn parse_jsonc_value(input: &str, path: &Path) -> Result<Value, CompatibilityError> {
    let without_comments = strip_jsonc_comments(input);
    let normalized = strip_trailing_commas(&without_comments);
    serde_json::from_str(&normalized).map_err(|source| CompatibilityError::InvalidJson {
        path: path.to_path_buf(),
        source,
    })
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

fn read_to_string(path: &Path) -> Result<String, CompatibilityError> {
    fs::read_to_string(path).map_err(|source| CompatibilityError::Read {
        path: path.to_path_buf(),
        source,
    })
}

fn push_setting_warning(warnings: &mut Vec<CompatibilityWarning>, key: &str, detail: &str) {
    warnings.push(CompatibilityWarning {
        path: None,
        kind: CompatibilityWarningKind::UnsupportedSetting {
            key: key.to_owned(),
        },
        contribution: ContributionKind::Settings,
        message: format!("setting `{key}` {detail}"),
    });
}

fn value_to_string_lossy(value: &Value) -> Option<String> {
    match value {
        Value::String(text) => Some(text.clone()),
        Value::Array(parts) => Some(
            parts
                .iter()
                .filter_map(Value::as_str)
                .collect::<Vec<_>>()
                .join(", "),
        ),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixtures() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("tests")
            .join("fixtures")
            .join("vscode-compat")
    }

    #[test]
    fn settings_jsonc_maps_supported_keys_and_warns_on_unknown_entries() {
        let parsed = load_vscode_settings_jsonc(
            r#"{
                // comment
                "editor.lineNumbers": false,
                "editor.tabSize": 2,
                "editor.insertSpaces": true,
                "editor.wordWrap": "bounded",
                "editor.autoClosingPairs": "always",
                "editor.formatOnSave": true,
                "editor.formatOnPaste": true,
                "workbench.colorTheme": "Editor Dark",
                "search.exclude": { "**/target/**": true, "**/*.tmp": false },
                "files.exclude": { "**/*.bak": true },
                "files.encoding": "utf-8",
                "files.eol": "crlf",
                "editor.largeFileThreshold": 1234,
                "future.setting": 1,
            }"#,
        )
        .expect("settings parse");

        assert!(!parsed.layer.line_numbers.expect("line numbers"));
        assert_eq!(parsed.layer.tab_size, Some(2));
        assert_eq!(parsed.layer.insert_spaces, Some(true));
        assert_eq!(parsed.layer.word_wrap, Some(WordWrap::Bounded));
        assert_eq!(
            parsed.layer.auto_closing_pairs,
            Some(AutoClosingPairs::Always)
        );
        assert_eq!(parsed.layer.theme.as_deref(), Some("Editor Dark"));
        assert_eq!(
            parsed.layer.search_excludes,
            Some(vec!["**/target/**".to_owned()])
        );
        assert_eq!(
            parsed.layer.files_excludes,
            Some(vec!["**/*.bak".to_owned()])
        );
        assert_eq!(parsed.layer.encoding_fallback.as_deref(), Some("utf-8"));
        assert_eq!(parsed.layer.line_ending, Some(LineEndingPreference::Crlf));
        assert_eq!(parsed.layer.large_file_threshold, Some(1234));
        assert_eq!(parsed.warnings.len(), 1);
    }

    #[test]
    fn keybindings_map_supported_commands_and_surface_unknown_commands() {
        let parsed = load_vscode_keybindings_jsonc(
            r#"[
                { "key": "ctrl+s", "command": "workbench.action.files.save" },
                { "key": "ctrl+shift+p", "command": "workbench.action.showCommands", "when": "editorTextFocus" },
                { "key": "ctrl+f13", "command": "not.supported" }
            ]"#,
        )
        .expect("keybindings parse");

        assert_eq!(parsed.keybindings.len(), 2);
        assert_eq!(parsed.keybindings[0].command, "editor.save");
        assert_eq!(parsed.keybindings[1].command, "editor.commandPalette");
        assert_eq!(
            parsed.keybindings[1].when.as_deref(),
            Some("editorTextFocus")
        );
        assert_eq!(parsed.warnings.len(), 1);
    }

    #[test]
    fn snippet_parser_keeps_tabstops_and_placeholders() {
        let (snippet_file, warnings) = load_snippet_json(
            Path::new("fixture.code-snippets"),
            r#"{
                "For Loop": {
                    "prefix": ["for", "loop"],
                    "body": "for (let ${1:item} of ${2:items}) {\n  $0\n}",
                    "description": "loop snippet"
                }
            }"#,
        )
        .expect("snippet parse");

        assert!(warnings.is_empty());
        assert_eq!(snippet_file.language, None);
        assert_eq!(snippet_file.snippets.len(), 1);
        let body = &snippet_file.snippets[0].body.parts;
        assert!(contains_placeholder(body, 1));
        assert!(contains_placeholder(body, 2));
        assert!(contains_tabstop(body, 0));
    }

    #[test]
    fn language_configuration_parses_pairs_comments_and_rules() {
        let (configuration, warnings) = load_language_configuration_json(
            Path::new("fixture.language-configuration.json"),
            r#"{
                "comments": { "lineComment": "//", "blockComment": ["/*", "*/"] },
                "brackets": [["{", "}"], ["[", "]"]],
                "autoClosingPairs": [
                    { "open": "{", "close": "}" },
                    ["(", ")"]
                ],
                "surroundingPairs": [
                    { "open": "(", "close": ")" }
                ],
                "indentationRules": {
                    "increaseIndentPattern": "^.*\\{[^}\"']*$",
                    "decreaseIndentPattern": "^\\s*\\}",
                    "indentNextLinePattern": "^.*:\\s*$",
                    "unIndentedLinePattern": "^\\s*$"
                },
                "wordPattern": "[A-Za-z_][A-Za-z0-9_]*",
                "onEnterRules": [
                    {
                        "beforeText": "^\\s*\\{\\s*$",
                        "action": { "indentAction": "indent" }
                    }
                ],
                "folding": { "markers": { "start": "^\\s*// #region", "end": "^\\s*// #endregion" } }
            }"#,
        )
        .expect("language configuration parse");

        assert!(warnings.is_empty());
        assert_eq!(
            configuration
                .comments
                .expect("comments")
                .line_comment
                .as_deref(),
            Some("//")
        );
        assert_eq!(configuration.brackets.len(), 2);
        assert_eq!(configuration.auto_closing_pairs.len(), 2);
        assert_eq!(configuration.surrounding_pairs.len(), 1);
        assert_eq!(
            configuration.word_pattern.as_deref(),
            Some("[A-Za-z_][A-Za-z0-9_]*")
        );
        assert_eq!(configuration.on_enter_rules.len(), 1);
    }

    #[test]
    fn fixture_extension_directory_loads_static_contributions() {
        let package =
            load_static_extension_directory(fixtures().join("extension")).expect("load extension");

        assert_eq!(
            package.manifest.name.as_deref(),
            Some("static-compat-fixture")
        );
        assert_eq!(package.themes.len(), 1);
        assert_eq!(package.snippets.len(), 1);
        assert_eq!(package.languages.len(), 1);
        assert!(package.warnings.iter().any(|warning| matches!(
            warning.kind,
            CompatibilityWarningKind::UnsupportedExecutableCode { .. }
        )));
        assert!(package.warnings.iter().any(|warning| matches!(
            warning.kind,
            CompatibilityWarningKind::UnsupportedContribution { .. }
        )));
        assert!(package.languages[0].configuration.is_some());
        assert!(package.themes[0].colors.contains_key("editor.foreground"));
    }

    #[test]
    fn vsix_extraction_rejects_traversal() {
        let archive = fixtures().join("traversal.vsix");
        let destination = tempfile::tempdir().expect("destination");
        let error = extract_vsix(&archive, destination.path()).expect_err("reject traversal");
        assert!(matches!(error, CompatibilityError::InvalidVsixEntry { .. }));
        assert!(
            destination
                .path()
                .read_dir()
                .expect("destination readable")
                .next()
                .is_none()
        );
    }

    #[test]
    fn fixture_vsix_loads_through_extraction() {
        let archive = fixtures().join("static-extension.vsix");
        let destination = tempfile::tempdir().expect("destination");
        let package = load_static_extension_vsix(&archive, destination.path()).expect("load vsix");
        assert_eq!(package.themes.len(), 1);
        assert_eq!(package.snippets.len(), 1);
        assert_eq!(package.languages.len(), 1);
    }

    #[test]
    fn vsix_archive_stem_cannot_escape_cache_directory() {
        let source = fixtures().join("static-extension.vsix");
        let archive_dir = tempfile::tempdir().expect("archive directory");
        let archive = archive_dir.path().join("..vsix");
        std::fs::copy(source, &archive).expect("copy archive");
        let destination = tempfile::tempdir().expect("destination");
        let extracted = extract_vsix(&archive, destination.path()).expect("extract archive");
        assert!(extracted.starts_with(destination.path()));
        assert!(extracted.file_name().is_some_and(|name| name != ".."));
    }

    #[test]
    fn malformed_manifest_is_reported_as_typed_error() {
        let error = load_static_extension_directory(fixtures().join("malformed-extension"))
            .expect_err("malformed manifest should fail");
        assert!(matches!(error, CompatibilityError::InvalidManifest { .. }));
    }

    fn contains_tabstop(parts: &[SnippetPart], target: u32) -> bool {
        parts.iter().any(|part| match part {
            SnippetPart::TabStop(index) => *index == target,
            SnippetPart::Placeholder { default, .. } | SnippetPart::Variable { default, .. } => {
                contains_tabstop(default, target)
            }
            SnippetPart::Text(_) => false,
        })
    }

    fn contains_placeholder(parts: &[SnippetPart], target: u32) -> bool {
        parts.iter().any(|part| match part {
            SnippetPart::Placeholder { index, default } => {
                *index == target || contains_placeholder(default, target)
            }
            SnippetPart::Variable { default, .. } => contains_placeholder(default, target),
            SnippetPart::TabStop(_) | SnippetPart::Text(_) => false,
        })
    }
}
