//! Lazy explorer traversal, file operations, quick-open indexing, and file-change tracking.
#![allow(
    clippy::double_must_use,
    clippy::cast_possible_truncation,
    clippy::map_unwrap_or,
    clippy::missing_errors_doc,
    clippy::must_use_candidate,
    clippy::redundant_closure_for_method_calls
)]

use std::{
    collections::BTreeMap,
    fs,
    hash::{Hash, Hasher},
    io,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{
    document::{DocumentLoadOptions, TextDocument, load_text_document, save_bytes_atomically},
    path::{CanonicalPath, is_case_only_rename, normalized_path_key, path_eq},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum ExplorerEntryKind {
    File,
    Directory,
    Symlink,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExplorerEntry {
    pub path: PathBuf,
    pub kind: ExplorerEntryKind,
    pub depth: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExplorerTree {
    roots: Vec<CanonicalPath>,
    excludes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuickOpenCandidate {
    pub path: PathBuf,
    pub score: u32,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuickOpenIndex {
    entries: Vec<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeletePlan {
    pub path: PathBuf,
    pub recursive: bool,
    pub is_directory: bool,
    pub byte_len: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RenamePlan {
    pub source: PathBuf,
    pub target: PathBuf,
    pub case_only_on_windows: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MovePlan {
    pub source: PathBuf,
    pub target: PathBuf,
    pub case_only_on_windows: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum FileOperationPlan {
    Rename(RenamePlan),
    Move(MovePlan),
    Delete(DeletePlan),
    CreateFile { path: PathBuf },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileChangeEvent {
    Reload { path: PathBuf, document: TextDocument },
    Conflict { path: PathBuf, document: TextDocument },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WatchedFile {
    path: PathBuf,
    load_options: DocumentLoadOptions,
    dirty: bool,
    fingerprint: FileFingerprint,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileChangeTracker {
    watched: BTreeMap<String, WatchedFile>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct FileFingerprint {
    byte_len: u64,
    content_hash: u64,
    modified_unix_seconds: u64,
}

#[derive(Debug, Error)]
pub enum FileOperationError {
    #[error("io error: {0}")]
    Io(#[from] io::Error),
    #[error("document error: {0}")]
    Document(#[from] crate::document::DocumentError),
    #[error("path does not belong to the workspace")]
    OutsideWorkspace,
    #[error("rename target already exists: {0}")]
    TargetExists(PathBuf),
    #[error("symlink loop or unreadable path: {0}")]
    Unreadable(PathBuf),
}

impl ExplorerTree {
    #[must_use]
    pub fn new(roots: Vec<CanonicalPath>, excludes: Vec<String>) -> Self {
        Self { roots, excludes }
    }

    #[must_use]
    pub fn roots(&self) -> &[CanonicalPath] {
        &self.roots
    }

    pub fn root_entries(&self) -> Vec<ExplorerEntry> {
        self.roots
            .iter()
            .map(|root| ExplorerEntry {
                path: root.as_path().to_path_buf(),
                kind: ExplorerEntryKind::Directory,
                depth: 0,
            })
            .collect()
    }

    pub fn children(&self, directory: &Path) -> Result<Vec<ExplorerEntry>, FileOperationError> {
        let mut entries = Vec::new();
        for root in &self.roots {
            let root_path = root.as_path();
            if !directory.starts_with(root_path) {
                continue;
            }
            let mut gitignore_patterns = load_gitignore_patterns(root_path);
            gitignore_patterns.extend(load_gitignore_patterns(directory));
            for entry in fs::read_dir(directory)? {
                let entry = entry?;
                let path = entry.path();
                let metadata = fs::symlink_metadata(&path)?;
                let is_directory = metadata.is_dir();
                if !is_visible_entry(
                    directory,
                    &path,
                    is_directory,
                    &gitignore_patterns,
                    &self.excludes,
                ) {
                    continue;
                }
                entries.push(ExplorerEntry {
                    path,
                    kind: classify_metadata(&metadata),
                    depth: 1,
                });
            }
        }
        entries.sort_by(|left, right| {
            left.depth
                .cmp(&right.depth)
                .then_with(|| left.kind.cmp(&right.kind))
                .then_with(|| left.path.cmp(&right.path))
        });
        Ok(entries)
    }
}

impl QuickOpenIndex {
    pub fn rebuild(tree: &ExplorerTree) -> Result<Self, FileOperationError> {
        let mut entries = Vec::new();
        for root in tree.roots() {
            entries.extend(visible_file_entries(root.as_path(), &tree.excludes)?);
        }
        entries.sort();
        entries.dedup();
        Ok(Self { entries })
    }

    #[must_use]
    pub fn entries(&self) -> &[PathBuf] {
        &self.entries
    }

    #[must_use]
    pub fn query(&self, needle: &str, recent: &[PathBuf]) -> Vec<QuickOpenCandidate> {
        let needle = needle.trim();
        let mut scored = Vec::new();
        for path in &self.entries {
            let score = quick_open_score(path, needle, recent);
            if score.is_some() {
                scored.push(QuickOpenCandidate {
                    path: path.clone(),
                    score: score.unwrap_or_default(),
                });
            }
        }
        scored.sort_by(|left, right| left.score.cmp(&right.score).then_with(|| left.path.cmp(&right.path)));
        scored
    }
}

impl FileChangeTracker {
    #[must_use]
    pub fn new() -> Self {
        Self {
            watched: BTreeMap::new(),
        }
    }

    pub fn watch(&mut self, document: &TextDocument, dirty: bool) -> Result<(), FileOperationError> {
        let fingerprint = fingerprint_path(document.path.as_path())?;
        self.watched.insert(
            normalized_path_key(document.path.as_path()),
            WatchedFile {
                path: document.path.clone(),
                load_options: DocumentLoadOptions {
                    fallback_encoding: document.encoding.clone(),
                    malformed_input_policy: crate::document::DecodePolicy::Strict,
                    large_file_settings: crate::LargeFileSettings {
                        threshold_bytes: u64::MAX,
                    },
                },
                dirty,
                fingerprint,
            },
        );
        Ok(())
    }

    pub fn poll(&mut self) -> Result<Vec<FileChangeEvent>, FileOperationError> {
        let mut events = Vec::new();
        for watched in self.watched.values_mut() {
            let current = fingerprint_path(watched.path.as_path())?;
            if current == watched.fingerprint {
                continue;
            }
            let document = load_text_document(watched.path.as_path(), &watched.load_options)?;
            let event = if watched.dirty {
                FileChangeEvent::Conflict {
                    path: watched.path.clone(),
                    document,
                }
            } else {
                FileChangeEvent::Reload {
                    path: watched.path.clone(),
                    document,
                }
            };
            events.push(event);
            watched.fingerprint = current;
        }
        Ok(events)
    }
}

impl Default for FileChangeTracker {
    fn default() -> Self {
        Self::new()
    }
}

pub fn plan_delete(path: &Path) -> Result<DeletePlan, FileOperationError> {
    let metadata = fs::symlink_metadata(path)?;
    Ok(DeletePlan {
        path: path.to_path_buf(),
        recursive: metadata.is_dir(),
        is_directory: metadata.is_dir(),
        byte_len: metadata.len(),
    })
}

pub fn plan_rename(source: &Path, target: &Path) -> Result<RenamePlan, FileOperationError> {
    let _ = fs::symlink_metadata(source)?;
    Ok(RenamePlan {
        source: source.to_path_buf(),
        target: target.to_path_buf(),
        case_only_on_windows: is_case_only_rename(source, target),
    })
}

pub fn plan_move(source: &Path, target: &Path) -> Result<MovePlan, FileOperationError> {
    let _ = fs::symlink_metadata(source)?;
    Ok(MovePlan {
        source: source.to_path_buf(),
        target: target.to_path_buf(),
        case_only_on_windows: is_case_only_rename(source, target),
    })
}

pub fn create_file(path: &Path, contents: &[u8]) -> Result<(), FileOperationError> {
    if let Some(parent) = path.parent() {
        if !parent.exists() {
            return Err(FileOperationError::OutsideWorkspace);
        }
    }
    save_bytes_atomically(path, contents)?;
    Ok(())
}

pub fn rename_path(source: &Path, target: &Path) -> Result<(), FileOperationError> {
    if let Some(parent) = target.parent() {
        if !parent.exists() {
            return Err(FileOperationError::OutsideWorkspace);
        }
    }
    if source == target {
        return Ok(());
    }
    if is_case_only_rename(source, target) {
        let mut temporary = source.to_path_buf();
        temporary.set_file_name(format!(
            "__workspace_core_tmp_{}",
            source
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("rename")
        ));
        if temporary.exists() {
            fs::remove_file(&temporary)?;
        }
        fs::rename(source, &temporary)?;
        fs::rename(&temporary, target)?;
        return Ok(());
    }
    fs::rename(source, target)?;
    Ok(())
}

pub fn move_path(source: &Path, target: &Path) -> Result<(), FileOperationError> {
    rename_path(source, target)
}

pub fn delete_file(path: &Path) -> Result<(), FileOperationError> {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.is_dir() {
        fs::remove_dir_all(path)?;
    } else {
        fs::remove_file(path)?;
    }
    Ok(())
}

fn visible_file_entries(root: &Path, excludes: &[String]) -> Result<Vec<PathBuf>, FileOperationError> {
    let mut files = Vec::new();
    walk_visible_files(root, &[], excludes, &mut files)?;
    Ok(files)
}

fn walk_visible_files(
    directory: &Path,
    inherited_patterns: &[String],
    excludes: &[String],
    files: &mut Vec<PathBuf>,
) -> Result<(), FileOperationError> {
    let mut patterns = inherited_patterns.to_vec();
    patterns.extend(load_gitignore_patterns(directory));
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path)?;
        let is_directory = metadata.is_dir();
        if !is_visible_entry(directory, &path, is_directory, &patterns, excludes) {
            continue;
        }
        if is_directory {
            walk_visible_files(&path, &patterns, excludes, files)?;
        } else {
            files.push(path);
        }
    }
    Ok(())
}

fn load_gitignore_patterns(directory: &Path) -> Vec<String> {
    let path = directory.join(".gitignore");
    let Ok(contents) = fs::read_to_string(&path) else {
        return Vec::new();
    };
    contents
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(|line| line.trim_start_matches('!').to_owned())
        .collect()
}

fn is_visible_entry(
    directory: &Path,
    path: &Path,
    is_directory: bool,
    gitignore_patterns: &[String],
    excludes: &[String],
) -> bool {
    let name = path.file_name().and_then(|value| value.to_str()).unwrap_or("");
    let relative = path
        .strip_prefix(directory)
        .ok()
        .and_then(|value| value.to_str())
        .unwrap_or(name);
    let candidates = [name, relative];
    !gitignore_patterns
        .iter()
        .chain(excludes.iter())
        .any(|pattern| candidates.iter().any(|candidate| matches_ignore_pattern(pattern, candidate, is_directory)))
}

fn matches_ignore_pattern(pattern: &str, candidate: &str, is_directory: bool) -> bool {
    let trimmed = pattern.trim();
    if trimmed.is_empty() {
        return false;
    }
    if let Some(directory_pattern) = trimmed.strip_suffix('/') {
        return is_directory && candidate == directory_pattern;
    }
    if let Some(extension) = trimmed.strip_prefix("*.") {
        return candidate
            .rsplit_once('.')
            .is_some_and(|(_, file_extension)| file_extension.eq_ignore_ascii_case(extension));
    }
    if let Some((prefix, suffix)) = trimmed.split_once('*') {
        return candidate.starts_with(prefix) && candidate.ends_with(suffix);
    }
    candidate == trimmed
}

fn classify_metadata(metadata: &fs::Metadata) -> ExplorerEntryKind {
    if metadata.file_type().is_symlink() {
        ExplorerEntryKind::Symlink
    } else if metadata.is_dir() {
        ExplorerEntryKind::Directory
    } else {
        ExplorerEntryKind::File
    }
}

fn fingerprint_path(path: &Path) -> Result<FileFingerprint, FileOperationError> {
    let metadata = fs::metadata(path)?;
    let bytes = if metadata.is_file() {
        fs::read(path)?
    } else {
        Vec::new()
    };
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    bytes.hash(&mut hasher);
    let modified_unix_seconds = metadata
        .modified()
        .ok()
        .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
        .map_or(0, |duration| duration.as_secs());
    Ok(FileFingerprint {
        byte_len: metadata.len(),
        content_hash: hasher.finish(),
        modified_unix_seconds,
    })
}

fn quick_open_score(path: &Path, needle: &str, recent: &[PathBuf]) -> Option<u32> {
    if needle.is_empty() {
        return Some(recent_rank(path, recent).unwrap_or(1000));
    }
    let query = needle.to_ascii_lowercase();
    let display = normalized_path_key(path).to_ascii_lowercase();
    let filename = path
        .file_name()
        .and_then(|value| value.to_str())
        .map_or_else(String::new, |value| value.to_ascii_lowercase());
    let rank = recent_rank(path, recent).unwrap_or(0);
    score_text(&filename, &query)
        .map(|score| score + rank)
        .or_else(|| score_text(&display, &query).map(|score| score + 1_000 + rank))
}

fn recent_rank(path: &Path, recent: &[PathBuf]) -> Option<u32> {
    recent
        .iter()
        .position(|candidate| path_eq(candidate, path))
        .map(|index| index as u32)
}

fn fuzzy_subsequence_score(candidate: &str, needle: &str) -> Option<u32> {
    let mut score = 0_u32;
    let mut search_index = 0_usize;
    for character in needle.chars() {
        let remainder = candidate.get(search_index..)?;
        let position = remainder.find(character)?;
        score = score.saturating_add(position as u32);
        search_index = search_index.saturating_add(position + character.len_utf8());
    }
    Some(score)
}

fn score_text(candidate: &str, needle: &str) -> Option<u32> {
    if candidate.contains(needle) {
        return candidate.find(needle).map(|index| index as u32);
    }
    fuzzy_subsequence_score(candidate, needle)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn quick_open_index_finds_recent_paths_first() {
        let dir = tempdir().expect("dir");
        let root = CanonicalPath::new(dir.path().to_path_buf());
        let tree = ExplorerTree::new(vec![root], Vec::new());
        let path_a = dir.path().join("alpha.txt");
        let path_b = dir.path().join("beta.txt");
        fs::write(&path_a, "a").expect("write");
        fs::write(&path_b, "b").expect("write");
        let index = QuickOpenIndex::rebuild(&tree).expect("index");
        let results = index.query("a", &[path_b.clone(), path_a.clone()]);
        assert!(!results.is_empty());
        assert_eq!(results[0].path, path_a);
    }

    #[test]
    fn rename_plan_flags_case_only_changes_on_windows() {
        let dir = tempdir().expect("dir");
        let source = dir.path().join("Name.txt");
        let target = dir.path().join("name.txt");
        fs::write(&source, "content").expect("seed");
        let plan = plan_rename(&source, &target).expect("plan");
        assert_eq!(plan.case_only_on_windows, cfg!(windows));
    }
}
