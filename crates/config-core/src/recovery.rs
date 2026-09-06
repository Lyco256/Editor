//! Versioned session persistence in the operating system's application-data directory.

use std::collections::BTreeMap;
use std::ffi::OsStr;
use std::io::Write;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

use crate::encoding::{EncodingKind, LineEndings};

/// Current on-disk session schema version.
pub const CURRENT_SESSION_FORMAT: u32 = 1;
const RECORD_PREFIX: &str = "session-";
const RECORD_SUFFIX: &str = ".json";

/// Cursor state using logical line and character coordinates.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CursorState {
    pub line: u32,
    pub character: u32,
}

/// Selection state for one editor.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SelectionState {
    pub anchor: CursorState,
    pub active: CursorState,
}

/// Persisted editor tab disposition. Unknown older records default to a permanent open tab.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum EditorDisposition {
    Preview,
    #[default]
    Open,
    Pinned,
}

/// Split orientation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SplitAxis {
    Horizontal,
    Vertical,
}

/// Serializable split layout with stable editor identifiers at leaves.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[derive(Default)]
pub enum SplitLayout {
    #[default]
    Empty,
    Editor {
        editor_id: String,
    },
    Split {
        axis: SplitAxis,
        ratio: f32,
        first: Box<Self>,
        second: Box<Self>,
    },
}

/// State required to restore one open editor and its unsaved buffer.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct EditorSession {
    pub editor_id: String,
    pub original_path: Option<PathBuf>,
    pub original_encoding: Option<EncodingKind>,
    pub original_bom: bool,
    pub original_line_ending: Option<LineEndings>,
    pub cursor: CursorState,
    pub selections: Vec<SelectionState>,
    pub unsaved_text: Option<String>,
    pub dirty: bool,
    pub disposition: EditorDisposition,
    #[serde(flatten)]
    pub unknown: BTreeMap<String, Value>,
}

/// Complete restart state. Additive fields remain compatible within format version 1.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SessionState {
    pub format_version: u32,
    pub workspace_roots: Vec<PathBuf>,
    /// Recently opened workspace roots, bounded by the UI to twenty entries.
    pub recent_workspaces: Vec<PathBuf>,
    pub editors: Vec<EditorSession>,
    pub tab_order: Vec<String>,
    pub split_layout: SplitLayout,
    pub active_editor: Option<String>,
    #[serde(flatten)]
    pub unknown: BTreeMap<String, Value>,
}

impl Default for SessionState {
    fn default() -> Self {
        Self {
            format_version: CURRENT_SESSION_FORMAT,
            workspace_roots: Vec::new(),
            recent_workspaces: Vec::new(),
            editors: Vec::new(),
            tab_order: Vec::new(),
            split_layout: SplitLayout::Empty,
            active_editor: None,
            unknown: BTreeMap::new(),
        }
    }
}

/// One corrupt/incompatible record skipped while loading an older valid generation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecoveryWarning {
    pub path: PathBuf,
    pub message: String,
}

/// Safe recovery result. Corrupt records are warnings rather than startup failures.
#[derive(Debug, Clone, PartialEq)]
pub struct RecoveryLoad {
    pub session: Option<SessionState>,
    pub source: Option<PathBuf>,
    pub warnings: Vec<RecoveryWarning>,
}

/// Typed persistence failures.
#[derive(Debug, Error)]
pub enum RecoveryError {
    #[error("application identifier must be a single safe path component")]
    InvalidApplicationId,
    #[error("the operating system application-data directory is unavailable")]
    AppDataUnavailable,
    #[error("recovery I/O failed for {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("could not serialize recovery state: {0}")]
    Serialize(#[from] serde_json::Error),
    #[error("concurrent recovery writers exhausted generation retries")]
    ConcurrentWriters,
}

/// Owns the application-data recovery directory. It never derives a write path from a document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecoveryStore {
    directory: PathBuf,
}

impl RecoveryStore {
    /// Creates a store under the platform application-data directory.
    ///
    /// # Errors
    ///
    /// Returns [`RecoveryError::InvalidApplicationId`] for unsafe identifiers and
    /// [`RecoveryError::AppDataUnavailable`] when the platform app-data directory cannot be located.
    pub fn for_application(application_id: &str) -> Result<Self, RecoveryError> {
        validate_application_id(application_id)?;
        let base = platform_app_data_dir().ok_or(RecoveryError::AppDataUnavailable)?;
        Ok(Self {
            directory: base.join(application_id).join("recovery"),
        })
    }

    /// Creates a store below an explicitly supplied application-data base (useful to embedders/tests).
    ///
    /// # Errors
    ///
    /// Returns [`RecoveryError::InvalidApplicationId`] for unsafe identifiers.
    pub fn from_app_data_base(
        app_data_base: &Path,
        application_id: &str,
    ) -> Result<Self, RecoveryError> {
        validate_application_id(application_id)?;
        Ok(Self {
            directory: app_data_base.join(application_id).join("recovery"),
        })
    }

    /// Returns the only directory this store writes.
    #[must_use]
    pub fn directory(&self) -> &Path {
        &self.directory
    }

    /// Writes a flushed temporary record and atomically publishes it under a new generation name.
    ///
    /// # Errors
    ///
    /// Returns [`RecoveryError::Io`] for filesystem failures, [`RecoveryError::Serialize`]
    /// when the session cannot be encoded, and [`RecoveryError::ConcurrentWriters`] when all
    /// bounded generation retries are exhausted.
    pub fn save(&self, session: &SessionState) -> Result<PathBuf, RecoveryError> {
        std::fs::create_dir_all(&self.directory).map_err(|source| RecoveryError::Io {
            path: self.directory.clone(),
            source,
        })?;
        let bytes = serde_json::to_vec(session)?;
        let first_generation = self.highest_generation()?.map_or(1, |value| value + 1);
        for offset in 0..16_u64 {
            let generation = first_generation + offset;
            let destination = self.record_path(generation);
            let mut temporary =
                tempfile::NamedTempFile::new_in(&self.directory).map_err(|source| {
                    RecoveryError::Io {
                        path: self.directory.clone(),
                        source,
                    }
                })?;
            temporary
                .write_all(&bytes)
                .map_err(|source| RecoveryError::Io {
                    path: temporary.path().to_path_buf(),
                    source,
                })?;
            temporary
                .write_all(b"\n")
                .map_err(|source| RecoveryError::Io {
                    path: temporary.path().to_path_buf(),
                    source,
                })?;
            temporary.flush().map_err(|source| RecoveryError::Io {
                path: temporary.path().to_path_buf(),
                source,
            })?;
            temporary
                .as_file()
                .sync_all()
                .map_err(|source| RecoveryError::Io {
                    path: temporary.path().to_path_buf(),
                    source,
                })?;
            match temporary.persist_noclobber(&destination) {
                Ok(_) => return Ok(destination),
                Err(error) if error.error.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(error) => {
                    return Err(RecoveryError::Io {
                        path: destination,
                        source: error.error,
                    });
                }
            }
        }
        Err(RecoveryError::ConcurrentWriters)
    }

    /// Loads the newest valid supported record, falling back across corrupt/truncated generations.
    ///
    /// # Errors
    ///
    /// Returns [`RecoveryError::Io`] if the recovery directory cannot be enumerated.
    pub fn load_latest(&self) -> Result<RecoveryLoad, RecoveryError> {
        let mut records = self.records_descending()?;
        let mut warnings = Vec::new();
        for (_, path) in records.drain(..) {
            let bytes = match std::fs::read(&path) {
                Ok(bytes) => bytes,
                Err(source) => {
                    warnings.push(RecoveryWarning {
                        path,
                        message: source.to_string(),
                    });
                    continue;
                }
            };
            match serde_json::from_slice::<SessionState>(&bytes) {
                Ok(session) if session.format_version == CURRENT_SESSION_FORMAT => {
                    return Ok(RecoveryLoad {
                        session: Some(session),
                        source: Some(path),
                        warnings,
                    });
                }
                Ok(session) => warnings.push(RecoveryWarning {
                    path,
                    message: format!("unsupported session format {}", session.format_version),
                }),
                Err(error) => warnings.push(RecoveryWarning {
                    path,
                    message: error.to_string(),
                }),
            }
        }
        Ok(RecoveryLoad {
            session: None,
            source: None,
            warnings,
        })
    }

    fn highest_generation(&self) -> Result<Option<u64>, RecoveryError> {
        Ok(self
            .records_descending()?
            .first()
            .map(|(generation, _)| *generation))
    }

    fn records_descending(&self) -> Result<Vec<(u64, PathBuf)>, RecoveryError> {
        let entries = match std::fs::read_dir(&self.directory) {
            Ok(entries) => entries,
            Err(source) if source.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(source) => {
                return Err(RecoveryError::Io {
                    path: self.directory.clone(),
                    source,
                });
            }
        };
        let mut records = Vec::new();
        for entry in entries {
            let entry = entry.map_err(|source| RecoveryError::Io {
                path: self.directory.clone(),
                source,
            })?;
            if let Some(generation) = parse_generation(&entry.file_name()) {
                records.push((generation, entry.path()));
            }
        }
        records.sort_unstable_by_key(|entry| std::cmp::Reverse(entry.0));
        Ok(records)
    }

    fn record_path(&self, generation: u64) -> PathBuf {
        self.directory
            .join(format!("{RECORD_PREFIX}{generation:020}{RECORD_SUFFIX}"))
    }
}

fn validate_application_id(application_id: &str) -> Result<(), RecoveryError> {
    let path = Path::new(application_id);
    if application_id.is_empty()
        || path.components().count() != 1
        || application_id == "."
        || application_id == ".."
    {
        Err(RecoveryError::InvalidApplicationId)
    } else {
        Ok(())
    }
}

fn platform_app_data_dir() -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        std::env::var_os("LOCALAPPDATA")
            .or_else(|| std::env::var_os("APPDATA"))
            .map(PathBuf::from)
    }
    #[cfg(not(target_os = "windows"))]
    {
        std::env::var_os("XDG_STATE_HOME")
            .map(PathBuf::from)
            .or_else(|| {
                std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/state"))
            })
    }
}

fn parse_generation(name: &OsStr) -> Option<u64> {
    let name = name.to_str()?;
    name.strip_prefix(RECORD_PREFIX)?
        .strip_suffix(RECORD_SUFFIX)?
        .parse()
        .ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dirty_session(text: &str, original_path: PathBuf) -> SessionState {
        SessionState {
            workspace_roots: vec![original_path.parent().expect("parent").to_path_buf()],
            editors: vec![EditorSession {
                editor_id: "editor-1".to_owned(),
                original_path: Some(original_path),
                original_encoding: Some(EncodingKind::Utf16Le),
                original_bom: false,
                original_line_ending: Some(LineEndings::Crlf),
                cursor: CursorState {
                    line: 3,
                    character: 7,
                },
                selections: vec![SelectionState::default()],
                unsaved_text: Some(text.to_owned()),
                dirty: true,
                disposition: EditorDisposition::Open,
                unknown: BTreeMap::new(),
            }],
            tab_order: vec!["editor-1".to_owned()],
            split_layout: SplitLayout::Editor {
                editor_id: "editor-1".to_owned(),
            },
            active_editor: Some("editor-1".to_owned()),
            ..SessionState::default()
        }
    }

    #[test]
    fn atomic_generations_survive_simulated_restart_without_workspace_writes() {
        let temp = tempfile::tempdir().expect("temp directory");
        let workspace = temp.path().join("workspace");
        let app_data = temp.path().join("app-data");
        std::fs::create_dir_all(&workspace).expect("workspace");
        let document = workspace.join("unsaved.rs");
        let store = RecoveryStore::from_app_data_base(&app_data, "Editor").expect("store");
        let first = dirty_session("first", document.clone());
        let second = dirty_session("second", document.clone());
        let first_path = store.save(&first).expect("first save");
        let second_path = store.save(&second).expect("replacement save");
        assert_ne!(first_path, second_path);
        let restarted = RecoveryStore::from_app_data_base(&app_data, "Editor").expect("restart");
        assert_eq!(restarted.load_latest().expect("load").session, Some(second));
        assert!(
            !document.exists(),
            "unsaved contents must not enter workspace"
        );
    }

    #[test]
    fn corrupt_newest_record_falls_back_to_last_valid_generation() {
        let temp = tempfile::tempdir().expect("temp directory");
        let store = RecoveryStore::from_app_data_base(temp.path(), "Editor").expect("store");
        let session = dirty_session("recover me", PathBuf::from("C:/work/file.rs"));
        store.save(&session).expect("valid save");
        let corrupt_path = store.record_path(2);
        std::fs::write(&corrupt_path, b"{\"formatVersion\":1").expect("truncated record");
        let loaded = store.load_latest().expect("safe load");
        assert_eq!(loaded.session, Some(session));
        assert_eq!(loaded.warnings.len(), 1);
        assert_eq!(loaded.warnings[0].path, corrupt_path);
    }

    #[test]
    fn all_corrupt_records_fail_safely() {
        let temp = tempfile::tempdir().expect("temp directory");
        let store = RecoveryStore::from_app_data_base(temp.path(), "Editor").expect("store");
        std::fs::create_dir_all(store.directory()).expect("recovery directory");
        std::fs::write(store.record_path(1), b"not json").expect("corrupt record");
        let loaded = store.load_latest().expect("safe load");
        assert!(loaded.session.is_none());
        assert_eq!(loaded.warnings.len(), 1);
    }

    #[test]
    fn version_one_allows_additive_unknown_fields() {
        let serialized = r#"{
            "formatVersion":1,
            "workspaceRoots":[],
            "editors":[],
            "tabOrder":[],
            "splitLayout":{"kind":"empty"},
            "activeEditor":null,
            "futureField":{"enabled":true}
        }"#;
        let session: SessionState = serde_json::from_str(serialized).expect("compatible session");
        assert_eq!(session.format_version, CURRENT_SESSION_FORMAT);
        assert_eq!(session.unknown["futureField"]["enabled"], true);
        let round_trip = serde_json::to_string(&session).expect("serialize");
        assert!(round_trip.contains("futureField"));
    }

    #[test]
    fn application_identifier_cannot_escape_app_data() {
        let temp = tempfile::tempdir().expect("temp directory");
        assert!(matches!(
            RecoveryStore::from_app_data_base(temp.path(), "../workspace"),
            Err(RecoveryError::InvalidApplicationId)
        ));
    }
}
