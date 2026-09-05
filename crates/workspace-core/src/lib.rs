//! Workspace, filesystem, search, and trust domain boundary.
//!
//! The crate owns the non-UI workspace model, canonical path handling, document open/save
//! adapters, search orchestration, file operation planning, quick-open indexing, and the
//! persistent trust/recent-workspace stores.

pub mod document;
pub mod filesystem;
pub mod path;
pub mod search;
pub mod workspace;

pub use config_core::LargeFileSettings;
pub use document::{
    DecodePolicy, DecodedText, DocumentEncodingError, DocumentLoadOptions, DocumentOpenError,
    DocumentSaveOptions, EncodingKind, LineEndings, SaveOutcome, TextDocument,
    decode_text_document, encode_text_document, inspect_line_endings, load_text_document,
    normalize_line_endings, save_text_document,
};
pub use filesystem::{
    DeletePlan, ExplorerEntry, ExplorerEntryKind, ExplorerTree, FileChangeEvent, FileChangeTracker,
    FileOperationError, FileOperationPlan, NativeFileWatcher, QuickOpenCandidate, QuickOpenIndex,
    create_file, delete_file, move_path, plan_delete, plan_move, plan_rename, rename_path,
};
pub use path::{
    CanonicalPath, PathError, canonical_workspace_identity, canonical_workspace_key,
    canonicalize_path, lexical_normalize_path, path_eq, workspace_identity_key,
};
pub use search::{
    ReplacementEdit, ReplacementPlan, ReplacementReport, SearchBackendPreference,
    SearchCancellation, SearchError, SearchEvent, SearchHit, SearchOptions, SearchSession,
    apply_replacement_plan, collect_search_results, plan_replacements, search_workspace,
};
pub use workspace::{
    RecentWorkspaceEntry, RecentWorkspaceStore, TrustState, TrustStore, TrustStoreEntry,
    WorkspaceIdentity, WorkspaceModel, WorkspaceRoot, WorkspaceSet, add_workspace_root,
    load_recent_workspaces, load_trust_store, persist_recent_workspaces, persist_trust_store,
    remove_workspace_root,
};
