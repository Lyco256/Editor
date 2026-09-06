//! Source-control view boundary.
//!
//! This module keeps Git UI pure: it turns repository and diff models into deterministic
//! framebuffer snapshots and normalized actions. It does not execute Git itself, talk to the
//! filesystem, or build shell command strings.

use std::{fmt::Write as _, path::PathBuf};

use editor_types::{GitStatusSummary, OutputLevel, StyleRole, TerminalCapabilities};
use terminal_backend::{Cell, Framebuffer};
use vcs_git::{
    GitBranchInfo, GitCommitRequest, GitConflictFile, GitDiffFile, GitDiffLineKind, GitDiscardPlan,
    GitFetchRequest, GitFileChange, GitLogEntry, GitPullRequest, GitPushRequest,
    GitStashCreateRequest, GitStashEntry, GitStatusEntry,
};

#[cfg(test)]
use vcs_git::{GitDiffHunk, GitDiffLine};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GitView {
    Changes,
    Diff,
    Branches,
    Stashes,
    History,
    Commit,
    Conflicts,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GitChangeMode {
    WorkingTree,
    Index,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GitTrustState {
    Trusted,
    Untrusted,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitOperationState {
    pub label: String,
    pub busy: bool,
    pub progress: Option<(u32, u32)>,
    pub level: OutputLevel,
    pub stderr: Vec<String>,
}

impl GitOperationState {
    #[must_use]
    pub fn idle() -> Self {
        Self {
            label: String::from("idle"),
            busy: false,
            progress: None,
            level: OutputLevel::Information,
            stderr: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitConfirmation {
    pub title: String,
    pub reason: String,
    pub action_label: String,
    pub cancel_label: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitCommitForm {
    pub message: String,
    pub amend: bool,
    pub allow_empty: bool,
    pub author_name: Option<String>,
    pub can_submit: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitDashboardState {
    pub root: PathBuf,
    pub summary: GitStatusSummary,
    pub branch_state: Option<String>,
    pub head: Option<String>,
    pub view: GitView,
    pub change_mode: GitChangeMode,
    pub trust: GitTrustState,
    pub busy: GitOperationState,
    pub confirmation: Option<GitConfirmation>,
    pub commit_form: GitCommitForm,
    pub changes: Vec<GitStatusEntry>,
    pub diff_files: Vec<GitDiffFile>,
    pub selected_file: Option<usize>,
    pub selected_hunk: Option<usize>,
    pub branches: Vec<GitBranchInfo>,
    pub selected_branch: Option<usize>,
    pub stashes: Vec<GitStashEntry>,
    pub selected_stash: Option<usize>,
    pub history: Vec<GitLogEntry>,
    pub selected_history: Option<usize>,
    pub conflicts: Vec<GitConflictFile>,
    pub command_log: Vec<String>,
}

impl GitDashboardState {
    #[must_use]
    pub fn from_repository(root: impl Into<PathBuf>, status: GitRepositorySnapshot) -> Self {
        Self {
            root: root.into(),
            summary: status.summary,
            branch_state: status.branch_state,
            head: status.head,
            view: GitView::Changes,
            change_mode: GitChangeMode::WorkingTree,
            trust: status.trust,
            busy: GitOperationState::idle(),
            confirmation: None,
            commit_form: GitCommitForm {
                message: String::new(),
                amend: false,
                allow_empty: false,
                author_name: None,
                can_submit: true,
            },
            changes: status.changes,
            diff_files: status.diff_files,
            selected_file: None,
            selected_hunk: None,
            branches: status.branches,
            selected_branch: None,
            stashes: status.stashes,
            selected_stash: None,
            history: status.history,
            selected_history: None,
            conflicts: status.conflicts,
            command_log: Vec::new(),
        }
    }

    #[must_use]
    pub fn disabled_by_policy(root: impl Into<PathBuf>) -> Self {
        Self {
            root: root.into(),
            summary: GitStatusSummary::default(),
            branch_state: None,
            head: None,
            view: GitView::Changes,
            change_mode: GitChangeMode::WorkingTree,
            trust: GitTrustState::Untrusted,
            busy: GitOperationState {
                label: String::from("git blocked by workspace trust"),
                busy: false,
                progress: None,
                level: OutputLevel::Warning,
                stderr: vec![String::from(
                    "Git operations are disabled in untrusted workspaces.",
                )],
            },
            confirmation: None,
            commit_form: GitCommitForm {
                message: String::from("trusted-workspace required"),
                amend: false,
                allow_empty: false,
                author_name: None,
                can_submit: false,
            },
            changes: Vec::new(),
            diff_files: Vec::new(),
            selected_file: None,
            selected_hunk: None,
            branches: Vec::new(),
            selected_branch: None,
            stashes: Vec::new(),
            selected_stash: None,
            history: Vec::new(),
            selected_history: None,
            conflicts: Vec::new(),
            command_log: vec![String::from("workspace trust gate active")],
        }
    }

    #[must_use]
    pub fn grouped_changes(&self) -> Vec<GitChangeGroup> {
        group_changes(&self.changes)
    }

    #[must_use]
    pub fn render_snapshot(&self, width: u16, height: u16) -> String {
        let mut frame = Framebuffer::new(width, height);
        self.render(
            &mut frame,
            Rect::new(0, 0, width, height),
            TerminalCapabilities::default(),
        );
        frame_snapshot(&frame)
    }

    pub fn render(&self, frame: &mut Framebuffer, area: Rect, _capabilities: TerminalCapabilities) {
        if area.is_empty() {
            return;
        }
        fill_rect(
            frame,
            area,
            " ",
            StyleRole::EditorText,
            StyleRole::EditorBackground,
        );

        let layout = layout(area, self.view);
        draw_border(frame, area, StyleRole::Gutter, StyleRole::EditorBackground);
        let _ = write_text(
            frame,
            area.x.saturating_add(1),
            area.y,
            &format!("Source Control - {}", self.root.display()),
            StyleRole::Panel,
            StyleRole::EditorBackground,
        );

        let summary = summary_line(self);
        let _ = write_text(
            frame,
            area.x.saturating_add(1),
            area.y.saturating_add(1),
            &summary,
            StyleRole::EditorText,
            StyleRole::EditorBackground,
        );

        if matches!(self.trust, GitTrustState::Untrusted) {
            let banner = "Git disabled by workspace trust policy";
            let _ = write_text(
                frame,
                area.x.saturating_add(1),
                area.y.saturating_add(2),
                banner,
                StyleRole::Warning,
                StyleRole::EditorBackground,
            );
        }

        self.render_sidebar(frame, layout.left);
        self.render_main(frame, layout.right, layout.compact);
        self.render_footer(frame, layout.footer);
        if let Some(confirmation) = &self.confirmation {
            Self::render_confirmation(frame, layout.confirmation, confirmation);
        }
        if let Some(output) = layout.output {
            self.render_output(frame, output);
        }
    }

    pub fn dispatch(&mut self, action: GitAction) -> Option<GitEffectRequest> {
        match action {
            GitAction::SwitchView(view) => {
                self.view = view;
                None
            }
            GitAction::ToggleChangeMode => {
                self.change_mode = match self.change_mode {
                    GitChangeMode::WorkingTree => GitChangeMode::Index,
                    GitChangeMode::Index => GitChangeMode::WorkingTree,
                };
                None
            }
            GitAction::SelectChange(index) => {
                self.selected_file = Some(index);
                self.selected_hunk = None;
                None
            }
            GitAction::SelectHunk(index) => {
                self.selected_hunk = Some(index);
                None
            }
            GitAction::StageFile(path) => Some(GitEffectRequest::StageFile { path }),
            GitAction::UnstageFile(path) => Some(GitEffectRequest::UnstageFile { path }),
            GitAction::StageHunk { path, hunk } => Some(GitEffectRequest::StageHunk { path, hunk }),
            GitAction::UnstageHunk { path, hunk } => {
                Some(GitEffectRequest::UnstageHunk { path, hunk })
            }
            GitAction::RequestDiscard(plan) => {
                self.confirmation = Some(GitConfirmation {
                    title: String::from("Discard changes"),
                    reason: plan.reason.clone(),
                    action_label: String::from("Discard"),
                    cancel_label: String::from("Cancel"),
                });
                Some(GitEffectRequest::PrepareDiscard(plan))
            }
            GitAction::ConfirmDiscard | GitAction::CancelDiscard => {
                self.confirmation = None;
                None
            }
            GitAction::EditCommitMessage(message) => {
                self.commit_form.message = message;
                None
            }
            GitAction::ToggleAmend => {
                self.commit_form.amend = !self.commit_form.amend;
                None
            }
            GitAction::ToggleAllowEmpty => {
                self.commit_form.allow_empty = !self.commit_form.allow_empty;
                None
            }
            GitAction::Commit => Some(GitEffectRequest::Commit(GitCommitRequest {
                message: self.commit_form.message.clone(),
                amend: self.commit_form.amend,
                allow_empty: self.commit_form.allow_empty,
                author_name: self.commit_form.author_name.clone(),
            })),
            GitAction::CreateBranch(name) => Some(GitEffectRequest::CreateBranch { name }),
            GitAction::SwitchBranch(name) => Some(GitEffectRequest::SwitchBranch { name }),
            GitAction::DeleteBranch { name, force } => {
                Some(GitEffectRequest::DeleteBranch { name, force })
            }
            GitAction::Fetch => Some(GitEffectRequest::Fetch(GitFetchRequest {
                remote: None,
                refspecs: Vec::new(),
                prune: true,
                tags: true,
            })),
            GitAction::Pull => Some(GitEffectRequest::Pull(GitPullRequest {
                remote: None,
                branch: None,
                rebase: false,
                ff_only: false,
            })),
            GitAction::Push => Some(GitEffectRequest::Push(GitPushRequest {
                remote: None,
                refspecs: Vec::new(),
                set_upstream: false,
                force_with_lease: false,
            })),
            GitAction::CreateStash => Some(GitEffectRequest::CreateStash(GitStashCreateRequest {
                message: self.commit_form.message.clone(),
                include_untracked: false,
                keep_index: false,
            })),
            GitAction::ApplyStash(reference) => Some(GitEffectRequest::ApplyStash { reference }),
            GitAction::PopStash(reference) => Some(GitEffectRequest::PopStash { reference }),
            GitAction::OpenDiffFile(path) => Some(GitEffectRequest::OpenFileDiff { path }),
            GitAction::OpenConflict(path) => Some(GitEffectRequest::OpenConflictFile { path }),
        }
    }

    #[allow(clippy::too_many_lines)]
    fn render_sidebar(&self, frame: &mut Framebuffer, area: Option<Rect>) {
        let Some(area) = area else {
            return;
        };
        draw_border(frame, area, StyleRole::Gutter, StyleRole::EditorBackground);
        let title = match self.view {
            GitView::Changes => "Changes",
            GitView::Diff => "Diff files",
            GitView::Branches => "Branches",
            GitView::Stashes => "Stashes",
            GitView::History => "History",
            GitView::Commit => "Commit",
            GitView::Conflicts => "Conflicts",
        };
        let _ = write_text(
            frame,
            area.x.saturating_add(1),
            area.y,
            title,
            StyleRole::Panel,
            StyleRole::EditorBackground,
        );
        let mut row = area.y.saturating_add(1);
        match self.view {
            GitView::Changes | GitView::Diff => {
                for (group_index, group) in self.grouped_changes().iter().enumerate() {
                    if row >= area.bottom() {
                        break;
                    }
                    let group_label = format!("{} ({})", group.title, group.entries.len());
                    let _ = write_text(
                        frame,
                        area.x.saturating_add(1),
                        row,
                        &group_label,
                        if self.selected_file == Some(group_index) {
                            StyleRole::Selection
                        } else {
                            group.severity_role()
                        },
                        if self.selected_file == Some(group_index) {
                            StyleRole::CurrentLine
                        } else {
                            StyleRole::EditorBackground
                        },
                    );
                    row = row.saturating_add(1);
                    for (entry_index, entry) in group.entries.iter().enumerate() {
                        if row >= area.bottom() {
                            break;
                        }
                        let label = format!(
                            "  {} {}",
                            status_tag(entry),
                            truncate(
                                &entry.path.display().to_string(),
                                area.width.saturating_sub(4) as usize
                            )
                        );
                        let selected = self.selected_file == Some(entry_index);
                        let _ = write_text(
                            frame,
                            area.x.saturating_add(1),
                            row,
                            &label,
                            if selected {
                                StyleRole::Selection
                            } else if entry.conflicted {
                                StyleRole::Error
                            } else if entry.staged {
                                StyleRole::GitAdded
                            } else if entry.untracked {
                                StyleRole::Hint
                            } else {
                                StyleRole::EditorText
                            },
                            if selected {
                                StyleRole::CurrentLine
                            } else {
                                StyleRole::EditorBackground
                            },
                        );
                        row = row.saturating_add(1);
                    }
                }
            }
            GitView::Branches => {
                for (index, branch) in self.branches.iter().enumerate() {
                    if row >= area.bottom() {
                        break;
                    }
                    let label = format!(
                        "{} {}{}",
                        if branch.head { ">" } else { " " },
                        branch.name,
                        branch
                            .tracking
                            .as_deref()
                            .map_or(String::new(), |tracking| format!(" -> {tracking}"))
                    );
                    let _ = write_text(
                        frame,
                        area.x.saturating_add(1),
                        row,
                        &truncate(&label, area.width.saturating_sub(2) as usize),
                        if self.selected_branch == Some(index) {
                            StyleRole::Selection
                        } else {
                            StyleRole::EditorText
                        },
                        if self.selected_branch == Some(index) {
                            StyleRole::CurrentLine
                        } else {
                            StyleRole::EditorBackground
                        },
                    );
                    row = row.saturating_add(1);
                }
            }
            GitView::Stashes => {
                for (index, stash) in self.stashes.iter().enumerate() {
                    if row >= area.bottom() {
                        break;
                    }
                    let label = format!("{} {} {}", stash.reference, stash.hash, stash.message);
                    let _ = write_text(
                        frame,
                        area.x.saturating_add(1),
                        row,
                        &truncate(&label, area.width.saturating_sub(2) as usize),
                        if self.selected_stash == Some(index) {
                            StyleRole::Selection
                        } else {
                            StyleRole::EditorText
                        },
                        if self.selected_stash == Some(index) {
                            StyleRole::CurrentLine
                        } else {
                            StyleRole::EditorBackground
                        },
                    );
                    row = row.saturating_add(1);
                }
            }
            GitView::History => {
                for (index, entry) in self.history.iter().enumerate() {
                    if row >= area.bottom() {
                        break;
                    }
                    let label = format!("{} {}", short_hash(&entry.hash), entry.summary);
                    let _ = write_text(
                        frame,
                        area.x.saturating_add(1),
                        row,
                        &truncate(&label, area.width.saturating_sub(2) as usize),
                        if self.selected_history == Some(index) {
                            StyleRole::Selection
                        } else {
                            StyleRole::EditorText
                        },
                        if self.selected_history == Some(index) {
                            StyleRole::CurrentLine
                        } else {
                            StyleRole::EditorBackground
                        },
                    );
                    row = row.saturating_add(1);
                }
            }
            GitView::Commit => {
                let lines = message_lines(&self.commit_form.message);
                let mut line_iter = lines.iter();
                for text in line_iter.by_ref() {
                    if row >= area.bottom() {
                        break;
                    }
                    let _ = write_text(
                        frame,
                        area.x.saturating_add(1),
                        row,
                        &truncate(text, area.width.saturating_sub(2) as usize),
                        StyleRole::EditorText,
                        StyleRole::EditorBackground,
                    );
                    row = row.saturating_add(1);
                }
                for flag in [
                    format!("amend: {}", yes_no(self.commit_form.amend)),
                    format!("allow empty: {}", yes_no(self.commit_form.allow_empty)),
                    format!("submit: {}", yes_no(self.commit_form.can_submit)),
                ] {
                    if row >= area.bottom() {
                        break;
                    }
                    let _ = write_text(
                        frame,
                        area.x.saturating_add(1),
                        row,
                        &truncate(&flag, area.width.saturating_sub(2) as usize),
                        StyleRole::Panel,
                        StyleRole::EditorBackground,
                    );
                    row = row.saturating_add(1);
                }
            }
            GitView::Conflicts => {
                for conflict in &self.conflicts {
                    if row >= area.bottom() {
                        break;
                    }
                    let stages = conflict
                        .stages
                        .iter()
                        .map(u8::to_string)
                        .collect::<Vec<_>>()
                        .join(",");
                    let label = format!("{} [{}]", conflict.path.display(), stages);
                    let _ = write_text(
                        frame,
                        area.x.saturating_add(1),
                        row,
                        &truncate(&label, area.width.saturating_sub(2) as usize),
                        StyleRole::Error,
                        StyleRole::EditorBackground,
                    );
                    row = row.saturating_add(1);
                }
            }
        }
    }

    fn render_main(&self, frame: &mut Framebuffer, area: Option<Rect>, compact: bool) {
        let Some(area) = area else {
            return;
        };
        draw_border(frame, area, StyleRole::Gutter, StyleRole::EditorBackground);
        let title = match self.view {
            GitView::Changes => "Repository summary",
            GitView::Diff => "Diff view",
            GitView::Branches => "Branch details",
            GitView::Stashes => "Stash details",
            GitView::History => "Commit history",
            GitView::Commit => "Commit form",
            GitView::Conflicts => "Conflict details",
        };
        let _ = write_text(
            frame,
            area.x.saturating_add(1),
            area.y,
            title,
            StyleRole::Panel,
            StyleRole::EditorBackground,
        );
        let inner_top = area.y.saturating_add(1);
        match self.view {
            GitView::Changes => self.render_changes_detail(frame, area, inner_top, compact),
            GitView::Diff => self.render_diff_view(frame, area, inner_top, compact),
            GitView::Branches => self.render_branch_detail(frame, area, inner_top),
            GitView::Stashes => self.render_stash_detail(frame, area, inner_top),
            GitView::History => self.render_history_detail(frame, area, inner_top),
            GitView::Commit => self.render_commit_form(frame, area, inner_top),
            GitView::Conflicts => self.render_conflict_detail(frame, area, inner_top),
        }
    }

    fn render_changes_detail(
        &self,
        frame: &mut Framebuffer,
        area: Rect,
        mut row: u16,
        compact: bool,
    ) {
        let grouped = self.grouped_changes();
        for group in grouped {
            if row >= area.bottom() {
                break;
            }
            let header = format!("{} {}", group.title, group.entries.len());
            let _ = write_text(
                frame,
                area.x.saturating_add(1),
                row,
                &truncate(&header, area.width.saturating_sub(2) as usize),
                group.severity_role(),
                StyleRole::EditorBackground,
            );
            row = row.saturating_add(1);
            for entry in group.entries {
                if row >= area.bottom() {
                    break;
                }
                let detail = match self.change_mode {
                    GitChangeMode::WorkingTree => {
                        if entry.staged {
                            "staged"
                        } else if entry.unstaged {
                            "unstaged"
                        } else if entry.untracked {
                            "untracked"
                        } else {
                            "clean"
                        }
                    }
                    GitChangeMode::Index => {
                        if entry.staged {
                            "index"
                        } else {
                            "working tree"
                        }
                    }
                };
                let label = if compact {
                    format!("{} {}", status_tag(&entry), detail)
                } else {
                    format!("{} {} ({detail})", status_tag(&entry), entry.path.display())
                };
                let _ = write_text(
                    frame,
                    area.x.saturating_add(1),
                    row,
                    &truncate(&label, area.width.saturating_sub(2) as usize),
                    status_role(&entry),
                    StyleRole::EditorBackground,
                );
                row = row.saturating_add(1);
            }
        }
        if row < area.bottom() {
            let toggle = format!("view: {:?}", self.change_mode);
            let _ = write_text(
                frame,
                area.x.saturating_add(1),
                row,
                &toggle,
                StyleRole::Panel,
                StyleRole::EditorBackground,
            );
        }
    }

    fn render_diff_view(&self, frame: &mut Framebuffer, area: Rect, mut row: u16, compact: bool) {
        let Some(file) = self.selected_diff_file() else {
            let _ = write_text(
                frame,
                area.x.saturating_add(1),
                row,
                "No diff selected",
                StyleRole::Hint,
                StyleRole::EditorBackground,
            );
            return;
        };
        let header = format!(
            "{} {}",
            diff_change_label(&file.change),
            file.path.display()
        );
        let _ = write_text(
            frame,
            area.x.saturating_add(1),
            row,
            &truncate(&header, area.width.saturating_sub(2) as usize),
            diff_role(file),
            StyleRole::EditorBackground,
        );
        row = row.saturating_add(1);
        for (hunk_index, hunk) in file.hunks.iter().enumerate() {
            if row >= area.bottom() {
                break;
            }
            let selected = self.selected_hunk == Some(hunk_index);
            let hunk_header = if compact {
                hunk.header.clone()
            } else {
                format!(
                    "{} {} -> {}",
                    hunk.header,
                    range_label(hunk.old_range),
                    range_label(hunk.new_range)
                )
            };
            let _ = write_text(
                frame,
                area.x.saturating_add(1),
                row,
                &truncate(&hunk_header, area.width.saturating_sub(2) as usize),
                if selected {
                    StyleRole::Selection
                } else {
                    StyleRole::Panel
                },
                if selected {
                    StyleRole::CurrentLine
                } else {
                    StyleRole::EditorBackground
                },
            );
            row = row.saturating_add(1);
            for line in &hunk.lines {
                if row >= area.bottom() {
                    break;
                }
                let (prefix, role) = match line.kind {
                    GitDiffLineKind::Context => (" ", StyleRole::EditorText),
                    GitDiffLineKind::Addition => ("+", StyleRole::GitAdded),
                    GitDiffLineKind::Removal => ("-", StyleRole::GitDeleted),
                    GitDiffLineKind::Meta => ("@", StyleRole::Panel),
                };
                let text = format!("{prefix} {}", line.text);
                let _ = write_text(
                    frame,
                    area.x.saturating_add(1),
                    row,
                    &truncate(&text, area.width.saturating_sub(2) as usize),
                    role,
                    if selected {
                        StyleRole::CurrentLine
                    } else {
                        StyleRole::EditorBackground
                    },
                );
                row = row.saturating_add(1);
            }
        }
    }

    fn render_branch_detail(&self, frame: &mut Framebuffer, area: Rect, mut row: u16) {
        for branch in &self.branches {
            if row >= area.bottom() {
                break;
            }
            let text = format!(
                "{} {} {} {}",
                if branch.head { "*" } else { " " },
                branch.name,
                branch.upstream.clone().unwrap_or_default(),
                branch.oid
            );
            let _ = write_text(
                frame,
                area.x.saturating_add(1),
                row,
                &truncate(&text, area.width.saturating_sub(2) as usize),
                if branch.head {
                    StyleRole::Selection
                } else {
                    StyleRole::EditorText
                },
                if branch.head {
                    StyleRole::CurrentLine
                } else {
                    StyleRole::EditorBackground
                },
            );
            row = row.saturating_add(1);
        }
    }

    fn render_stash_detail(&self, frame: &mut Framebuffer, area: Rect, mut row: u16) {
        for stash in &self.stashes {
            if row >= area.bottom() {
                break;
            }
            let text = format!("{} {} {}", stash.reference, stash.hash, stash.message);
            let _ = write_text(
                frame,
                area.x.saturating_add(1),
                row,
                &truncate(&text, area.width.saturating_sub(2) as usize),
                StyleRole::EditorText,
                StyleRole::EditorBackground,
            );
            row = row.saturating_add(1);
        }
    }

    fn render_history_detail(&self, frame: &mut Framebuffer, area: Rect, mut row: u16) {
        for entry in &self.history {
            if row >= area.bottom() {
                break;
            }
            let text = format!(
                "{} {} {}",
                short_hash(&entry.hash),
                entry.author_name,
                entry.summary
            );
            let _ = write_text(
                frame,
                area.x.saturating_add(1),
                row,
                &truncate(&text, area.width.saturating_sub(2) as usize),
                StyleRole::EditorText,
                StyleRole::EditorBackground,
            );
            row = row.saturating_add(1);
        }
    }

    fn render_commit_form(&self, frame: &mut Framebuffer, area: Rect, mut row: u16) {
        let author = self
            .commit_form
            .author_name
            .clone()
            .unwrap_or_else(|| String::from("author: <not set>"));
        for text in [
            format!("message: {}", self.commit_form.message),
            format!("amend: {}", yes_no(self.commit_form.amend)),
            format!("allow-empty: {}", yes_no(self.commit_form.allow_empty)),
            author,
            format!("submit: {}", yes_no(self.commit_form.can_submit)),
        ] {
            if row >= area.bottom() {
                break;
            }
            let _ = write_text(
                frame,
                area.x.saturating_add(1),
                row,
                &truncate(&text, area.width.saturating_sub(2) as usize),
                StyleRole::EditorText,
                StyleRole::EditorBackground,
            );
            row = row.saturating_add(1);
        }
    }

    fn render_conflict_detail(&self, frame: &mut Framebuffer, area: Rect, mut row: u16) {
        for conflict in &self.conflicts {
            if row >= area.bottom() {
                break;
            }
            let text = format!("{} stages {:?}", conflict.path.display(), conflict.stages);
            let _ = write_text(
                frame,
                area.x.saturating_add(1),
                row,
                &truncate(&text, area.width.saturating_sub(2) as usize),
                StyleRole::Error,
                StyleRole::EditorBackground,
            );
            row = row.saturating_add(1);
        }
    }

    fn render_footer(&self, frame: &mut Framebuffer, area: Option<Rect>) {
        let Some(area) = area else {
            return;
        };
        fill_rect(
            frame,
            area,
            " ",
            StyleRole::EditorText,
            StyleRole::StatusBar,
        );
        let mut line = format!(
            "{} | {} staged, {} unstaged, {} untracked, {} conflicts",
            self.branch_state
                .clone()
                .unwrap_or_else(|| String::from("detached")),
            self.summary.staged,
            self.summary.unstaged,
            self.summary.untracked,
            self.summary.conflicts
        );
        if self.busy.busy {
            if let Some((done, total)) = self.busy.progress {
                let _ = write!(line, " | {} {done}/{total}", self.busy.label);
            } else {
                let _ = write!(line, " | {}", self.busy.label);
            }
        }
        if matches!(self.trust, GitTrustState::Untrusted) {
            line.push_str(" | disabled by policy");
        }
        let _ = write_text(
            frame,
            area.x,
            area.y,
            &truncate(&line, area.width as usize),
            StyleRole::EditorText,
            StyleRole::StatusBar,
        );
    }

    fn render_confirmation(
        frame: &mut Framebuffer,
        area: Option<Rect>,
        confirmation: &GitConfirmation,
    ) {
        let Some(area) = area else {
            return;
        };
        draw_border(frame, area, StyleRole::Warning, StyleRole::EditorBackground);
        let _ = write_text(
            frame,
            area.x.saturating_add(1),
            area.y,
            &truncate(&confirmation.title, area.width.saturating_sub(2) as usize),
            StyleRole::Warning,
            StyleRole::EditorBackground,
        );
        let _ = write_text(
            frame,
            area.x.saturating_add(1),
            area.y.saturating_add(1),
            &truncate(&confirmation.reason, area.width.saturating_sub(2) as usize),
            StyleRole::EditorText,
            StyleRole::EditorBackground,
        );
        let buttons = format!(
            "[{}]  [{}]",
            confirmation.action_label, confirmation.cancel_label
        );
        let _ = write_text(
            frame,
            area.x.saturating_add(1),
            area.bottom().saturating_sub(1),
            &truncate(&buttons, area.width.saturating_sub(2) as usize),
            StyleRole::Panel,
            StyleRole::EditorBackground,
        );
    }

    fn render_output(&self, frame: &mut Framebuffer, area: Rect) {
        draw_border(frame, area, StyleRole::Gutter, StyleRole::EditorBackground);
        let _ = write_text(
            frame,
            area.x.saturating_add(1),
            area.y,
            "Git stderr / output",
            StyleRole::Panel,
            StyleRole::EditorBackground,
        );
        let mut row = area.y.saturating_add(1);
        for line in &self.busy.stderr {
            if row >= area.bottom() {
                break;
            }
            let _ = write_text(
                frame,
                area.x.saturating_add(1),
                row,
                &truncate(line, area.width.saturating_sub(2) as usize),
                StyleRole::Error,
                StyleRole::EditorBackground,
            );
            row = row.saturating_add(1);
        }
        for line in &self.command_log {
            if row >= area.bottom() {
                break;
            }
            let _ = write_text(
                frame,
                area.x.saturating_add(1),
                row,
                &truncate(line, area.width.saturating_sub(2) as usize),
                StyleRole::Information,
                StyleRole::EditorBackground,
            );
            row = row.saturating_add(1);
        }
    }

    fn selected_diff_file(&self) -> Option<&GitDiffFile> {
        self.selected_file
            .and_then(|index| self.diff_files.get(index))
            .or_else(|| self.diff_files.first())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GitAction {
    SwitchView(GitView),
    ToggleChangeMode,
    SelectChange(usize),
    SelectHunk(usize),
    StageFile(PathBuf),
    UnstageFile(PathBuf),
    StageHunk { path: PathBuf, hunk: usize },
    UnstageHunk { path: PathBuf, hunk: usize },
    RequestDiscard(GitDiscardPlan),
    ConfirmDiscard,
    CancelDiscard,
    EditCommitMessage(String),
    ToggleAmend,
    ToggleAllowEmpty,
    Commit,
    CreateBranch(String),
    SwitchBranch(String),
    DeleteBranch { name: String, force: bool },
    Fetch,
    Pull,
    Push,
    CreateStash,
    ApplyStash(String),
    PopStash(String),
    OpenDiffFile(PathBuf),
    OpenConflict(PathBuf),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GitEffectRequest {
    StageFile { path: PathBuf },
    UnstageFile { path: PathBuf },
    StageHunk { path: PathBuf, hunk: usize },
    UnstageHunk { path: PathBuf, hunk: usize },
    PrepareDiscard(GitDiscardPlan),
    Commit(GitCommitRequest),
    CreateBranch { name: String },
    SwitchBranch { name: String },
    DeleteBranch { name: String, force: bool },
    Fetch(GitFetchRequest),
    Pull(GitPullRequest),
    Push(GitPushRequest),
    CreateStash(GitStashCreateRequest),
    ApplyStash { reference: String },
    PopStash { reference: String },
    OpenFileDiff { path: PathBuf },
    OpenConflictFile { path: PathBuf },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitRepositorySnapshot {
    pub summary: GitStatusSummary,
    pub branch_state: Option<String>,
    pub head: Option<String>,
    pub trust: GitTrustState,
    pub changes: Vec<GitStatusEntry>,
    pub diff_files: Vec<GitDiffFile>,
    pub branches: Vec<GitBranchInfo>,
    pub stashes: Vec<GitStashEntry>,
    pub history: Vec<GitLogEntry>,
    pub conflicts: Vec<GitConflictFile>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitChangeGroup {
    pub title: String,
    pub severity: StyleRole,
    pub entries: Vec<GitStatusEntry>,
}

impl GitChangeGroup {
    #[must_use]
    pub fn severity_role(&self) -> StyleRole {
        self.severity
    }
}

fn group_changes(entries: &[GitStatusEntry]) -> Vec<GitChangeGroup> {
    let mut conflicts = Vec::new();
    let mut staged = Vec::new();
    let mut unstaged = Vec::new();
    let mut untracked = Vec::new();
    let mut clean = Vec::new();
    for entry in entries {
        if entry.conflicted {
            conflicts.push(entry.clone());
        } else if entry.staged {
            staged.push(entry.clone());
        } else if entry.unstaged {
            unstaged.push(entry.clone());
        } else if entry.untracked {
            untracked.push(entry.clone());
        } else {
            clean.push(entry.clone());
        }
    }
    let mut groups = Vec::new();
    if !conflicts.is_empty() {
        groups.push(GitChangeGroup {
            title: String::from("Conflicts"),
            severity: StyleRole::Error,
            entries: conflicts,
        });
    }
    if !staged.is_empty() {
        groups.push(GitChangeGroup {
            title: String::from("Staged"),
            severity: StyleRole::GitAdded,
            entries: staged,
        });
    }
    if !unstaged.is_empty() {
        groups.push(GitChangeGroup {
            title: String::from("Unstaged"),
            severity: StyleRole::GitModified,
            entries: unstaged,
        });
    }
    if !untracked.is_empty() {
        groups.push(GitChangeGroup {
            title: String::from("Untracked"),
            severity: StyleRole::Hint,
            entries: untracked,
        });
    }
    if !clean.is_empty() {
        groups.push(GitChangeGroup {
            title: String::from("Clean"),
            severity: StyleRole::EditorText,
            entries: clean,
        });
    }
    groups
}

fn status_tag(entry: &GitStatusEntry) -> &'static str {
    if entry.conflicted {
        "conflict"
    } else if entry.staged {
        "staged"
    } else if entry.unstaged {
        "modified"
    } else if entry.untracked {
        "untracked"
    } else {
        "clean"
    }
}

fn status_role(entry: &GitStatusEntry) -> StyleRole {
    if entry.conflicted {
        StyleRole::Error
    } else if entry.staged {
        StyleRole::GitAdded
    } else if entry.unstaged {
        StyleRole::GitModified
    } else if entry.untracked {
        StyleRole::Hint
    } else {
        StyleRole::EditorText
    }
}

fn diff_role(file: &GitDiffFile) -> StyleRole {
    match file.change {
        GitFileChange::Added | GitFileChange::Copied => StyleRole::GitAdded,
        GitFileChange::Deleted => StyleRole::GitDeleted,
        GitFileChange::Modified | GitFileChange::TypeChanged | GitFileChange::Unknown(_) => {
            StyleRole::GitModified
        }
        GitFileChange::Unmerged => StyleRole::Error,
        GitFileChange::Untracked => StyleRole::Hint,
        GitFileChange::Renamed => StyleRole::Information,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Layout {
    left: Option<Rect>,
    right: Option<Rect>,
    footer: Option<Rect>,
    confirmation: Option<Rect>,
    output: Option<Rect>,
    compact: bool,
}

fn layout(area: Rect, view: GitView) -> Layout {
    let compact = area.width < 100 || area.height < 24;
    let sidebar_width = if compact {
        0
    } else {
        area.width
            .saturating_mul(36)
            .saturating_div(100)
            .clamp(24, 36)
    };
    let footer_height = u16::from(area.height >= 8);
    let output_height = if area.height >= 18
        && matches!(
            view,
            GitView::Commit | GitView::Diff | GitView::Changes | GitView::History
        ) {
        4
    } else {
        0
    };
    let content_height = area
        .height
        .saturating_sub(2 + footer_height + output_height);
    let left = if sidebar_width > 0 {
        Some(Rect::new(
            area.x.saturating_add(1),
            area.y.saturating_add(2),
            sidebar_width.saturating_sub(1),
            content_height,
        ))
    } else {
        None
    };
    let right_x = area.x.saturating_add(1).saturating_add(sidebar_width);
    let right_width = area.width.saturating_sub(1).saturating_sub(sidebar_width);
    let right = Some(Rect::new(
        right_x,
        area.y.saturating_add(2),
        right_width,
        content_height,
    ));
    let footer = footer_height.checked_sub(0).map(|height| {
        Rect::new(
            area.x.saturating_add(1),
            area.bottom().saturating_sub(height),
            area.width.saturating_sub(2),
            height,
        )
    });
    let confirmation = if area.width >= 40 && area.height >= 8 {
        Some(Rect::new(
            area.x.saturating_add(area.width / 6),
            area.y.saturating_add(area.height / 4),
            area.width.saturating_mul(2).saturating_div(3),
            5,
        ))
    } else {
        None
    };
    let output = if output_height > 0 {
        Some(Rect::new(
            right_x,
            area.bottom().saturating_sub(footer_height + output_height),
            right_width,
            output_height,
        ))
    } else {
        None
    };
    Layout {
        left,
        right,
        footer,
        confirmation,
        output,
        compact,
    }
}

fn summary_line(state: &GitDashboardState) -> String {
    let mut line = format!(
        "{}{} | branch: {} | head: {} | staged {} unstaged {} untracked {} conflicts {}",
        state.root.display(),
        if matches!(state.trust, GitTrustState::Untrusted) {
            " | blocked by policy"
        } else {
            ""
        },
        state
            .branch_state
            .clone()
            .unwrap_or_else(|| String::from("detached")),
        state
            .head
            .clone()
            .unwrap_or_else(|| String::from("unknown")),
        state.summary.staged,
        state.summary.unstaged,
        state.summary.untracked,
        state.summary.conflicts
    );
    if state.busy.busy {
        if let Some((done, total)) = state.busy.progress {
            let _ = write!(line, " | {} {done}/{total}", state.busy.label);
        } else {
            let _ = write!(line, " | {}", state.busy.label);
        }
    }
    line
}

fn range_label(range: (usize, usize)) -> String {
    format!("{}..{}", range.0, range.1)
}

fn yes_no(value: bool) -> &'static str {
    if value { "yes" } else { "no" }
}

fn short_hash(hash: &str) -> String {
    hash.chars().take(8).collect()
}

fn message_lines(message: &str) -> Vec<String> {
    if message.is_empty() {
        vec![String::from("<empty commit message>")]
    } else {
        message.lines().map(ToOwned::to_owned).collect()
    }
}

fn diff_change_label(change: &GitFileChange) -> &'static str {
    match change {
        GitFileChange::Added => "added",
        GitFileChange::Deleted => "deleted",
        GitFileChange::Modified => "modified",
        GitFileChange::Untracked => "untracked",
        GitFileChange::Renamed => "renamed",
        GitFileChange::Copied => "copied",
        GitFileChange::TypeChanged => "type changed",
        GitFileChange::Unmerged => "unmerged",
        GitFileChange::Unknown(_) => "unknown",
    }
}

fn truncate(text: &str, width: usize) -> String {
    if width == 0 {
        return String::new();
    }
    let count = text.chars().count();
    if count <= width {
        return text.to_owned();
    }
    if width == 1 {
        return String::from("…");
    }
    let mut output = String::new();
    for character in text.chars().take(width.saturating_sub(1)) {
        output.push(character);
    }
    output.push('…');
    output
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rect {
    pub x: u16,
    pub y: u16,
    pub width: u16,
    pub height: u16,
}

impl Rect {
    #[must_use]
    pub const fn new(x: u16, y: u16, width: u16, height: u16) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    #[must_use]
    pub const fn right(self) -> u16 {
        self.x.saturating_add(self.width)
    }

    #[must_use]
    pub const fn bottom(self) -> u16 {
        self.y.saturating_add(self.height)
    }

    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.width == 0 || self.height == 0
    }
}

#[must_use]
pub fn cell(
    symbol: impl Into<String>,
    foreground: StyleRole,
    background: StyleRole,
    bold: bool,
) -> Cell {
    Cell {
        symbol: symbol.into(),
        foreground,
        background,
        bold,
        continuation: false,
    }
}

pub fn fill_rect(
    frame: &mut Framebuffer,
    rect: Rect,
    symbol: &str,
    foreground: StyleRole,
    background: StyleRole,
) {
    for row in rect.y..rect.bottom() {
        for column in rect.x..rect.right() {
            let _ = frame.set(
                column,
                row,
                Cell {
                    symbol: symbol.to_owned(),
                    foreground,
                    background,
                    bold: false,
                    continuation: false,
                },
            );
        }
    }
}

pub fn draw_border(
    frame: &mut Framebuffer,
    rect: Rect,
    foreground: StyleRole,
    background: StyleRole,
) {
    if rect.width == 0 || rect.height == 0 {
        return;
    }
    let last_column = rect.right().saturating_sub(1);
    let last_row = rect.bottom().saturating_sub(1);
    let _ = frame.set(rect.x, rect.y, cell("┌", foreground, background, false));
    let _ = frame.set(
        last_column,
        rect.y,
        cell("┐", foreground, background, false),
    );
    let _ = frame.set(rect.x, last_row, cell("└", foreground, background, false));
    let _ = frame.set(
        last_column,
        last_row,
        cell("┘", foreground, background, false),
    );
    for column in rect.x.saturating_add(1)..last_column {
        let _ = frame.set(column, rect.y, cell("─", foreground, background, false));
        let _ = frame.set(column, last_row, cell("─", foreground, background, false));
    }
    for row in rect.y.saturating_add(1)..last_row {
        let _ = frame.set(rect.x, row, cell("│", foreground, background, false));
        let _ = frame.set(last_column, row, cell("│", foreground, background, false));
    }
}

#[must_use]
pub fn write_text(
    frame: &mut Framebuffer,
    column: u16,
    row: u16,
    text: &str,
    foreground: StyleRole,
    background: StyleRole,
) -> u16 {
    let mut current = column;
    for character in text.chars() {
        if frame
            .set(
                current,
                row,
                Cell {
                    symbol: character.to_string(),
                    foreground,
                    background,
                    bold: false,
                    continuation: false,
                },
            )
            .is_err()
        {
            break;
        }
        current = current.saturating_add(1);
    }
    current
}

/// Returns a stable text dump of the framebuffer.
///
/// # Panics
///
/// Panics if any snapshot coordinate falls outside the framebuffer bounds.
#[must_use]
pub fn frame_snapshot(frame: &Framebuffer) -> String {
    let (columns, rows) = frame.size();
    let mut output = String::new();
    let _ = writeln!(output, "[{columns}x{rows}]");
    for row in 0..rows {
        let _ = write!(output, "{row:02}|");
        for column in 0..columns {
            let cell = frame.get(column, row).expect("frame coordinates are valid");
            if cell.symbol == " "
                && cell.foreground == StyleRole::EditorText
                && cell.background == StyleRole::EditorBackground
                && !cell.bold
            {
                output.push('·');
                continue;
            }
            output.push('<');
            output.push_str(role_name(cell.foreground));
            output.push('/');
            output.push_str(role_name(cell.background));
            if cell.bold {
                output.push('!');
            }
            output.push('>');
            output.push_str(&cell.symbol);
            output.push_str("</>");
        }
        if row + 1 != rows {
            output.push('\n');
        }
    }
    output
}

fn role_name(role: StyleRole) -> &'static str {
    match role {
        StyleRole::EditorText => "EditorText",
        StyleRole::EditorBackground => "EditorBackground",
        StyleRole::Selection => "Selection",
        StyleRole::CurrentLine => "CurrentLine",
        StyleRole::LineNumber => "LineNumber",
        StyleRole::Gutter => "Gutter",
        StyleRole::StatusBar => "StatusBar",
        StyleRole::Panel => "Panel",
        StyleRole::Error => "Error",
        StyleRole::Warning => "Warning",
        StyleRole::Information => "Information",
        StyleRole::Hint => "Hint",
        StyleRole::GitAdded => "GitAdded",
        StyleRole::GitModified => "GitModified",
        StyleRole::GitDeleted => "GitDeleted",
        StyleRole::SearchMatch => "SearchMatch",
        StyleRole::SyntaxKeyword => "SyntaxKeyword",
        StyleRole::SyntaxString => "SyntaxString",
        StyleRole::SyntaxComment => "SyntaxComment",
        StyleRole::SemanticType => "SemanticType",
        StyleRole::CurrentLineBackground => "CurrentLineBackground",
        StyleRole::SelectionForeground => "SelectionForeground",
        StyleRole::SelectionBackground => "SelectionBackground",
        StyleRole::SecondaryCursorForeground => "SecondaryCursorForeground",
        StyleRole::SecondaryCursorBackground => "SecondaryCursorBackground",
        StyleRole::LineNumberActive => "LineNumberActive",
        StyleRole::SplitSeparator => "SplitSeparator",
        StyleRole::MenuForeground => "MenuForeground",
        StyleRole::MenuBackground => "MenuBackground",
        StyleRole::MenuSelectedForeground => "MenuSelectedForeground",
        StyleRole::MenuSelectedBackground => "MenuSelectedBackground",
        StyleRole::ExplorerForeground => "ExplorerForeground",
        StyleRole::ExplorerBackground => "ExplorerBackground",
        StyleRole::ExplorerDirectory => "ExplorerDirectory",
        StyleRole::ExplorerSelectedForeground => "ExplorerSelectedForeground",
        StyleRole::ExplorerSelectedBackground => "ExplorerSelectedBackground",
        StyleRole::TabForeground => "TabForeground",
        StyleRole::TabBackground => "TabBackground",
        StyleRole::TabActiveForeground => "TabActiveForeground",
        StyleRole::TabActiveBackground => "TabActiveBackground",
        StyleRole::TabPreviewForeground => "TabPreviewForeground",
        StyleRole::TabPinnedForeground => "TabPinnedForeground",
        StyleRole::PanelForeground => "PanelForeground",
        StyleRole::PanelBackground => "PanelBackground",
        StyleRole::PanelTitle => "PanelTitle",
        StyleRole::InputForeground => "InputForeground",
        StyleRole::InputBackground => "InputBackground",
        StyleRole::StatusForeground => "StatusForeground",
        StyleRole::StatusBackground => "StatusBackground",
        StyleRole::Border => "Border",
    }
}

#[cfg(test)]
mod tests {
    use std::{fs, path::PathBuf};

    use super::*;

    fn repo_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("crates dir")
            .parent()
            .expect("repo root")
            .to_path_buf()
    }

    fn fixture(name: &str) -> String {
        fs::read_to_string(repo_root().join("tests/fixtures/app-ui/git").join(name))
            .expect("fixture should exist")
            .replace("\r\n", "\n")
    }

    fn snapshot(name: &str) -> String {
        fs::read_to_string(repo_root().join("tests/snapshots/ui-git").join(name))
            .expect("snapshot should exist")
            .replace("\r\n", "\n")
            .trim_end()
            .to_owned()
    }

    fn snapshot_path(name: &str) -> PathBuf {
        repo_root().join("tests/snapshots/ui-git").join(name)
    }

    fn maybe_write_snapshot(name: &str, rendered: &str) {
        if std::env::var_os("WRITE_GIT_SNAPSHOTS").is_some() {
            fs::write(snapshot_path(name), rendered).expect("write snapshot");
        }
    }

    fn plain_text(snapshot: &str) -> String {
        let mut output = String::new();
        let mut chars = snapshot.chars().peekable();
        while let Some(character) = chars.next() {
            if character == '<' {
                for next in chars.by_ref() {
                    if next == '>' {
                        break;
                    }
                }
                if chars.peek() == Some(&'<') {
                    continue;
                }
                continue;
            }
            if character == '·' {
                output.push(' ');
            } else if character != '\n' {
                output.push(character);
            } else {
                output.push('\n');
            }
        }
        output
    }

    #[allow(clippy::needless_pass_by_value, clippy::fn_params_excessive_bools)]
    fn file_change(
        path: &str,
        change: GitFileChange,
        staged: bool,
        unstaged: bool,
        untracked: bool,
        conflicted: bool,
    ) -> GitStatusEntry {
        GitStatusEntry {
            status: format!("{change:?}"),
            path: PathBuf::from(path),
            previous_path: None,
            staged,
            unstaged,
            untracked,
            conflicted,
        }
    }

    fn sample_diff_file(
        path: &str,
        change: GitFileChange,
        header: &str,
        lines: Vec<GitDiffLine>,
    ) -> GitDiffFile {
        GitDiffFile {
            change,
            path: PathBuf::from(path),
            previous_path: None,
            binary: false,
            hunks: vec![GitDiffHunk {
                header: header.to_owned(),
                old_range: (1, 3),
                new_range: (1, 4),
                lines,
                patch: String::new(),
            }],
            status: String::from("MM"),
        }
    }

    #[allow(clippy::too_many_lines)]
    fn sample_state() -> GitDashboardState {
        GitDashboardState::from_repository(
            "C:/work/editor",
            GitRepositorySnapshot {
                summary: GitStatusSummary {
                    branch: Some(String::from("main")),
                    staged: 2,
                    unstaged: 1,
                    untracked: 1,
                    conflicts: 1,
                    busy: false,
                },
                branch_state: Some(String::from("main")),
                head: Some(String::from("abc1234")),
                trust: GitTrustState::Trusted,
                changes: vec![
                    file_change(
                        "src/lib.rs",
                        GitFileChange::Modified,
                        true,
                        false,
                        false,
                        false,
                    ),
                    file_change(
                        "src/main.rs",
                        GitFileChange::Modified,
                        false,
                        true,
                        false,
                        false,
                    ),
                    file_change(
                        "README.md",
                        GitFileChange::Untracked,
                        false,
                        false,
                        true,
                        false,
                    ),
                    file_change(
                        "src/conflict.rs",
                        GitFileChange::Unmerged,
                        false,
                        false,
                        false,
                        true,
                    ),
                ],
                diff_files: vec![sample_diff_file(
                    "src/main.rs",
                    GitFileChange::Modified,
                    "@@ -1,3 +1,4 @@",
                    vec![
                        GitDiffLine {
                            kind: GitDiffLineKind::Context,
                            text: String::from("fn main() {"),
                        },
                        GitDiffLine {
                            kind: GitDiffLineKind::Removal,
                            text: String::from("    println!(\"old\");"),
                        },
                        GitDiffLine {
                            kind: GitDiffLineKind::Addition,
                            text: String::from("    println!(\"new\");"),
                        },
                        GitDiffLine {
                            kind: GitDiffLineKind::Meta,
                            text: String::from("}"),
                        },
                    ],
                )],
                branches: vec![
                    GitBranchInfo {
                        name: String::from("main"),
                        oid: String::from("abc1234"),
                        upstream: Some(String::from("origin/main")),
                        tracking: Some(String::from("ahead 1")),
                        head: true,
                    },
                    GitBranchInfo {
                        name: String::from("feature/ui"),
                        oid: String::from("def5678"),
                        upstream: Some(String::from("origin/feature/ui")),
                        tracking: Some(String::from("behind 2")),
                        head: false,
                    },
                ],
                stashes: vec![GitStashEntry {
                    reference: String::from("stash@{0}"),
                    hash: String::from("feedbeef"),
                    message: String::from("wip: layout tweak"),
                }],
                history: vec![GitLogEntry {
                    hash: String::from("abc1234"),
                    parents: vec![String::from("2222222")],
                    author_name: String::from("Editor Bot"),
                    author_email: String::from("bot@example.com"),
                    authored_at: String::from("2026-09-05"),
                    summary: String::from("Add source control UI"),
                    body: String::from("Implement pure view layer"),
                }],
                conflicts: vec![GitConflictFile {
                    path: PathBuf::from("src/conflict.rs"),
                    stages: vec![1, 2, 3],
                }],
            },
        )
    }

    fn render(state: &GitDashboardState, width: u16, height: u16) -> String {
        state.render_snapshot(width, height)
    }

    #[test]
    fn clean_repo_snapshot_is_compact() {
        let mut state = GitDashboardState::from_repository(
            "C:/work/editor",
            GitRepositorySnapshot {
                summary: GitStatusSummary {
                    branch: Some(String::from("main")),
                    staged: 0,
                    unstaged: 0,
                    untracked: 0,
                    conflicts: 0,
                    busy: false,
                },
                branch_state: Some(String::from("main")),
                head: Some(String::from("abc1234")),
                trust: GitTrustState::Trusted,
                changes: Vec::new(),
                diff_files: Vec::new(),
                branches: Vec::new(),
                stashes: Vec::new(),
                history: Vec::new(),
                conflicts: Vec::new(),
            },
        );
        state.view = GitView::Changes;
        let rendered = render(&state, 80, 24);
        maybe_write_snapshot("clean_repo.txt", &rendered);
        assert_eq!(rendered, snapshot("clean_repo.txt"));
    }

    #[test]
    fn mixed_change_groups_render() {
        let state = sample_state();
        let rendered = render(&state, 120, 40);
        maybe_write_snapshot("mixed_changes.txt", &rendered);
        assert_eq!(rendered, snapshot("mixed_changes.txt"));
    }

    #[test]
    fn staged_and_unstaged_transitions_are_visible() {
        let mut staged = sample_state();
        staged.view = GitView::Changes;
        staged.change_mode = GitChangeMode::Index;
        let staged_render = render(&staged, 96, 24);
        assert!(plain_text(&staged_render).contains("view: Index"));
        let mut unstaged = staged.clone();
        unstaged.change_mode = GitChangeMode::WorkingTree;
        assert_ne!(staged_render, render(&unstaged, 96, 24));
    }

    #[test]
    fn diff_and_hunk_selection_are_rendered() {
        let mut state = sample_state();
        state.view = GitView::Diff;
        state.selected_file = Some(0);
        state.selected_hunk = Some(0);
        let rendered = render(&state, 120, 36);
        maybe_write_snapshot("diff_view.txt", &rendered);
        assert_eq!(rendered, snapshot("diff_view.txt"));
    }

    #[test]
    fn commit_form_and_branches_and_stashes_render() {
        let mut state = sample_state();
        state.view = GitView::Commit;
        state.commit_form.message = fixture("commit_message.txt").trim().to_owned();
        state.commit_form.amend = true;
        state.commit_form.allow_empty = false;
        let rendered = render(&state, 80, 24);
        maybe_write_snapshot("commit_form.txt", &rendered);
        assert_eq!(rendered, snapshot("commit_form.txt"));

        state.view = GitView::Branches;
        let rendered = render(&state, 80, 24);
        maybe_write_snapshot("branch_chooser.txt", &rendered);
        assert_eq!(rendered, snapshot("branch_chooser.txt"));

        state.view = GitView::Stashes;
        let rendered = render(&state, 80, 24);
        maybe_write_snapshot("stash_list.txt", &rendered);
        assert_eq!(rendered, snapshot("stash_list.txt"));
    }

    #[test]
    fn history_and_conflicts_render() {
        let mut state = sample_state();
        state.view = GitView::History;
        let rendered = render(&state, 80, 24);
        maybe_write_snapshot("history.txt", &rendered);
        assert_eq!(rendered, snapshot("history.txt"));
        state.view = GitView::Conflicts;
        let rendered = render(&state, 80, 24);
        maybe_write_snapshot("conflicts.txt", &rendered);
        assert_eq!(rendered, snapshot("conflicts.txt"));
    }

    #[test]
    fn command_failure_and_policy_state_render() {
        let mut state = sample_state();
        state.busy.busy = true;
        state.busy.label = String::from("fetching origin");
        state.busy.progress = Some((2, 5));
        state.busy.stderr = vec![String::from("network failure"), String::from("retry later")];
        state.command_log = vec![String::from("command failed with exit 128")];
        let rendered = render(&state, 120, 32);
        maybe_write_snapshot("command_failure.txt", &rendered);
        let text = plain_text(&rendered);
        assert!(text.contains("network failure"));
        assert!(text.contains("fetching origin"));
        assert!(text.contains("command failed with exit 128"));

        let untrusted = GitDashboardState::disabled_by_policy("C:/work/editor");
        let rendered = render(&untrusted, 96, 24);
        maybe_write_snapshot("untrusted_disabled.txt", &rendered);
        let text = plain_text(&rendered);
        assert!(text.contains("blocked by policy"));
        assert!(text.contains("workspace trust gate active"));
    }

    #[test]
    fn narrow_layout_still_shows_context() {
        let state = sample_state();
        let snapshot = render(&state, 70, 18);
        let text = plain_text(&snapshot);
        assert!(text.contains("Source Control"));
        assert!(text.contains("main"));
    }

    #[test]
    fn dispatch_returns_typed_requests_only() {
        let mut state = sample_state();
        let effect = state.dispatch(GitAction::Commit);
        match effect {
            Some(GitEffectRequest::Commit(request)) => {
                assert_eq!(request.message, "");
            }
            other => panic!("unexpected effect: {other:?}"),
        }
        assert!(
            state
                .dispatch(GitAction::SwitchView(GitView::Branches))
                .is_none()
        );
    }
}
