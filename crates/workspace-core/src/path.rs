//! Canonical path handling and workspace identity helpers.
#![allow(clippy::missing_errors_doc)]

use std::{
    ffi::OsStr,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum PathError {
    #[error("path does not exist: {path}")]
    NotFound { path: PathBuf },
    #[error("path error: {0}")]
    Io(#[from] std::io::Error),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CanonicalPath(PathBuf);

impl CanonicalPath {
    #[must_use]
    pub fn new(path: PathBuf) -> Self {
        Self(path)
    }

    #[must_use]
    pub fn as_path(&self) -> &Path {
        &self.0
    }

    #[must_use]
    pub fn into_path(self) -> PathBuf {
        self.0
    }
}

#[must_use]
pub fn canonical_workspace_identity(path: &Path) -> PathBuf {
    match std::fs::canonicalize(path) {
        Ok(canonical) => canonical,
        Err(_) => lexical_normalize_path(path),
    }
}

#[must_use]
pub fn canonical_workspace_key(path: &Path) -> String {
    normalized_path_key(&canonical_workspace_identity(path))
}

#[must_use]
pub fn workspace_identity_key(path: &Path) -> String {
    canonical_workspace_key(path)
}

#[must_use]
pub fn lexical_normalize_path(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        normalized.push(component.as_os_str());
    }
    normalized
}

pub fn canonicalize_path(path: &Path) -> Result<CanonicalPath, PathError> {
    if !path.exists() {
        return Err(PathError::NotFound {
            path: path.to_path_buf(),
        });
    }
    Ok(CanonicalPath(std::fs::canonicalize(path)?))
}

#[must_use]
pub fn path_eq(left: &Path, right: &Path) -> bool {
    normalized_path_key(left) == normalized_path_key(right)
}

#[must_use]
pub fn normalized_path_key(path: &Path) -> String {
    let mut key = path.to_string_lossy().replace('\\', "/");
    while key.contains("//") {
        key = key.replace("//", "/");
    }
    if cfg!(windows) {
        key.make_ascii_lowercase();
    }
    key
}

#[must_use]
pub fn is_case_only_rename(source: &Path, target: &Path) -> bool {
    cfg!(windows)
        && normalized_path_key(source) == normalized_path_key(target)
        && source != target
}

#[must_use]
pub fn file_name(path: &Path) -> Option<&OsStr> {
    path.file_name()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_fold_separator_variants_and_case_on_windows() {
        let a = Path::new(r"C:\Temp\Editor\workspace");
        let b = Path::new(r"c:/temp/editor/workspace");
        assert_eq!(normalized_path_key(a), normalized_path_key(b));
    }

    #[test]
    fn rename_helper_detects_case_only_changes_on_windows() {
        let source = Path::new(r"C:\Temp\Name.txt");
        let target = Path::new(r"C:\Temp\name.txt");
        if cfg!(windows) {
            assert!(is_case_only_rename(source, target));
        } else {
            assert!(!is_case_only_rename(source, target));
        }
    }
}
