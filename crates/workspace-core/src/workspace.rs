//! Workspace root identity, trust, and recent-workspace persistence.
#![allow(
    clippy::double_must_use,
    clippy::map_unwrap_or,
    clippy::missing_errors_doc,
    clippy::must_use_candidate
)]

use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use editor_types::WorkspaceId;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::path::{CanonicalPath, PathError, canonical_workspace_key, canonical_workspace_identity};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TrustState {
    Trusted,
    Untrusted,
}

impl TrustState {
    #[must_use]
    pub const fn allows_external_processes(self) -> bool {
        matches!(self, Self::Trusted)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceIdentity {
    pub key: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceRoot {
    pub id: WorkspaceId,
    pub path: CanonicalPath,
    pub identity: WorkspaceIdentity,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceSet {
    roots: BTreeMap<String, WorkspaceRoot>,
    next_id: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceModel {
    pub roots: WorkspaceSet,
    pub trust: TrustStore,
    pub recent: RecentWorkspaceStore,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrustStoreEntry {
    pub key: String,
    pub state: TrustState,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrustStore {
    entries: BTreeMap<String, TrustStoreEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecentWorkspaceEntry {
    pub key: String,
    pub path: PathBuf,
    pub last_opened_unix_seconds: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecentWorkspaceStore {
    entries: Vec<RecentWorkspaceEntry>,
}

#[derive(Debug, Error)]
pub enum WorkspaceError {
    #[error(transparent)]
    Path(#[from] PathError),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
}

impl WorkspaceIdentity {
    #[must_use]
    pub fn from_path(path: &Path) -> Self {
        Self {
            key: canonical_workspace_key(path),
        }
    }
}

impl WorkspaceRoot {
    #[must_use]
    pub fn new(path: &Path, id: WorkspaceId) -> Result<Self, WorkspaceError> {
        Ok(Self {
            id,
            path: CanonicalPath::new(canonical_workspace_identity(path)),
            identity: WorkspaceIdentity::from_path(path),
        })
    }
}

impl WorkspaceSet {
    pub fn add_root(&mut self, path: &Path) -> Result<WorkspaceId, WorkspaceError> {
        let identity = WorkspaceIdentity::from_path(path);
        if let Some(existing) = self.roots.get(&identity.key) {
            return Ok(existing.id);
        }
        let id = WorkspaceId(self.next_id);
        self.next_id = self.next_id.saturating_add(1);
        let root = WorkspaceRoot::new(path, id)?;
        self.roots.insert(identity.key, root);
        Ok(id)
    }

    pub fn remove_root(&mut self, path: &Path) -> Result<Option<WorkspaceRoot>, WorkspaceError> {
        let key = canonical_workspace_key(path);
        Ok(self.roots.remove(&key))
    }

    #[must_use]
    pub fn roots(&self) -> impl Iterator<Item = &WorkspaceRoot> {
        self.roots.values()
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.roots.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.roots.is_empty()
    }

    #[must_use]
    pub fn contains(&self, path: &Path) -> bool {
        self.roots.contains_key(&canonical_workspace_key(path))
    }
}

impl TrustStore {
    #[must_use]
    pub fn state_for_path(&self, path: &Path) -> TrustState {
        self.entries
            .get(&canonical_workspace_key(path))
            .map(|entry| entry.state)
            .unwrap_or(TrustState::Untrusted)
    }

    pub fn set_state(&mut self, path: &Path, state: TrustState) {
        let key = canonical_workspace_key(path);
        self.entries
            .insert(key.clone(), TrustStoreEntry { key, state });
    }

    pub fn load(path: &Path) -> Result<Self, WorkspaceError> {
        let bytes = std::fs::read(path)?;
        Ok(serde_json::from_slice(&bytes)?)
    }

    pub fn save(&self, path: &Path) -> Result<(), WorkspaceError> {
        let bytes = serde_json::to_vec_pretty(self)?;
        std::fs::write(path, bytes)?;
        Ok(())
    }

    #[must_use]
    pub fn entries(&self) -> impl Iterator<Item = &TrustStoreEntry> {
        self.entries.values()
    }
}

impl RecentWorkspaceStore {
    pub fn touch(&mut self, path: &Path) {
        let key = canonical_workspace_key(path);
        let entry = RecentWorkspaceEntry {
            key: key.clone(),
            path: canonical_workspace_identity(path),
            last_opened_unix_seconds: now_unix_seconds(),
        };
        self.entries.retain(|candidate| candidate.key != key);
        self.entries.insert(0, entry);
    }

    #[must_use]
    pub fn entries(&self) -> &[RecentWorkspaceEntry] {
        &self.entries
    }

    pub fn load(path: &Path) -> Result<Self, WorkspaceError> {
        let bytes = std::fs::read(path)?;
        Ok(serde_json::from_slice(&bytes)?)
    }

    pub fn save(&self, path: &Path) -> Result<(), WorkspaceError> {
        let bytes = serde_json::to_vec_pretty(self)?;
        std::fs::write(path, bytes)?;
        Ok(())
    }
}

#[must_use]
pub fn add_workspace_root(set: &mut WorkspaceSet, path: &Path) -> Result<WorkspaceId, WorkspaceError> {
    set.add_root(path)
}

pub fn remove_workspace_root(
    set: &mut WorkspaceSet,
    path: &Path,
) -> Result<Option<WorkspaceRoot>, WorkspaceError> {
    set.remove_root(path)
}

pub fn load_trust_store(path: &Path) -> Result<TrustStore, WorkspaceError> {
    TrustStore::load(path)
}

pub fn persist_trust_store(store: &TrustStore, path: &Path) -> Result<(), WorkspaceError> {
    store.save(path)
}

pub fn load_recent_workspaces(path: &Path) -> Result<RecentWorkspaceStore, WorkspaceError> {
    RecentWorkspaceStore::load(path)
}

pub fn persist_recent_workspaces(
    store: &RecentWorkspaceStore,
    path: &Path,
) -> Result<(), WorkspaceError> {
    store.save(path)
}

#[must_use]
pub fn workspace_identity_key(path: &Path) -> String {
    canonical_workspace_key(path)
}

#[must_use]
pub fn now_unix_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn add_root_deduplicates_canonical_paths() {
        let dir = tempdir().expect("dir");
        let root = dir.path();
        let alias = root.join(".");
        let mut set = WorkspaceSet::default();
        let first = set.add_root(root).expect("first");
        let second = set.add_root(&alias).expect("second");
        assert_eq!(first, second);
        assert_eq!(set.len(), 1);
    }

    #[test]
    fn trust_keys_remain_stable_for_same_workspace() {
        let dir = tempdir().expect("dir");
        let key_a = workspace_identity_key(dir.path());
        let key_b = workspace_identity_key(&dir.path().join("."));
        assert_eq!(key_a, key_b);
    }

    #[test]
    fn recent_workspaces_deduplicate_and_promote_latest() {
        let dir = tempdir().expect("dir");
        let mut store = RecentWorkspaceStore::default();
        store.touch(dir.path());
        store.touch(dir.path());
        assert_eq!(store.entries().len(), 1);
    }
}
