//! Streaming workspace search and replacement planning.
#![allow(
    clippy::bind_instead_of_map,
    clippy::double_must_use,
    clippy::map_unwrap_or,
    clippy::missing_errors_doc,
    clippy::must_use_candidate,
    clippy::unnecessary_map_or,
    clippy::unnecessary_sort_by,
    clippy::unnecessary_wraps,
    clippy::unnested_or_patterns
)]

use std::{
    collections::BTreeMap,
    fs,
    io::{BufRead, BufReader},
    ops::Range,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{
        Arc, OnceLock,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, SyncSender},
    },
};

use ignore::{WalkBuilder, overrides::OverrideBuilder};
use regex::RegexBuilder;
use serde::Deserialize;
use thiserror::Error;

use crate::document::{DocumentError, DocumentSaveOptions, save_text_document};

static SEARCH_RUNTIME: OnceLock<Result<tokio::runtime::Runtime, String>> = OnceLock::new();

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchBackendPreference {
    PreferRg,
    ForceRg,
    ForceRust,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchOptions {
    pub roots: Vec<PathBuf>,
    pub pattern: String,
    pub literal: bool,
    pub case_sensitive: bool,
    pub whole_word: bool,
    pub includes: Vec<String>,
    pub excludes: Vec<String>,
    pub backend: SearchBackendPreference,
    pub max_results: Option<usize>,
}

impl Default for SearchOptions {
    fn default() -> Self {
        Self {
            roots: Vec::new(),
            pattern: String::new(),
            literal: true,
            case_sensitive: true,
            whole_word: false,
            includes: Vec::new(),
            excludes: Vec::new(),
            backend: SearchBackendPreference::PreferRg,
            max_results: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchHit {
    pub path: PathBuf,
    pub line_number: usize,
    pub line_text: String,
    pub byte_range: Range<usize>,
    /// Match range relative to the reported line, for exact UI marker placement.
    pub line_byte_range: Range<usize>,
    pub matched_text: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SearchEvent {
    Match(SearchHit),
    Finished { backend: SearchBackendPreference },
    Cancelled,
    Error(String),
}

#[derive(Debug)]
pub struct SearchSession {
    receiver: Receiver<SearchEvent>,
    cancel: Arc<AtomicBool>,
}

#[derive(Debug, Clone)]
pub struct SearchCancellation(Arc<AtomicBool>);

impl SearchCancellation {
    pub fn cancel(&self) {
        self.0.store(true, Ordering::SeqCst);
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplacementEdit {
    pub range: Range<usize>,
    pub replacement: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileReplacementPlan {
    pub path: PathBuf,
    pub edits: Vec<ReplacementEdit>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ReplacementPlan {
    pub files: Vec<FileReplacementPlan>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ReplacementReport {
    pub modified_files: Vec<PathBuf>,
    pub failed_files: Vec<PathBuf>,
}

#[derive(Debug, Error)]
pub enum SearchError {
    #[error("no search roots were provided")]
    MissingRoots,
    #[error("search pattern is empty")]
    EmptyPattern,
    #[error("regex error: {0}")]
    Regex(#[from] regex::Error),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("document save error: {0}")]
    Document(#[from] DocumentError),
    #[error("search was cancelled")]
    Cancelled,
    #[error("rg executable was requested but not available")]
    RgUnavailable,
    #[error("background search runtime is unavailable: {0}")]
    RuntimeUnavailable(String),
    #[error("replacement range {start}..{end} is invalid for `{path}`")]
    InvalidReplacementRange {
        path: PathBuf,
        start: usize,
        end: usize,
    },
}

pub fn search_workspace(options: SearchOptions) -> Result<SearchSession, SearchError> {
    if options.roots.is_empty() {
        return Err(SearchError::MissingRoots);
    }
    if options.pattern.trim().is_empty() {
        return Err(SearchError::EmptyPattern);
    }
    // Bound the event queue so a fast search cannot grow memory without limit when the
    // consumer is temporarily busy rendering or applying results.
    let (sender, receiver) = mpsc::sync_channel(256);
    let cancel = Arc::new(AtomicBool::new(false));
    let cancel_clone = Arc::clone(&cancel);
    spawn_search_task(move || {
        let result = match options.backend {
            SearchBackendPreference::ForceRust => run_rust_search(&options, &sender, &cancel_clone),
            SearchBackendPreference::ForceRg => run_rg_search(&options, &sender, &cancel_clone)
                .or_else(|error| {
                    let _ = sender.send(SearchEvent::Error(error.to_string()));
                    Err(error)
                }),
            SearchBackendPreference::PreferRg => {
                match run_rg_search(&options, &sender, &cancel_clone) {
                    Ok(()) => Ok(()),
                    Err(SearchError::RgUnavailable) => {
                        run_rust_search(&options, &sender, &cancel_clone)
                    }
                    Err(error) => Err(error),
                }
            }
        };
        match result {
            Ok(()) => {
                let _ = sender.send(SearchEvent::Finished {
                    backend: match options.backend {
                        SearchBackendPreference::ForceRust => SearchBackendPreference::ForceRust,
                        SearchBackendPreference::ForceRg => SearchBackendPreference::ForceRg,
                        SearchBackendPreference::PreferRg => {
                            if rg_available() {
                                SearchBackendPreference::PreferRg
                            } else {
                                SearchBackendPreference::ForceRust
                            }
                        }
                    },
                });
            }
            Err(SearchError::Cancelled) => {
                let _ = sender.send(SearchEvent::Cancelled);
            }
            Err(error) => {
                let _ = sender.send(SearchEvent::Error(error.to_string()));
            }
        }
    })?;
    Ok(SearchSession { receiver, cancel })
}

fn spawn_search_task<F>(task: F) -> Result<(), SearchError>
where
    F: FnOnce() + Send + 'static,
{
    if let Ok(handle) = tokio::runtime::Handle::try_current() {
        handle.spawn_blocking(task);
        return Ok(());
    }

    let runtime = SEARCH_RUNTIME.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .map_err(|error| error.to_string())
    });
    match runtime {
        Ok(runtime) => {
            runtime.spawn_blocking(task);
            Ok(())
        }
        Err(error) => Err(SearchError::RuntimeUnavailable(error.clone())),
    }
}

pub fn collect_search_results(session: &SearchSession) -> Result<Vec<SearchHit>, SearchError> {
    let mut hits = Vec::new();
    loop {
        match session.receiver.recv() {
            Ok(SearchEvent::Match(hit)) => hits.push(hit),
            Ok(SearchEvent::Finished { .. }) | Ok(SearchEvent::Cancelled) | Err(_) => {
                return Ok(hits);
            }
            Ok(SearchEvent::Error(message)) => {
                return Err(SearchError::Io(std::io::Error::other(message)));
            }
        }
    }
}

impl SearchSession {
    #[must_use]
    pub fn cancellation(&self) -> SearchCancellation {
        SearchCancellation(Arc::clone(&self.cancel))
    }

    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::SeqCst);
    }

    pub fn try_recv(&self) -> Option<SearchEvent> {
        self.receiver.try_recv().ok()
    }

    pub fn recv(&self) -> Option<SearchEvent> {
        self.receiver.recv().ok()
    }
}

pub fn plan_replacements(hits: &[SearchHit], replacement: &str) -> ReplacementPlan {
    let mut grouped: BTreeMap<PathBuf, Vec<ReplacementEdit>> = BTreeMap::new();
    for hit in hits {
        grouped
            .entry(hit.path.clone())
            .or_default()
            .push(ReplacementEdit {
                range: hit.byte_range.clone(),
                replacement: replacement.to_owned(),
            });
    }
    let mut files = Vec::new();
    for (path, mut edits) in grouped {
        edits.sort_by(|left, right| right.range.start.cmp(&left.range.start));
        files.push(FileReplacementPlan { path, edits });
    }
    ReplacementPlan { files }
}

pub fn apply_replacement_plan(plan: &ReplacementPlan) -> Result<ReplacementReport, SearchError> {
    let mut report = ReplacementReport::default();
    for file in &plan.files {
        match apply_file_edits(file) {
            Ok(()) => report.modified_files.push(file.path.clone()),
            Err(_) => report.failed_files.push(file.path.clone()),
        }
    }
    Ok(report)
}

fn apply_file_edits(file: &FileReplacementPlan) -> Result<(), SearchError> {
    let text = fs::read_to_string(&file.path)?;
    let mut buffer = text;
    let mut previous_start = None;
    for edit in &file.edits {
        let valid_range = edit.range.start <= edit.range.end
            && edit.range.end <= buffer.len()
            && buffer.is_char_boundary(edit.range.start)
            && buffer.is_char_boundary(edit.range.end)
            && previous_start.is_none_or(|start| edit.range.end <= start);
        if !valid_range {
            return Err(SearchError::InvalidReplacementRange {
                path: file.path.clone(),
                start: edit.range.start,
                end: edit.range.end,
            });
        }
        previous_start = Some(edit.range.start);
        buffer.replace_range(edit.range.clone(), &edit.replacement);
    }
    save_text_document(&file.path, &buffer, &DocumentSaveOptions::default())?;
    Ok(())
}

fn run_rg_search(
    options: &SearchOptions,
    sender: &SyncSender<SearchEvent>,
    cancel: &AtomicBool,
) -> Result<(), SearchError> {
    if !rg_available() {
        return Err(SearchError::RgUnavailable);
    }
    let mut command = Command::new("rg");
    command
        .arg("--json")
        .arg("--no-config")
        .arg("--no-messages")
        .arg("--color")
        .arg("never");
    if options.literal {
        command.arg("--fixed-strings");
    }
    if options.case_sensitive {
        command.arg("--case-sensitive");
    } else {
        command.arg("--ignore-case");
    }
    if options.whole_word {
        command.arg("--word-regexp");
    }
    for include in &options.includes {
        command.arg("--glob").arg(include);
    }
    for exclude in &options.excludes {
        command.arg("--glob").arg(format!("!{exclude}"));
    }
    command.arg(&options.pattern);
    for root in &options.roots {
        command.arg(root);
    }
    command.stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = command.spawn()?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| SearchError::Io(std::io::Error::other("rg stdout unavailable")))?;
    let reader = BufReader::new(stdout);
    for line in reader.lines() {
        if cancel.load(Ordering::SeqCst) {
            let _ = child.kill();
            return Err(SearchError::Cancelled);
        }
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let envelope: RgEnvelope = serde_json::from_str(&line)?;
        if let Some(hit) = envelope.into_hit() {
            if sender.send(SearchEvent::Match(hit)).is_err() {
                let _ = child.kill();
                return Err(SearchError::Cancelled);
            }
        }
    }
    let _ = child.wait();
    Ok(())
}

fn run_rust_search(
    options: &SearchOptions,
    sender: &SyncSender<SearchEvent>,
    cancel: &AtomicBool,
) -> Result<(), SearchError> {
    let regex = compile_search_regex(options)?;
    let overrides = build_overrides(&options.includes, &options.excludes)?;
    let mut emitted = 0_usize;
    for root in &options.roots {
        let mut builder = WalkBuilder::new(root);
        builder
            .follow_links(false)
            .git_ignore(true)
            .git_exclude(true)
            .ignore(true)
            .hidden(false)
            .overrides(overrides.clone());
        for result in builder.build() {
            if cancel.load(Ordering::SeqCst) {
                return Err(SearchError::Cancelled);
            }
            let entry = match result {
                Ok(entry) => entry,
                Err(error) => {
                    let _ = sender.send(SearchEvent::Error(error.to_string()));
                    continue;
                }
            };
            if !entry
                .file_type()
                .map(|file_type| file_type.is_file())
                .unwrap_or(false)
            {
                continue;
            }
            let path = entry.path().to_path_buf();
            if let Ok(text) = fs::read_to_string(&path) {
                if text.chars().any(|character| character == '\0') {
                    continue;
                }
                for hit in search_text_file(&path, &text, &regex, options.whole_word)? {
                    if cancel.load(Ordering::SeqCst) {
                        return Err(SearchError::Cancelled);
                    }
                    if sender.send(SearchEvent::Match(hit)).is_err() {
                        return Err(SearchError::Cancelled);
                    }
                    emitted = emitted.saturating_add(1);
                    if options.max_results.is_some_and(|limit| emitted >= limit) {
                        return Ok(());
                    }
                }
            }
        }
    }
    Ok(())
}

fn search_text_file(
    path: &Path,
    text: &str,
    regex: &regex::Regex,
    whole_word: bool,
) -> Result<Vec<SearchHit>, SearchError> {
    let mut hits = Vec::new();
    let mut line_start = 0_usize;
    for (line_number, line) in text.split_inclusive('\n').enumerate() {
        let line_text = line.trim_end_matches('\n').trim_end_matches('\r');
        for capture in regex.find_iter(line_text) {
            if whole_word && !matches_word_boundaries(line_text, capture.start(), capture.end()) {
                continue;
            }
            hits.push(SearchHit {
                path: path.to_path_buf(),
                line_number: line_number + 1,
                line_text: line_text.to_owned(),
                byte_range: line_start + capture.start()..line_start + capture.end(),
                line_byte_range: capture.start()..capture.end(),
                matched_text: capture.as_str().to_owned(),
            });
        }
        line_start += line.len();
    }
    Ok(hits)
}

fn compile_search_regex(options: &SearchOptions) -> Result<regex::Regex, SearchError> {
    let pattern = if options.literal {
        regex::escape(&options.pattern)
    } else {
        options.pattern.clone()
    };
    let pattern = if options.whole_word {
        format!(r"\b(?:{pattern})\b")
    } else {
        pattern
    };
    let mut builder = RegexBuilder::new(&pattern);
    builder.case_insensitive(!options.case_sensitive);
    builder.build().map_err(SearchError::from)
}

fn matches_word_boundaries(text: &str, start: usize, end: usize) -> bool {
    let left = text[..start]
        .chars()
        .next_back()
        .map_or(true, |character| !is_word_character(character));
    let right = text[end..]
        .chars()
        .next()
        .map_or(true, |character| !is_word_character(character));
    left && right
}

fn is_word_character(character: char) -> bool {
    character.is_alphanumeric() || character == '_'
}

fn build_overrides(
    includes: &[String],
    excludes: &[String],
) -> Result<ignore::overrides::Override, SearchError> {
    let mut builder = OverrideBuilder::new(Path::new("."));
    for include in includes {
        builder
            .add(include)
            .map_err(|error| SearchError::Io(std::io::Error::other(error)))?;
    }
    for exclude in excludes {
        builder
            .add(&format!("!{exclude}"))
            .map_err(|error| SearchError::Io(std::io::Error::other(error)))?;
    }
    builder
        .build()
        .map_err(|error| SearchError::Io(std::io::Error::other(error)))
}

fn rg_available() -> bool {
    Command::new("rg")
        .arg("--version")
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

#[derive(Debug, Deserialize)]
struct RgEnvelope {
    #[serde(rename = "type")]
    kind: String,
    data: Option<RgData>,
}

#[derive(Debug, Deserialize)]
struct RgData {
    path: Option<RgPath>,
    lines: Option<RgLines>,
    line_number: Option<usize>,
    absolute_offset: Option<usize>,
    submatches: Option<Vec<RgSubmatch>>,
}

#[derive(Debug, Deserialize)]
struct RgPath {
    text: String,
}

#[derive(Debug, Deserialize)]
struct RgLines {
    text: String,
}

#[derive(Debug, Deserialize)]
struct RgSubmatch {
    #[serde(rename = "match")]
    matched: RgMatchText,
    start: usize,
    end: usize,
}

#[derive(Debug, Deserialize)]
struct RgMatchText {
    text: String,
}

impl RgEnvelope {
    fn into_hit(self) -> Option<SearchHit> {
        if self.kind != "match" {
            return None;
        }
        let data = self.data?;
        let path = PathBuf::from(data.path?.text);
        let line_text = data.lines?.text;
        let line_number = data.line_number.unwrap_or(0);
        let absolute_offset = data.absolute_offset.unwrap_or(0);
        let submatch = data.submatches?.into_iter().next()?;
        Some(SearchHit {
            path,
            line_number,
            line_text,
            byte_range: absolute_offset + submatch.start..absolute_offset + submatch.end,
            line_byte_range: submatch.start..submatch.end,
            matched_text: submatch.matched.text,
        })
    }
}

pub fn parse_rg_json_lines(lines: &[&str]) -> Result<Vec<SearchHit>, SearchError> {
    let mut hits = Vec::new();
    for line in lines {
        if line.trim().is_empty() {
            continue;
        }
        let envelope: RgEnvelope = serde_json::from_str(line)?;
        if let Some(hit) = envelope.into_hit() {
            hits.push(hit);
        }
    }
    Ok(hits)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn rg_json_fixture_parses_hits() {
        let lines = [
            r#"{"type":"match","data":{"path":{"text":"src/main.rs"},"lines":{"text":"let apple = 1;"},"line_number":3,"absolute_offset":21,"submatches":[{"match":{"text":"apple"},"start":4,"end":9}]}}"#,
            r#"{"type":"context","data":{"path":{"text":"src/main.rs"},"lines":{"text":"fn main() {}"}}}"#,
        ];
        let hits = parse_rg_json_lines(&lines).expect("parse");
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].matched_text, "apple");
        assert_eq!(hits[0].byte_range, 25..30);
    }

    #[test]
    fn replacement_plan_groups_edits_by_file() {
        let hits = vec![
            SearchHit {
                path: PathBuf::from("a.txt"),
                line_number: 1,
                line_text: "foo".to_owned(),
                byte_range: 0..3,
                line_byte_range: 0..3,
                matched_text: "foo".to_owned(),
            },
            SearchHit {
                path: PathBuf::from("a.txt"),
                line_number: 2,
                line_text: "bar".to_owned(),
                byte_range: 4..7,
                line_byte_range: 0..3,
                matched_text: "bar".to_owned(),
            },
        ];
        let plan = plan_replacements(&hits, "x");
        assert_eq!(plan.files.len(), 1);
        assert_eq!(plan.files[0].edits.len(), 2);
    }

    #[test]
    fn invalid_replacement_range_is_reported_without_writing() {
        let directory = tempdir().expect("directory");
        let path = directory.path().join("unicode.txt");
        fs::write(&path, "界").expect("seed");
        let plan = ReplacementPlan {
            files: vec![FileReplacementPlan {
                path: path.clone(),
                edits: vec![ReplacementEdit {
                    range: 1..2,
                    replacement: "x".to_owned(),
                }],
            }],
        };
        let report = apply_replacement_plan(&plan).expect("report");
        assert!(report.modified_files.is_empty());
        assert_eq!(report.failed_files, vec![path.clone()]);
        assert_eq!(fs::read_to_string(path).expect("read"), "界");
    }

    #[test]
    fn fallback_search_finds_literal_matches() {
        let dir = tempdir().expect("dir");
        let file = dir.path().join("example.txt");
        fs::write(&file, "Alpha\nbeta\n").expect("write");
        let options = SearchOptions {
            roots: vec![dir.path().to_path_buf()],
            pattern: "alpha".to_owned(),
            literal: true,
            case_sensitive: false,
            whole_word: false,
            includes: Vec::new(),
            excludes: Vec::new(),
            backend: SearchBackendPreference::ForceRust,
            max_results: None,
        };
        let session = search_workspace(options).expect("session");
        let hits = collect_search_results(&session).expect("results");
        assert_eq!(hits.len(), 1);
    }
}
