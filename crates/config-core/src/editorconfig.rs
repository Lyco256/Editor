//! Side-effect-free `EditorConfig` discovery, precedence, and property mapping.

use std::path::{Path, PathBuf};

use thiserror::Error;

use crate::encoding::{EncodingKind, LineEndings};

/// Document formatting settings contributed by applicable `EditorConfig` sections.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DocumentSettings {
    pub indent_style: Option<IndentStyle>,
    pub indent_size: Option<IndentSize>,
    pub tab_width: Option<u8>,
    pub end_of_line: Option<LineEndings>,
    pub charset: Option<EncodingKind>,
    pub trim_trailing_whitespace: Option<bool>,
    pub insert_final_newline: Option<bool>,
    pub warnings: Vec<EditorConfigWarning>,
}

/// Indentation character policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IndentStyle {
    Tab,
    Space,
}

/// Indentation width, including `EditorConfig`'s `tab` sentinel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IndentSize {
    Width(u8),
    Tab,
}

/// A non-fatal invalid property with source context.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditorConfigWarning {
    pub path: PathBuf,
    pub line: usize,
    pub property: String,
    pub value: String,
}

/// Typed `EditorConfig` I/O failures.
#[derive(Debug, Error)]
pub enum EditorConfigError {
    #[error("could not read EditorConfig at {path}: {source}")]
    Read {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}

#[derive(Debug)]
struct ConfigFile {
    path: PathBuf,
    root: bool,
    sections: Vec<Section>,
}

#[derive(Debug)]
struct Section {
    pattern: String,
    properties: Vec<(usize, String, String)>,
}

/// Walks from the document directory upward and applies matching files farthest-to-nearest.
///
/// # Errors
///
/// Returns [`EditorConfigError::Read`] when an encountered `.editorconfig` file cannot be read.
pub fn load_editorconfig(document: &Path) -> Result<DocumentSettings, EditorConfigError> {
    let mut discovered = Vec::new();
    let mut directory = document.parent();
    while let Some(current) = directory {
        let candidate = current.join(".editorconfig");
        match std::fs::read_to_string(&candidate) {
            Ok(contents) => {
                let parsed = parse_file(candidate, &contents);
                let is_root = parsed.root;
                discovered.push(parsed);
                if is_root {
                    break;
                }
            }
            Err(source) if source.kind() == std::io::ErrorKind::NotFound => {}
            Err(source) => {
                return Err(EditorConfigError::Read {
                    path: candidate,
                    source,
                });
            }
        }
        directory = current.parent();
    }

    let mut settings = DocumentSettings::default();
    for config in discovered.iter().rev() {
        let base = config.path.parent().unwrap_or_else(|| Path::new(""));
        let relative = document.strip_prefix(base).unwrap_or(document);
        let relative = relative.to_string_lossy().replace('\\', "/");
        let file_name = document
            .file_name()
            .map_or_else(String::new, |name| name.to_string_lossy().into_owned());
        for section in &config.sections {
            if section_matches(&section.pattern, &relative, &file_name) {
                for (line, key, value) in &section.properties {
                    settings.apply(&config.path, *line, key, value);
                }
            }
        }
    }
    Ok(settings)
}

fn parse_file(path: PathBuf, input: &str) -> ConfigFile {
    let mut root = false;
    let mut sections: Vec<Section> = Vec::new();
    let mut current = None;
    for (index, raw_line) in input.lines().enumerate() {
        let line_number = index + 1;
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
            continue;
        }
        if let Some(pattern) = line
            .strip_prefix('[')
            .and_then(|value| value.strip_suffix(']'))
        {
            sections.push(Section {
                pattern: pattern.trim().to_owned(),
                properties: Vec::new(),
            });
            current = Some(sections.len() - 1);
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim().to_ascii_lowercase();
        let value = value.trim().to_owned();
        if let Some(section_index) = current {
            sections[section_index]
                .properties
                .push((line_number, key, value));
        } else if key == "root" {
            root = value.eq_ignore_ascii_case("true");
        }
    }
    ConfigFile {
        path,
        root,
        sections,
    }
}

impl DocumentSettings {
    fn apply(&mut self, path: &Path, line: usize, key: &str, value: &str) {
        if value.eq_ignore_ascii_case("unset") {
            self.unset(key);
            return;
        }
        let valid = match key {
            "indent_style" => {
                parse_indent_style(value).map(|parsed| self.indent_style = Some(parsed))
            }
            "indent_size" => parse_indent_size(value).map(|parsed| self.indent_size = Some(parsed)),
            "tab_width" => parse_width(value).map(|parsed| self.tab_width = Some(parsed)),
            "end_of_line" => parse_eol(value).map(|parsed| self.end_of_line = Some(parsed)),
            "charset" => parse_charset(value).map(|parsed| self.charset = Some(parsed)),
            "trim_trailing_whitespace" => {
                parse_bool(value).map(|parsed| self.trim_trailing_whitespace = Some(parsed))
            }
            "insert_final_newline" => {
                parse_bool(value).map(|parsed| self.insert_final_newline = Some(parsed))
            }
            _ => return,
        };
        if valid.is_none() {
            self.warnings.push(EditorConfigWarning {
                path: path.to_path_buf(),
                line,
                property: key.to_owned(),
                value: value.to_owned(),
            });
        }
    }

    fn unset(&mut self, key: &str) {
        match key {
            "indent_style" => self.indent_style = None,
            "indent_size" => self.indent_size = None,
            "tab_width" => self.tab_width = None,
            "end_of_line" => self.end_of_line = None,
            "charset" => self.charset = None,
            "trim_trailing_whitespace" => self.trim_trailing_whitespace = None,
            "insert_final_newline" => self.insert_final_newline = None,
            _ => {}
        }
    }
}

fn parse_indent_style(value: &str) -> Option<IndentStyle> {
    match value.to_ascii_lowercase().as_str() {
        "tab" => Some(IndentStyle::Tab),
        "space" => Some(IndentStyle::Space),
        _ => None,
    }
}

fn parse_indent_size(value: &str) -> Option<IndentSize> {
    if value.eq_ignore_ascii_case("tab") {
        Some(IndentSize::Tab)
    } else {
        parse_width(value).map(IndentSize::Width)
    }
}

fn parse_width(value: &str) -> Option<u8> {
    value.parse::<u8>().ok().filter(|width| *width > 0)
}

fn parse_eol(value: &str) -> Option<LineEndings> {
    match value.to_ascii_lowercase().as_str() {
        "lf" => Some(LineEndings::Lf),
        "crlf" => Some(LineEndings::Crlf),
        _ => None,
    }
}

fn parse_charset(value: &str) -> Option<EncodingKind> {
    let normalized = value.to_ascii_lowercase();
    let label = match normalized.as_str() {
        "utf-8-bom" => "utf-8",
        "latin1" => "windows-1252",
        other => other,
    };
    EncodingKind::for_label(label).ok()
}

fn parse_bool(value: &str) -> Option<bool> {
    match value.to_ascii_lowercase().as_str() {
        "true" => Some(true),
        "false" => Some(false),
        _ => None,
    }
}

fn section_matches(pattern: &str, relative: &str, file_name: &str) -> bool {
    expand_braces(pattern).iter().any(|expanded| {
        let candidate = if expanded.contains('/') {
            relative
        } else {
            file_name
        };
        wildcard_match(expanded.as_bytes(), candidate.as_bytes())
    })
}

fn expand_braces(pattern: &str) -> Vec<String> {
    let Some(open) = pattern.find('{') else {
        return vec![pattern.to_owned()];
    };
    let Some(relative_close) = pattern[open + 1..].find('}') else {
        return vec![pattern.to_owned()];
    };
    let close = open + 1 + relative_close;
    pattern[open + 1..close]
        .split(',')
        .map(|alternative| {
            format!(
                "{}{}{}",
                &pattern[..open],
                alternative,
                &pattern[close + 1..]
            )
        })
        .collect()
}

fn wildcard_match(pattern: &[u8], candidate: &[u8]) -> bool {
    let mut memo = vec![vec![None; candidate.len() + 1]; pattern.len() + 1];
    wildcard_match_at(pattern, candidate, 0, 0, &mut memo)
}

fn wildcard_match_at(
    pattern: &[u8],
    candidate: &[u8],
    pattern_index: usize,
    candidate_index: usize,
    memo: &mut [Vec<Option<bool>>],
) -> bool {
    if let Some(result) = memo[pattern_index][candidate_index] {
        return result;
    }
    let result = if pattern_index == pattern.len() {
        candidate_index == candidate.len()
    } else if pattern[pattern_index] == b'*' {
        let double = pattern.get(pattern_index + 1) == Some(&b'*');
        let next_pattern = pattern_index + if double { 2 } else { 1 };
        wildcard_match_at(pattern, candidate, next_pattern, candidate_index, memo)
            || (candidate_index < candidate.len()
                && (double || candidate[candidate_index] != b'/')
                && wildcard_match_at(pattern, candidate, pattern_index, candidate_index + 1, memo))
    } else if candidate_index < candidate.len()
        && (pattern[pattern_index] == b'?' || pattern[pattern_index] == candidate[candidate_index])
    {
        wildcard_match_at(
            pattern,
            candidate,
            pattern_index + 1,
            candidate_index + 1,
            memo,
        )
    } else {
        false
    };
    memo[pattern_index][candidate_index] = Some(result);
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn traversal_stops_at_root_and_nearest_file_wins() {
        let temp = tempfile::tempdir().expect("temp directory");
        let project = temp.path().join("project");
        let source = project.join("src");
        std::fs::create_dir_all(&source).expect("directories");
        std::fs::write(
            project.join(".editorconfig"),
            "root = true\n[*]\nindent_size = 2\n",
        )
        .expect("root config");
        std::fs::write(source.join(".editorconfig"), "[*.rs]\nindent_size = 4\n")
            .expect("near config");
        let settings = load_editorconfig(&source.join("main.rs")).expect("load config");
        assert_eq!(settings.indent_size, Some(IndentSize::Width(4)));
    }

    #[test]
    fn all_required_properties_are_mapped() {
        let temp = tempfile::tempdir().expect("temp directory");
        std::fs::write(
            temp.path().join(".editorconfig"),
            "root=true\n[*.rs]\nindent_style=space\nindent_size=2\ntab_width=8\nend_of_line=crlf\ncharset=utf-16be\ntrim_trailing_whitespace=true\ninsert_final_newline=false\n",
        )
        .expect("config");
        let settings = load_editorconfig(&temp.path().join("lib.rs")).expect("load config");
        assert_eq!(settings.indent_style, Some(IndentStyle::Space));
        assert_eq!(settings.indent_size, Some(IndentSize::Width(2)));
        assert_eq!(settings.tab_width, Some(8));
        assert_eq!(settings.end_of_line, Some(LineEndings::Crlf));
        assert_eq!(settings.charset, Some(EncodingKind::Utf16Be));
        assert_eq!(settings.trim_trailing_whitespace, Some(true));
        assert_eq!(settings.insert_final_newline, Some(false));
    }

    #[test]
    fn path_and_brace_patterns_match() {
        assert!(section_matches(
            "src/**/{*.rs,*.toml}",
            "src/deep/lib.rs",
            "lib.rs"
        ));
        assert!(!section_matches("src/*.rs", "src/deep/lib.rs", "lib.rs"));
    }

    #[test]
    fn invalid_known_values_are_reported_not_applied() {
        let temp = tempfile::tempdir().expect("temp directory");
        std::fs::write(temp.path().join(".editorconfig"), "[*]\ntab_width=zero\n").expect("config");
        let settings = load_editorconfig(&temp.path().join("a.rs")).expect("load config");
        assert_eq!(settings.tab_width, None);
        assert_eq!(settings.warnings.len(), 1);
    }
}
