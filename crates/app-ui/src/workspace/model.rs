//! Workspace state, fake model events, and pure action handling.

use std::path::{Path, PathBuf};

use editor_types::{MouseButton, MouseEvent, ScreenCell};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkspaceTrustState {
    Trusted,
    Untrusted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkspaceEntryKind {
    Root,
    Directory,
    File,
    Symlink,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceEntry {
    pub path: PathBuf,
    pub label: String,
    pub kind: WorkspaceEntryKind,
    pub expanded: bool,
    pub dirty: bool,
    pub children: Vec<WorkspaceEntry>,
}

impl WorkspaceEntry {
    #[must_use]
    pub fn root(path: impl Into<PathBuf>, label: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            label: label.into(),
            kind: WorkspaceEntryKind::Root,
            expanded: true,
            dirty: false,
            children: Vec::new(),
        }
    }

    #[must_use]
    pub fn directory(path: impl Into<PathBuf>, label: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            label: label.into(),
            kind: WorkspaceEntryKind::Directory,
            expanded: false,
            dirty: false,
            children: Vec::new(),
        }
    }

    #[must_use]
    pub fn file(path: impl Into<PathBuf>, label: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            label: label.into(),
            kind: WorkspaceEntryKind::File,
            expanded: false,
            dirty: false,
            children: Vec::new(),
        }
    }

    #[must_use]
    pub fn symlink(path: impl Into<PathBuf>, label: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            label: label.into(),
            kind: WorkspaceEntryKind::Symlink,
            expanded: false,
            dirty: false,
            children: Vec::new(),
        }
    }

    #[must_use]
    pub fn with_expanded(mut self, expanded: bool) -> Self {
        self.expanded = expanded;
        self
    }

    #[must_use]
    pub fn with_dirty(mut self, dirty: bool) -> Self {
        self.dirty = dirty;
        self
    }

    #[must_use]
    pub fn with_children(mut self, children: Vec<WorkspaceEntry>) -> Self {
        self.children = children;
        self
    }

    #[must_use]
    pub fn is_branch(&self) -> bool {
        matches!(
            self.kind,
            WorkspaceEntryKind::Root | WorkspaceEntryKind::Directory
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuickOpenDisposition {
    CurrentEditor,
    OpenToSide,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuickOpenCandidate {
    pub path: PathBuf,
    pub label: String,
    pub recent_rank: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QuickOpenAction {
    SetQuery(String),
    Append(char),
    Backspace,
    SelectNext,
    SelectPrevious,
    SelectIndex(usize),
    MousePick(usize),
    OpenCurrent,
    OpenToSide,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct QuickOpenState {
    pub query: String,
    pub candidates: Vec<QuickOpenCandidate>,
    pub results: Vec<QuickOpenCandidate>,
    pub selected: usize,
}

impl QuickOpenState {
    pub fn set_candidates(&mut self, candidates: Vec<QuickOpenCandidate>) {
        self.candidates = candidates;
        self.recompute();
    }

    pub fn set_query(&mut self, query: impl Into<String>) {
        self.query = query.into();
        self.recompute();
    }

    pub fn apply_action(
        &mut self,
        action: QuickOpenAction,
    ) -> Option<(QuickOpenDisposition, PathBuf)> {
        match action {
            QuickOpenAction::SetQuery(query) => {
                self.query = query;
                self.recompute();
                None
            }
            QuickOpenAction::Append(ch) => {
                self.query.push(ch);
                self.recompute();
                None
            }
            QuickOpenAction::Backspace => {
                self.query.pop();
                self.recompute();
                None
            }
            QuickOpenAction::SelectNext => {
                if !self.results.is_empty() {
                    self.selected = (self.selected + 1) % self.results.len();
                }
                None
            }
            QuickOpenAction::SelectPrevious => {
                if !self.results.is_empty() {
                    self.selected = if self.selected == 0 {
                        self.results.len() - 1
                    } else {
                        self.selected - 1
                    };
                }
                None
            }
            QuickOpenAction::SelectIndex(index) | QuickOpenAction::MousePick(index) => {
                if index < self.results.len() {
                    self.selected = index;
                }
                None
            }
            QuickOpenAction::OpenCurrent => self
                .results
                .get(self.selected)
                .map(|candidate| (QuickOpenDisposition::CurrentEditor, candidate.path.clone())),
            QuickOpenAction::OpenToSide => self
                .results
                .get(self.selected)
                .map(|candidate| (QuickOpenDisposition::OpenToSide, candidate.path.clone())),
        }
    }

    #[must_use]
    pub fn selected_candidate(&self) -> Option<&QuickOpenCandidate> {
        self.results.get(self.selected)
    }

    fn recompute(&mut self) {
        let mut results: Vec<_> = self
            .candidates
            .iter()
            .filter_map(|candidate| {
                let score = quick_open_score(candidate, &self.query)?;
                Some((score, candidate.clone()))
            })
            .collect();
        results.sort_by(|left, right| {
            left.0
                .cmp(&right.0)
                .then_with(|| left.1.recent_rank.cmp(&right.1.recent_rank))
                .then_with(|| left.1.path.cmp(&right.1.path))
        });
        self.results = results
            .into_iter()
            .map(|(_, candidate)| candidate)
            .collect();
        if self.results.is_empty() {
            self.selected = 0;
        } else if self.selected >= self.results.len() {
            self.selected = self.results.len() - 1;
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchOptionsView {
    pub literal: bool,
    pub case_sensitive: bool,
    pub whole_word: bool,
    pub max_results: Option<usize>,
    pub include: Option<String>,
    pub exclude: Option<String>,
}

impl Default for SearchOptionsView {
    fn default() -> Self {
        Self {
            literal: true,
            case_sensitive: false,
            whole_word: false,
            max_results: None,
            include: None,
            exclude: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchResult {
    pub path: PathBuf,
    pub line_number: usize,
    pub line_text: String,
    pub matched_text: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplacePreview {
    pub file_count: usize,
    pub match_count: usize,
    pub confirmed: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchStatus {
    Idle,
    Running,
    Cancelled,
    Superseded,
    Complete,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchState {
    pub session_id: u64,
    pub query: String,
    pub options: SearchOptionsView,
    pub results: Vec<SearchResult>,
    pub selected: usize,
    pub replace_preview: Option<ReplacePreview>,
    pub status: SearchStatus,
}

impl Default for SearchState {
    fn default() -> Self {
        Self {
            session_id: 0,
            query: String::new(),
            options: SearchOptionsView::default(),
            results: Vec::new(),
            selected: 0,
            replace_preview: None,
            status: SearchStatus::Idle,
        }
    }
}

impl SearchState {
    pub fn start_session(
        &mut self,
        session_id: u64,
        query: impl Into<String>,
        options: SearchOptionsView,
    ) {
        self.session_id = session_id;
        self.query = query.into();
        self.options = options;
        self.results.clear();
        self.selected = 0;
        self.replace_preview = None;
        self.status = SearchStatus::Running;
    }

    pub fn supersede(
        &mut self,
        session_id: u64,
        query: impl Into<String>,
        options: SearchOptionsView,
    ) {
        self.session_id = session_id;
        self.query = query.into();
        self.options = options;
        self.results.clear();
        self.selected = 0;
        self.replace_preview = None;
        self.status = SearchStatus::Superseded;
    }

    pub fn push_result(&mut self, session_id: u64, result: SearchResult) {
        if self.session_id == session_id && self.status == SearchStatus::Running {
            self.results.push(result);
        }
    }

    pub fn finish_session(&mut self, session_id: u64) {
        if self.session_id == session_id && self.status == SearchStatus::Running {
            self.status = SearchStatus::Complete;
        }
    }

    pub fn cancel_session(&mut self, session_id: u64) {
        if self.session_id == session_id && self.status == SearchStatus::Running {
            self.status = SearchStatus::Cancelled;
        }
    }

    pub fn set_replace_preview(&mut self, file_count: usize, match_count: usize) {
        self.replace_preview = Some(ReplacePreview {
            file_count,
            match_count,
            confirmed: false,
        });
    }

    pub fn confirm_replace_preview(&mut self) {
        if let Some(preview) = &mut self.replace_preview {
            preview.confirmed = true;
        }
    }

    pub fn select_next(&mut self) {
        if !self.results.is_empty() {
            self.selected = (self.selected + 1) % self.results.len();
        }
    }

    pub fn select_previous(&mut self) {
        if !self.results.is_empty() {
            self.selected = if self.selected == 0 {
                self.results.len() - 1
            } else {
                self.selected - 1
            };
        }
    }

    #[must_use]
    pub fn selected_result(&self) -> Option<&SearchResult> {
        self.results.get(self.selected)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkspacePrompt {
    CreateFile {
        path: PathBuf,
    },
    Rename {
        source: PathBuf,
        target: PathBuf,
    },
    Move {
        source: PathBuf,
        target: PathBuf,
    },
    Delete {
        path: PathBuf,
        recursive: bool,
        byte_len: u64,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceUiState {
    pub explorer: ExplorerState,
    pub quick_open: QuickOpenState,
    pub search: SearchState,
    pub recent_workspaces: Vec<PathBuf>,
    pub trust: WorkspaceTrustState,
    pub prompt: Option<WorkspacePrompt>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecentWorkspaceRow {
    pub id: String,
    pub path: PathBuf,
    pub exists: bool,
}

impl WorkspaceUiState {
    /// Returns stable, typed recent-workspace rows with missing-path state preserved for UI.
    #[must_use]
    pub fn recent_rows(&self) -> Vec<RecentWorkspaceRow> {
        self.recent_workspaces
            .iter()
            .map(|path| RecentWorkspaceRow {
                id: path.to_string_lossy().into_owned(),
                path: path.clone(),
                exists: path.is_dir(),
            })
            .collect()
    }

    pub fn remove_recent(&mut self, path: &Path) {
        self.recent_workspaces.retain(|item| item != path);
    }

    pub fn remember_workspace(&mut self, path: PathBuf) {
        self.recent_workspaces.retain(|item| item != &path);
        self.recent_workspaces.insert(0, path);
        self.recent_workspaces.truncate(20);
    }
}

impl Default for WorkspaceUiState {
    fn default() -> Self {
        Self {
            explorer: ExplorerState::default(),
            quick_open: QuickOpenState::default(),
            search: SearchState::default(),
            recent_workspaces: Vec::new(),
            trust: WorkspaceTrustState::Untrusted,
            prompt: None,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ExplorerState {
    pub roots: Vec<WorkspaceEntry>,
    pub selected: Option<PathBuf>,
    pub scroll: usize,
}

impl ExplorerState {
    pub fn set_roots(&mut self, roots: Vec<WorkspaceEntry>) {
        self.roots = roots;
        if self.selected.is_none() {
            self.selected = self
                .roots
                .iter()
                .find(|entry| entry.is_branch())
                .map(|entry| entry.path.clone());
        }
    }

    pub fn select(&mut self, path: impl Into<PathBuf>) {
        self.selected = Some(path.into());
    }

    pub fn toggle_expanded(&mut self, path: &Path) -> bool {
        toggle_expanded(&mut self.roots, path)
    }

    #[must_use]
    pub fn visible_entries(&self) -> Vec<ExplorerLine> {
        let mut lines = Vec::new();
        for root in &self.roots {
            collect_visible_entries(root, 0, self.selected.as_deref(), &mut lines);
        }
        lines
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExplorerLine {
    pub path: PathBuf,
    pub label: String,
    pub kind: WorkspaceEntryKind,
    pub depth: usize,
    pub expanded: bool,
    pub dirty: bool,
    pub selected: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkspaceModelEvent {
    RootsChanged {
        roots: Vec<WorkspaceEntry>,
        recent: Vec<PathBuf>,
        trust: WorkspaceTrustState,
    },
    QuickOpenChanged {
        query: String,
        candidates: Vec<QuickOpenCandidate>,
    },
    SearchStarted {
        session_id: u64,
        query: String,
        options: SearchOptionsView,
    },
    SearchResult {
        session_id: u64,
        result: SearchResult,
    },
    SearchFinished {
        session_id: u64,
    },
    SearchCancelled {
        session_id: u64,
    },
    SearchSuperseded {
        session_id: u64,
        query: String,
        options: SearchOptionsView,
    },
    ReplacePreview {
        file_count: usize,
        match_count: usize,
    },
    RecentChanged {
        recent: Vec<PathBuf>,
    },
    TrustChanged {
        trust: WorkspaceTrustState,
    },
    PromptChanged {
        prompt: Option<WorkspacePrompt>,
    },
}

impl WorkspaceUiState {
    pub fn apply_event(&mut self, event: WorkspaceModelEvent) {
        match event {
            WorkspaceModelEvent::RootsChanged {
                roots,
                recent,
                trust,
            } => {
                self.explorer.set_roots(roots);
                self.recent_workspaces = recent;
                self.trust = trust;
            }
            WorkspaceModelEvent::QuickOpenChanged { query, candidates } => {
                self.quick_open.query = query;
                self.quick_open.set_candidates(candidates);
            }
            WorkspaceModelEvent::SearchStarted {
                session_id,
                query,
                options,
            } => self.search.start_session(session_id, query, options),
            WorkspaceModelEvent::SearchResult { session_id, result } => {
                self.search.push_result(session_id, result);
            }
            WorkspaceModelEvent::SearchFinished { session_id } => {
                self.search.finish_session(session_id);
            }
            WorkspaceModelEvent::SearchCancelled { session_id } => {
                self.search.cancel_session(session_id);
            }
            WorkspaceModelEvent::SearchSuperseded {
                session_id,
                query,
                options,
            } => self.search.supersede(session_id, query, options),
            WorkspaceModelEvent::ReplacePreview {
                file_count,
                match_count,
            } => self.search.set_replace_preview(file_count, match_count),
            WorkspaceModelEvent::RecentChanged { recent } => {
                self.recent_workspaces = recent;
            }
            WorkspaceModelEvent::TrustChanged { trust } => {
                self.trust = trust;
            }
            WorkspaceModelEvent::PromptChanged { prompt } => {
                self.prompt = prompt;
            }
        }
    }
}

fn collect_visible_entries(
    entry: &WorkspaceEntry,
    depth: usize,
    selected: Option<&Path>,
    lines: &mut Vec<ExplorerLine>,
) {
    lines.push(ExplorerLine {
        path: entry.path.clone(),
        label: entry.label.clone(),
        kind: entry.kind,
        depth,
        expanded: entry.expanded,
        dirty: entry.dirty,
        selected: selected.is_some_and(|path| path == entry.path.as_path()),
    });
    if entry.expanded {
        for child in &entry.children {
            collect_visible_entries(child, depth + 1, selected, lines);
        }
    }
}

fn toggle_expanded(entries: &mut [WorkspaceEntry], path: &Path) -> bool {
    for entry in entries {
        if entry.path == path {
            if entry.is_branch() {
                entry.expanded = !entry.expanded;
            }
            return true;
        }
        if toggle_expanded(&mut entry.children, path) {
            return true;
        }
    }
    false
}

fn quick_open_score(candidate: &QuickOpenCandidate, query: &str) -> Option<u32> {
    let needle = query.trim().to_ascii_lowercase();
    if needle.is_empty() {
        return Some(saturating_usize_to_u32(candidate.recent_rank));
    }
    let path = candidate.path.to_string_lossy().to_ascii_lowercase();
    let label = candidate.label.to_ascii_lowercase();
    let path_score = score_text(&path, &needle);
    let label_score = score_text(&label, &needle);
    match (label_score, path_score) {
        (Some(left), Some(right)) => {
            Some(left.min(right) + saturating_usize_to_u32(candidate.recent_rank))
        }
        (Some(score), None) | (None, Some(score)) => {
            Some(score + saturating_usize_to_u32(candidate.recent_rank))
        }
        (None, None) => None,
    }
}

fn score_text(candidate: &str, needle: &str) -> Option<u32> {
    if candidate.contains(needle) {
        return candidate.find(needle).map(saturating_usize_to_u32);
    }
    let mut score = 0_u32;
    let mut search_index = 0_usize;
    for character in needle.chars() {
        let remainder = candidate.get(search_index..)?;
        let position = remainder.find(character)?;
        score = score.saturating_add(saturating_usize_to_u32(position));
        search_index = search_index.saturating_add(position + character.len_utf8());
    }
    Some(score.saturating_add(100))
}

fn saturating_usize_to_u32(value: usize) -> u32 {
    u32::try_from(value).unwrap_or(u32::MAX)
}

#[must_use]
pub fn infer_label(path: &Path) -> String {
    path.file_name()
        .and_then(|segment| segment.to_str())
        .map_or_else(|| path.to_string_lossy().into_owned(), ToOwned::to_owned)
}

#[must_use]
pub fn quick_open_candidate(path: impl Into<PathBuf>, recent_rank: usize) -> QuickOpenCandidate {
    let path = path.into();
    QuickOpenCandidate {
        label: infer_label(&path),
        path,
        recent_rank,
    }
}

#[must_use]
pub fn mouse_pick_from_row(position: ScreenCell, first_item_row: u16) -> Option<usize> {
    position.row.checked_sub(first_item_row).map(usize::from)
}

#[must_use]
pub fn is_mouse_left_click(event: &MouseEvent) -> bool {
    matches!(
        event.action,
        editor_types::MouseAction::Down(MouseButton::Left)
    )
}
