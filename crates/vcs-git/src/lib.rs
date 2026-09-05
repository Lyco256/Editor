#![allow(
    clippy::missing_errors_doc,
    clippy::module_name_repetitions,
    clippy::similar_names,
    clippy::struct_excessive_bools,
    clippy::too_many_lines
)]

//! Structured Git process adapter boundary.

use std::{
    ffi::{OsStr, OsString},
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

use editor_types::GitStatusSummary;
use thiserror::Error;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    process::Command,
    sync::Notify,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiffTarget {
    WorkingTree,
    Index,
}

#[derive(Debug, Clone, Default)]
pub struct CancellationToken {
    cancelled: Arc<AtomicBool>,
    notify: Arc<Notify>,
}

impl CancellationToken {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::SeqCst);
        self.notify.notify_waiters();
    }

    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::SeqCst)
    }

    async fn wait(&self) {
        if self.is_cancelled() {
            return;
        }
        self.notify.notified().await;
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitInvocation {
    pub cwd: PathBuf,
    pub args: Vec<OsString>,
}

impl GitInvocation {
    #[must_use]
    pub fn new<C, I, S>(cwd: C, args: I) -> Self
    where
        C: Into<PathBuf>,
        I: IntoIterator<Item = S>,
        S: Into<OsString>,
    {
        Self {
            cwd: cwd.into(),
            args: args.into_iter().map(Into::into).collect(),
        }
    }
}

#[derive(Debug)]
pub struct GitClient {
    executable: OsString,
}

impl Default for GitClient {
    fn default() -> Self {
        Self::new("git")
    }
}

impl GitClient {
    #[must_use]
    pub fn new(executable: impl Into<OsString>) -> Self {
        Self {
            executable: executable.into(),
        }
    }

    pub fn discover() -> Result<Self, GitError> {
        let executable = OsString::from("git");
        let output = std::process::Command::new(&executable)
            .arg("--version")
            .output();
        match output {
            Ok(output) if output.status.success() => Ok(Self { executable }),
            Ok(output) => Err(GitError::CommandFailed {
                command: "git --version".to_owned(),
                code: output.status.code(),
                stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
            }),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                Err(GitError::ExecutableNotFound("git".to_owned()))
            }
            Err(error) => Err(GitError::Io(error)),
        }
    }

    #[must_use]
    pub fn executable(&self) -> &OsStr {
        &self.executable
    }

    pub async fn repository_root(
        &self,
        path: impl AsRef<Path>,
        cancellation: Option<&CancellationToken>,
    ) -> Result<Option<PathBuf>, GitError> {
        let cwd = command_cwd(path.as_ref());
        let invocation = GitInvocation::new(&cwd, ["rev-parse", "--show-toplevel"]);
        let output = self.run_command(&invocation, None, cancellation).await?;
        if output.status.success() {
            let text = stdout_string(&output.stdout)?;
            return Ok(Some(PathBuf::from(text.trim())));
        }
        let stderr = stderr_string(&output.stderr);
        if stderr.contains("not a git repository") || stderr.contains("fatal: not a git repository")
        {
            return Ok(None);
        }
        Err(GitError::from_output(&invocation, &output))
    }

    pub async fn branch_state(
        &self,
        repository: impl AsRef<Path>,
        cancellation: Option<&CancellationToken>,
    ) -> Result<GitBranchState, GitError> {
        let root = self
            .require_repository_root(repository.as_ref(), cancellation)
            .await?;
        let symbolic = self
            .run_git(
                &root,
                ["symbolic-ref", "--quiet", "--short", "HEAD"],
                None,
                cancellation,
            )
            .await?;
        if symbolic.status.success() {
            let branch = stdout_string(&symbolic.stdout)?.trim().to_owned();
            let head = self.rev_parse_head(&root, cancellation).await.ok();
            return Ok(GitBranchState {
                branch: Some(branch),
                detached_head: false,
                head,
            });
        }
        let head = self.rev_parse_head(&root, cancellation).await.ok();
        Ok(GitBranchState {
            branch: None,
            detached_head: true,
            head,
        })
    }

    pub async fn status(
        &self,
        repository: impl AsRef<Path>,
        cancellation: Option<&CancellationToken>,
    ) -> Result<GitRepositoryStatus, GitError> {
        let root = self
            .require_repository_root(repository.as_ref(), cancellation)
            .await?;
        let branch_state = self.branch_state(&root, cancellation).await?;
        let invocation = GitInvocation::new(
            &root,
            [
                "status",
                "--porcelain=v1",
                "--branch",
                "--untracked-files=all",
            ],
        );
        let output = self.run_command(&invocation, None, cancellation).await?;
        let output = self.ensure_success(&invocation, output)?;
        let text = stdout_string(&output.stdout)?;
        let mut summary = GitStatusSummary {
            branch: branch_state.branch.clone(),
            busy: false,
            ..GitStatusSummary::default()
        };
        let mut entries = Vec::new();
        let mut conflicts = Vec::new();
        for line in text.lines() {
            if line.starts_with("## ") {
                continue;
            }
            if line.is_empty() {
                continue;
            }
            let entry = parse_status_line(line)?;
            if entry.untracked {
                summary.untracked = summary.untracked.saturating_add(1);
            }
            if entry.staged {
                summary.staged = summary.staged.saturating_add(1);
            }
            if entry.unstaged {
                summary.unstaged = summary.unstaged.saturating_add(1);
            }
            if entry.conflicted {
                summary.conflicts = summary.conflicts.saturating_add(1);
                conflicts.push(entry.path.clone());
            }
            entries.push(entry);
        }
        Ok(GitRepositoryStatus {
            root,
            summary,
            branch_state,
            entries,
            conflicts,
        })
    }

    pub async fn diff(
        &self,
        repository: impl AsRef<Path>,
        target: DiffTarget,
        paths: &[PathBuf],
        cancellation: Option<&CancellationToken>,
    ) -> Result<Vec<GitDiffFile>, GitError> {
        let root = self
            .require_repository_root(repository.as_ref(), cancellation)
            .await?;
        let mut name_status_args = vec![
            OsString::from("diff"),
            OsString::from("--name-status"),
            OsString::from("-z"),
        ];
        if matches!(target, DiffTarget::Index) {
            name_status_args.insert(1, OsString::from("--cached"));
        }
        append_pathspecs(&mut name_status_args, &root, paths);
        let name_status = GitInvocation::new(&root, name_status_args);
        let name_status_output = self.run_command(&name_status, None, cancellation).await?;
        let name_status_output = self.ensure_success(&name_status, name_status_output)?;

        let mut patch_args = vec![
            OsString::from("diff"),
            OsString::from("--no-color"),
            OsString::from("--no-ext-diff"),
            OsString::from("--unified=0"),
            OsString::from("--find-renames"),
            OsString::from("--find-copies"),
        ];
        if matches!(target, DiffTarget::Index) {
            patch_args.insert(1, OsString::from("--cached"));
        }
        append_pathspecs(&mut patch_args, &root, paths);
        let patch = GitInvocation::new(&root, patch_args);
        let patch_output = self.run_command(&patch, None, cancellation).await?;
        let patch_output = self.ensure_success(&patch, patch_output)?;

        let mut files = parse_name_status_diff(&name_status_output.stdout)?;
        let patch_files = parse_patch_files(&stdout_string(&patch_output.stdout)?)?;
        if files.len() != patch_files.len() {
            return Err(GitError::Parse {
                context: "diff file count",
                message: format!(
                    "name-status returned {} files but patch returned {} files",
                    files.len(),
                    patch_files.len()
                ),
            });
        }
        for (file, patch_file) in files.iter_mut().zip(patch_files) {
            file.binary = patch_file.binary;
            file.hunks = patch_file.hunks;
        }
        Ok(files)
    }

    pub async fn stage_file(
        &self,
        repository: impl AsRef<Path>,
        path: impl AsRef<Path>,
        cancellation: Option<&CancellationToken>,
    ) -> Result<GitCommandOutput, GitError> {
        let root = self
            .require_repository_root(repository.as_ref(), cancellation)
            .await?;
        let invocation = GitInvocation::new(
            &root,
            vec![
                OsString::from("add"),
                OsString::from("--"),
                pathspec_for(&root, path.as_ref()).into_os_string(),
            ],
        );
        let output = self.run_command(&invocation, None, cancellation).await?;
        self.ensure_success(&invocation, output)
    }

    pub async fn unstage_file(
        &self,
        repository: impl AsRef<Path>,
        path: impl AsRef<Path>,
        cancellation: Option<&CancellationToken>,
    ) -> Result<GitCommandOutput, GitError> {
        let root = self
            .require_repository_root(repository.as_ref(), cancellation)
            .await?;
        let invocation = GitInvocation::new(
            &root,
            vec![
                OsString::from("restore"),
                OsString::from("--staged"),
                OsString::from("--"),
                pathspec_for(&root, path.as_ref()).into_os_string(),
            ],
        );
        let output = self.run_command(&invocation, None, cancellation).await?;
        self.ensure_success(&invocation, output)
    }

    pub async fn stage_hunk(
        &self,
        repository: impl AsRef<Path>,
        hunk: &GitDiffHunk,
        cancellation: Option<&CancellationToken>,
    ) -> Result<GitCommandOutput, GitError> {
        let root = self
            .require_repository_root(repository.as_ref(), cancellation)
            .await?;
        let invocation = GitInvocation::new(
            &root,
            ["apply", "--cached", "--unidiff-zero", "--whitespace=nowarn"],
        );
        let output = self
            .run_command(&invocation, Some(hunk.patch.as_bytes()), cancellation)
            .await?;
        self.ensure_success(&invocation, output)
    }

    pub async fn unstage_hunk(
        &self,
        repository: impl AsRef<Path>,
        hunk: &GitDiffHunk,
        cancellation: Option<&CancellationToken>,
    ) -> Result<GitCommandOutput, GitError> {
        let root = self
            .require_repository_root(repository.as_ref(), cancellation)
            .await?;
        let invocation = GitInvocation::new(
            &root,
            [
                "apply",
                "--cached",
                "--reverse",
                "--unidiff-zero",
                "--whitespace=nowarn",
            ],
        );
        let output = self
            .run_command(&invocation, Some(hunk.patch.as_bytes()), cancellation)
            .await?;
        self.ensure_success(&invocation, output)
    }

    pub async fn execute_discard(
        &self,
        repository: impl AsRef<Path>,
        plan: &GitDiscardPlan,
        confirmed: bool,
        cancellation: Option<&CancellationToken>,
    ) -> Result<GitCommandOutput, GitError> {
        if plan.requires_confirmation && !confirmed {
            return Err(GitError::ConfirmationRequired(plan.reason.clone()));
        }
        let root = self
            .require_repository_root(repository.as_ref(), cancellation)
            .await?;
        match (plan.scope, plan.target) {
            (GitDiscardScope::File, DiffTarget::WorkingTree) => {
                if plan.untracked {
                    let invocation = GitInvocation::new(
                        &root,
                        vec![
                            OsString::from("clean"),
                            OsString::from("-fd"),
                            OsString::from("--"),
                            pathspec_for(&root, &plan.path).into_os_string(),
                        ],
                    );
                    let output = self.run_command(&invocation, None, cancellation).await?;
                    self.ensure_success(&invocation, output)
                } else {
                    let invocation = GitInvocation::new(
                        &root,
                        vec![
                            OsString::from("restore"),
                            OsString::from("--worktree"),
                            OsString::from("--source=HEAD"),
                            OsString::from("--"),
                            pathspec_for(&root, &plan.path).into_os_string(),
                        ],
                    );
                    let output = self.run_command(&invocation, None, cancellation).await?;
                    self.ensure_success(&invocation, output)
                }
            }
            (GitDiscardScope::File, DiffTarget::Index) => {
                let invocation = GitInvocation::new(
                    &root,
                    vec![
                        OsString::from("restore"),
                        OsString::from("--staged"),
                        OsString::from("--source=HEAD"),
                        OsString::from("--"),
                        pathspec_for(&root, &plan.path).into_os_string(),
                    ],
                );
                let output = self.run_command(&invocation, None, cancellation).await?;
                self.ensure_success(&invocation, output)
            }
            (GitDiscardScope::Hunk, DiffTarget::WorkingTree) => {
                let patch = plan.hunk.as_ref().ok_or_else(|| GitError::Parse {
                    context: "discard hunk",
                    message: "missing hunk patch".to_owned(),
                })?;
                let invocation = GitInvocation::new(
                    &root,
                    [
                        "apply",
                        "--reverse",
                        "--unidiff-zero",
                        "--whitespace=nowarn",
                    ],
                );
                let output = self
                    .run_command(&invocation, Some(patch.patch.as_bytes()), cancellation)
                    .await?;
                self.ensure_success(&invocation, output)
            }
            (GitDiscardScope::Hunk, DiffTarget::Index) => {
                let patch = plan.hunk.as_ref().ok_or_else(|| GitError::Parse {
                    context: "discard hunk",
                    message: "missing hunk patch".to_owned(),
                })?;
                let invocation = GitInvocation::new(
                    &root,
                    [
                        "apply",
                        "--cached",
                        "--reverse",
                        "--unidiff-zero",
                        "--whitespace=nowarn",
                    ],
                );
                let output = self
                    .run_command(&invocation, Some(patch.patch.as_bytes()), cancellation)
                    .await?;
                self.ensure_success(&invocation, output)
            }
        }
    }

    pub async fn commit(
        &self,
        repository: impl AsRef<Path>,
        request: &GitCommitRequest,
        cancellation: Option<&CancellationToken>,
    ) -> Result<GitCommandOutput, GitError> {
        let root = self
            .require_repository_root(repository.as_ref(), cancellation)
            .await?;
        let mut args = vec![
            OsString::from("commit"),
            OsString::from("-m"),
            OsString::from(&request.message),
        ];
        if request.amend {
            args.insert(1, OsString::from("--amend"));
        }
        if request.allow_empty {
            args.insert(1, OsString::from("--allow-empty"));
        }
        if let Some(author) = &request.author_name {
            args.push(OsString::from("--author"));
            args.push(OsString::from(author));
        }
        let invocation = GitInvocation::new(&root, args);
        let output = self.run_command(&invocation, None, cancellation).await?;
        self.ensure_success(&invocation, output)
    }

    pub async fn branches(
        &self,
        repository: impl AsRef<Path>,
        cancellation: Option<&CancellationToken>,
    ) -> Result<Vec<GitBranchInfo>, GitError> {
        let root = self
            .require_repository_root(repository.as_ref(), cancellation)
            .await?;
        let invocation = GitInvocation::new(
            &root,
            [
                "for-each-ref",
                "--format=%(refname:short)\t%(objectname:short)\t%(upstream:short)\t%(upstream:trackshort)\t%(HEAD)",
                "refs/heads",
            ],
        );
        let output = self.run_command(&invocation, None, cancellation).await?;
        let output = self.ensure_success(&invocation, output)?;
        let text = stdout_string(&output.stdout)?;
        let mut branches = Vec::new();
        for line in text.lines() {
            if line.is_empty() {
                continue;
            }
            let mut parts = line.split('\t');
            let Some(name) = parts.next() else {
                return Err(GitError::Parse {
                    context: "branch list",
                    message: "missing branch name".to_owned(),
                });
            };
            let Some(oid) = parts.next() else {
                return Err(GitError::Parse {
                    context: "branch list",
                    message: "missing branch oid".to_owned(),
                });
            };
            let upstream = parts
                .next()
                .and_then(|value| (!value.is_empty()).then(|| value.to_owned()));
            let track = parts
                .next()
                .and_then(|value| (!value.is_empty()).then(|| value.to_owned()));
            let head = matches!(parts.next(), Some("*"));
            branches.push(GitBranchInfo {
                name: name.to_owned(),
                oid: oid.to_owned(),
                upstream,
                tracking: track,
                head,
            });
        }
        Ok(branches)
    }

    pub async fn branch_create(
        &self,
        repository: impl AsRef<Path>,
        name: impl AsRef<OsStr>,
        start_point: Option<&OsStr>,
        cancellation: Option<&CancellationToken>,
    ) -> Result<GitCommandOutput, GitError> {
        let root = self
            .require_repository_root(repository.as_ref(), cancellation)
            .await?;
        let mut args = vec![OsString::from("branch"), name.as_ref().to_os_string()];
        if let Some(start_point) = start_point {
            args.push(start_point.to_os_string());
        }
        let invocation = GitInvocation::new(&root, args);
        let output = self.run_command(&invocation, None, cancellation).await?;
        self.ensure_success(&invocation, output)
    }

    pub async fn branch_switch(
        &self,
        repository: impl AsRef<Path>,
        name: impl AsRef<OsStr>,
        create: bool,
        cancellation: Option<&CancellationToken>,
    ) -> Result<GitCommandOutput, GitError> {
        let root = self
            .require_repository_root(repository.as_ref(), cancellation)
            .await?;
        let mut args = vec![OsString::from("switch")];
        if create {
            args.push(OsString::from("-c"));
        }
        args.push(name.as_ref().to_os_string());
        let invocation = GitInvocation::new(&root, args);
        let output = self.run_command(&invocation, None, cancellation).await?;
        self.ensure_success(&invocation, output)
    }

    pub async fn branch_delete(
        &self,
        repository: impl AsRef<Path>,
        name: impl AsRef<OsStr>,
        force: bool,
        cancellation: Option<&CancellationToken>,
    ) -> Result<GitCommandOutput, GitError> {
        let root = self
            .require_repository_root(repository.as_ref(), cancellation)
            .await?;
        let mut args = vec![OsString::from("branch")];
        args.push(if force {
            OsString::from("-D")
        } else {
            OsString::from("-d")
        });
        args.push(name.as_ref().to_os_string());
        let invocation = GitInvocation::new(&root, args);
        let output = self.run_command(&invocation, None, cancellation).await?;
        self.ensure_success(&invocation, output)
    }

    pub async fn fetch(
        &self,
        repository: impl AsRef<Path>,
        request: &GitFetchRequest,
        cancellation: Option<&CancellationToken>,
    ) -> Result<GitCommandOutput, GitError> {
        let root = self
            .require_repository_root(repository.as_ref(), cancellation)
            .await?;
        let invocation = GitInvocation::new(&root, fetch_args(request));
        let output = self.run_command(&invocation, None, cancellation).await?;
        self.ensure_success(&invocation, output)
    }

    pub async fn pull(
        &self,
        repository: impl AsRef<Path>,
        request: &GitPullRequest,
        cancellation: Option<&CancellationToken>,
    ) -> Result<GitCommandOutput, GitError> {
        let root = self
            .require_repository_root(repository.as_ref(), cancellation)
            .await?;
        let invocation = GitInvocation::new(&root, pull_args(request));
        let output = self.run_command(&invocation, None, cancellation).await?;
        self.ensure_success(&invocation, output)
    }

    pub async fn push(
        &self,
        repository: impl AsRef<Path>,
        request: &GitPushRequest,
        cancellation: Option<&CancellationToken>,
    ) -> Result<GitCommandOutput, GitError> {
        let root = self
            .require_repository_root(repository.as_ref(), cancellation)
            .await?;
        let invocation = GitInvocation::new(&root, push_args(request));
        let output = self.run_command(&invocation, None, cancellation).await?;
        self.ensure_success(&invocation, output)
    }

    pub async fn stash_list(
        &self,
        repository: impl AsRef<Path>,
        cancellation: Option<&CancellationToken>,
    ) -> Result<Vec<GitStashEntry>, GitError> {
        let root = self
            .require_repository_root(repository.as_ref(), cancellation)
            .await?;
        let invocation = GitInvocation::new(&root, ["stash", "list", "--format=%gd%x09%H%x09%gs"]);
        let output = self.run_command(&invocation, None, cancellation).await?;
        let output = self.ensure_success(&invocation, output)?;
        let text = stdout_string(&output.stdout)?;
        let mut entries = Vec::new();
        for line in text.lines() {
            if line.is_empty() {
                continue;
            }
            let mut parts = line.split('\t');
            let Some(reference) = parts.next() else {
                return Err(GitError::Parse {
                    context: "stash list",
                    message: "missing stash reference".to_owned(),
                });
            };
            let Some(hash) = parts.next() else {
                return Err(GitError::Parse {
                    context: "stash list",
                    message: "missing stash hash".to_owned(),
                });
            };
            let message = parts.next().unwrap_or_default();
            entries.push(GitStashEntry {
                reference: reference.to_owned(),
                hash: hash.to_owned(),
                message: message.to_owned(),
            });
        }
        Ok(entries)
    }

    pub async fn stash_create(
        &self,
        repository: impl AsRef<Path>,
        request: &GitStashCreateRequest,
        cancellation: Option<&CancellationToken>,
    ) -> Result<Option<GitStashEntry>, GitError> {
        let root = self
            .require_repository_root(repository.as_ref(), cancellation)
            .await?;
        let mut args = vec![
            OsString::from("stash"),
            OsString::from("push"),
            OsString::from("--message"),
            OsString::from(&request.message),
        ];
        if request.include_untracked {
            args.insert(2, OsString::from("--include-untracked"));
        }
        if request.keep_index {
            args.insert(2, OsString::from("--keep-index"));
        }
        let invocation = GitInvocation::new(&root, args);
        let output = self.run_command(&invocation, None, cancellation).await?;
        self.ensure_success(&invocation, output)?;
        let mut list = self.stash_list(&root, cancellation).await?;
        Ok(list.drain(..1).next())
    }

    pub async fn stash_apply(
        &self,
        repository: impl AsRef<Path>,
        reference: impl AsRef<OsStr>,
        pop: bool,
        index: bool,
        cancellation: Option<&CancellationToken>,
    ) -> Result<GitCommandOutput, GitError> {
        let root = self
            .require_repository_root(repository.as_ref(), cancellation)
            .await?;
        let mut args = vec![
            OsString::from("stash"),
            OsString::from(if pop { "pop" } else { "apply" }),
        ];
        if index {
            args.push(OsString::from("--index"));
        }
        args.push(reference.as_ref().to_os_string());
        let invocation = GitInvocation::new(&root, args);
        let output = self.run_command(&invocation, None, cancellation).await?;
        self.ensure_success(&invocation, output)
    }

    pub async fn log(
        &self,
        repository: impl AsRef<Path>,
        limit: usize,
        cancellation: Option<&CancellationToken>,
    ) -> Result<Vec<GitLogEntry>, GitError> {
        let root = self
            .require_repository_root(repository.as_ref(), cancellation)
            .await?;
        let invocation = GitInvocation::new(
            &root,
            [
                "log",
                "--date=iso-strict",
                "--decorate=short",
                "--format=%H%x1f%P%x1f%an%x1f%ae%x1f%ad%x1f%s%x1f%b%x1e",
            ],
        );
        let mut invocation = invocation;
        invocation
            .args
            .insert(3, OsString::from(format!("--max-count={limit}")));
        let output = self.run_command(&invocation, None, cancellation).await?;
        let output = self.ensure_success(&invocation, output)?;
        let text = stdout_string(&output.stdout)?;
        let mut entries = Vec::new();
        for record in text.split('\u{1e}') {
            let record = record.trim();
            if record.is_empty() {
                continue;
            }
            let mut parts = record.split('\u{1f}');
            let hash = parts.next().ok_or_else(|| GitError::Parse {
                context: "log",
                message: "missing hash".to_owned(),
            })?;
            let parents = parts
                .next()
                .unwrap_or_default()
                .split_whitespace()
                .map(ToOwned::to_owned)
                .collect();
            let author_name = parts.next().unwrap_or_default().to_owned();
            let author_email = parts.next().unwrap_or_default().to_owned();
            let authored_at = parts.next().unwrap_or_default().to_owned();
            let summary = parts.next().unwrap_or_default().to_owned();
            let body = parts.next().unwrap_or_default().to_owned();
            entries.push(GitLogEntry {
                hash: hash.to_owned(),
                parents,
                author_name,
                author_email,
                authored_at,
                summary,
                body,
            });
        }
        Ok(entries)
    }

    pub async fn conflict_files(
        &self,
        repository: impl AsRef<Path>,
        cancellation: Option<&CancellationToken>,
    ) -> Result<Vec<GitConflictFile>, GitError> {
        let status = self.status(repository, cancellation).await?;
        Ok(status
            .conflicts
            .into_iter()
            .map(|path| GitConflictFile {
                path,
                stages: vec![1, 2, 3],
            })
            .collect())
    }

    #[must_use]
    pub fn discard_file_plan(&self, diff: &GitDiffFile, target: DiffTarget) -> GitDiscardPlan {
        GitDiscardPlan {
            path: diff.path.clone(),
            target,
            scope: GitDiscardScope::File,
            requires_confirmation: diff.change != GitFileChange::Untracked
                || matches!(target, DiffTarget::Index),
            reason: if diff.change == GitFileChange::Untracked {
                "discarding an untracked file deletes it".to_owned()
            } else {
                "discarding tracked changes loses local edits".to_owned()
            },
            untracked: diff.change == GitFileChange::Untracked,
            hunk: None,
        }
    }

    pub fn discard_hunk_plan(
        &self,
        diff: &GitDiffFile,
        hunk: &GitDiffHunk,
        target: DiffTarget,
    ) -> Result<GitDiscardPlan, GitError> {
        if diff.binary {
            return Err(GitError::Parse {
                context: "discard hunk",
                message: "binary diffs do not support hunk discard".to_owned(),
            });
        }
        Ok(GitDiscardPlan {
            path: diff.path.clone(),
            target,
            scope: GitDiscardScope::Hunk,
            requires_confirmation: true,
            reason: "discarding a hunk loses local edits".to_owned(),
            untracked: diff.change == GitFileChange::Untracked,
            hunk: Some(hunk.clone()),
        })
    }

    async fn run_git(
        &self,
        cwd: &Path,
        args: impl IntoIterator<Item = impl Into<OsString>>,
        stdin: Option<&[u8]>,
        cancellation: Option<&CancellationToken>,
    ) -> Result<GitCommandOutput, GitError> {
        let invocation = GitInvocation::new(cwd, args);
        self.run_command(&invocation, stdin, cancellation).await
    }

    async fn run_command(
        &self,
        invocation: &GitInvocation,
        stdin: Option<&[u8]>,
        cancellation: Option<&CancellationToken>,
    ) -> Result<GitCommandOutput, GitError> {
        if matches!(cancellation, Some(token) if token.is_cancelled()) {
            return Err(GitError::Cancelled);
        }
        let mut command = Command::new(&self.executable);
        command
            .current_dir(&invocation.cwd)
            .args(&invocation.args)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());
        let mut child = command.spawn().map_err(GitError::Io)?;
        if let Some(stdin_bytes) = stdin {
            if let Some(mut child_stdin) = child.stdin.take() {
                child_stdin
                    .write_all(stdin_bytes)
                    .await
                    .map_err(GitError::Io)?;
                child_stdin.shutdown().await.map_err(GitError::Io)?;
            }
        }
        let mut stdout = child.stdout.take().ok_or_else(|| GitError::Parse {
            context: "process",
            message: "missing stdout pipe".to_owned(),
        })?;
        let mut stderr = child.stderr.take().ok_or_else(|| GitError::Parse {
            context: "process",
            message: "missing stderr pipe".to_owned(),
        })?;
        let stdout_task = tokio::spawn(async move {
            let mut buffer = Vec::new();
            stdout.read_to_end(&mut buffer).await.map(|_| buffer)
        });
        let stderr_task = tokio::spawn(async move {
            let mut buffer = Vec::new();
            stderr.read_to_end(&mut buffer).await.map(|_| buffer)
        });
        let status = if let Some(token) = cancellation {
            tokio::select! {
                status = child.wait() => status.map_err(GitError::Io)?,
                () = token.wait() => {
                    let _ = child.kill().await;
                    let _ = child.wait().await;
                    return Err(GitError::Cancelled);
                }
            }
        } else {
            child.wait().await.map_err(GitError::Io)?
        };
        let stdout = stdout_task
            .await
            .map_err(|error| GitError::Parse {
                context: "process stdout",
                message: error.to_string(),
            })?
            .map_err(GitError::Io)?;
        let stderr = stderr_task
            .await
            .map_err(|error| GitError::Parse {
                context: "process stderr",
                message: error.to_string(),
            })?
            .map_err(GitError::Io)?;
        Ok(GitCommandOutput {
            status,
            stdout,
            stderr,
        })
    }

    #[allow(clippy::unused_self)]
    fn ensure_success(
        &self,
        invocation: &GitInvocation,
        output: GitCommandOutput,
    ) -> Result<GitCommandOutput, GitError> {
        if output.status.success() {
            return Ok(output);
        }
        Err(GitError::from_output(invocation, &output))
    }

    async fn require_repository_root(
        &self,
        path: &Path,
        cancellation: Option<&CancellationToken>,
    ) -> Result<PathBuf, GitError> {
        self.repository_root(path, cancellation)
            .await?
            .ok_or_else(|| GitError::NotRepository {
                path: path.to_path_buf(),
            })
    }

    async fn rev_parse_head(
        &self,
        root: &Path,
        cancellation: Option<&CancellationToken>,
    ) -> Result<String, GitError> {
        let output = self
            .run_git(root, ["rev-parse", "--short", "HEAD"], None, cancellation)
            .await?;
        if !output.status.success() {
            return Err(GitError::from_output(
                &GitInvocation::new(root, ["rev-parse", "--short", "HEAD"]),
                &output,
            ));
        }
        Ok(stdout_string(&output.stdout)?.trim().to_owned())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitCommandOutput {
    pub status: std::process::ExitStatus,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitRepositoryStatus {
    pub root: PathBuf,
    pub summary: GitStatusSummary,
    pub branch_state: GitBranchState,
    pub entries: Vec<GitStatusEntry>,
    pub conflicts: Vec<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitBranchState {
    pub branch: Option<String>,
    pub detached_head: bool,
    pub head: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitStatusEntry {
    pub status: String,
    pub path: PathBuf,
    pub previous_path: Option<PathBuf>,
    pub staged: bool,
    pub unstaged: bool,
    pub untracked: bool,
    pub conflicted: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitDiffFile {
    pub change: GitFileChange,
    pub path: PathBuf,
    pub previous_path: Option<PathBuf>,
    pub binary: bool,
    pub hunks: Vec<GitDiffHunk>,
    pub status: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GitFileChange {
    Added,
    Deleted,
    Modified,
    Untracked,
    Renamed,
    Copied,
    TypeChanged,
    Unmerged,
    Unknown(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitDiffHunk {
    pub header: String,
    pub old_range: (usize, usize),
    pub new_range: (usize, usize),
    pub lines: Vec<GitDiffLine>,
    pub patch: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitDiffLine {
    pub kind: GitDiffLineKind,
    pub text: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GitDiffLineKind {
    Context,
    Addition,
    Removal,
    Meta,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitBranchInfo {
    pub name: String,
    pub oid: String,
    pub upstream: Option<String>,
    pub tracking: Option<String>,
    pub head: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitStashEntry {
    pub reference: String,
    pub hash: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitLogEntry {
    pub hash: String,
    pub parents: Vec<String>,
    pub author_name: String,
    pub author_email: String,
    pub authored_at: String,
    pub summary: String,
    pub body: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitConflictFile {
    pub path: PathBuf,
    pub stages: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitCommitRequest {
    pub message: String,
    pub amend: bool,
    pub allow_empty: bool,
    pub author_name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitFetchRequest {
    pub remote: Option<String>,
    pub refspecs: Vec<String>,
    pub prune: bool,
    pub tags: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitPullRequest {
    pub remote: Option<String>,
    pub branch: Option<String>,
    pub rebase: bool,
    pub ff_only: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitPushRequest {
    pub remote: Option<String>,
    pub refspecs: Vec<String>,
    pub set_upstream: bool,
    pub force_with_lease: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitStashCreateRequest {
    pub message: String,
    pub include_untracked: bool,
    pub keep_index: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GitDiscardScope {
    File,
    Hunk,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitDiscardPlan {
    pub path: PathBuf,
    pub target: DiffTarget,
    pub scope: GitDiscardScope,
    pub requires_confirmation: bool,
    pub reason: String,
    pub untracked: bool,
    pub hunk: Option<GitDiffHunk>,
}

#[derive(Debug, Error)]
pub enum GitError {
    #[error("git executable not found: {0}")]
    ExecutableNotFound(String),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("process was cancelled")]
    Cancelled,
    #[error("repository not found for path: {path}")]
    NotRepository { path: PathBuf },
    #[error("confirmation required: {0}")]
    ConfirmationRequired(String),
    #[error("git command failed: {command} (exit {code:?}) stderr: {stderr}")]
    CommandFailed {
        command: String,
        code: Option<i32>,
        stderr: String,
    },
    #[error("parse error in {context}: {message}")]
    Parse {
        context: &'static str,
        message: String,
    },
}

impl GitError {
    fn command_failed(invocation: &GitInvocation, output: &GitCommandOutput) -> Self {
        Self::CommandFailed {
            command: format_command(invocation),
            code: output.status.code(),
            stderr: stderr_string(&output.stderr),
        }
    }
}

impl GitError {
    fn from_output(invocation: &GitInvocation, output: &GitCommandOutput) -> Self {
        Self::command_failed(invocation, output)
    }
}

fn command_cwd(path: &Path) -> PathBuf {
    if path.is_file() {
        path.parent()
            .map_or_else(|| path.to_path_buf(), Path::to_path_buf)
    } else {
        path.to_path_buf()
    }
}

fn pathspec_for(root: &Path, path: &Path) -> PathBuf {
    path.strip_prefix(root)
        .map_or_else(|_| path.to_path_buf(), Path::to_path_buf)
}

fn append_pathspecs(args: &mut Vec<OsString>, root: &Path, paths: &[PathBuf]) {
    if paths.is_empty() {
        return;
    }
    args.push(OsString::from("--"));
    for path in paths {
        args.push(pathspec_for(root, path).into_os_string());
    }
}

fn stdout_string(bytes: &[u8]) -> Result<String, GitError> {
    String::from_utf8(bytes.to_vec()).map_err(|error| GitError::Parse {
        context: "utf8",
        message: error.to_string(),
    })
}

fn stderr_string(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

fn format_command(invocation: &GitInvocation) -> String {
    let mut command = String::from("git");
    for arg in &invocation.args {
        command.push(' ');
        command.push_str(&arg.to_string_lossy());
    }
    command
}

fn parse_status_line(line: &str) -> Result<GitStatusEntry, GitError> {
    if line.len() < 3 {
        return Err(GitError::Parse {
            context: "status",
            message: format!("short status line: {line}"),
        });
    }
    let status = &line[..2];
    let payload = &line[3..];
    let (staged, unstaged, untracked, conflicted) = classify_status(status);
    let (previous_path, path) = if let Some((from, to)) = payload.split_once(" -> ") {
        (Some(PathBuf::from(from)), PathBuf::from(to))
    } else {
        (None, PathBuf::from(payload))
    };
    Ok(GitStatusEntry {
        status: status.to_owned(),
        path,
        previous_path,
        staged,
        unstaged,
        untracked,
        conflicted,
    })
}

fn classify_status(status: &str) -> (bool, bool, bool, bool) {
    let bytes = status.as_bytes();
    let x = bytes.first().copied().unwrap_or(b' ');
    let y = bytes.get(1).copied().unwrap_or(b' ');
    let untracked = x == b'?' && y == b'?';
    let conflicted = [
        (b'A', b'A'),
        (b'D', b'D'),
        (b'U', b'U'),
        (b'U', b'A'),
        (b'U', b'D'),
        (b'A', b'U'),
        (b'D', b'U'),
        (b'D', b'A'),
        (b'A', b'D'),
    ]
    .contains(&(x, y));
    let staged = !untracked && x != b' ' && x != b'?';
    let unstaged = !untracked && y != b' ' && y != b'?';
    (staged, unstaged, untracked, conflicted)
}

fn parse_name_status_diff(stdout: &[u8]) -> Result<Vec<GitDiffFile>, GitError> {
    let mut fields = stdout
        .split(|byte| *byte == 0)
        .filter(|field| !field.is_empty());
    let mut files = Vec::new();
    while let Some(status) = fields.next() {
        let status = String::from_utf8(status.to_vec()).map_err(|error| GitError::Parse {
            context: "diff name-status",
            message: error.to_string(),
        })?;
        let change = classify_change(&status);
        let file = if let Some('R' | 'C') = status.chars().next() {
            let previous = fields.next().ok_or_else(|| GitError::Parse {
                context: "diff name-status",
                message: "missing original path for rename/copy".to_owned(),
            })?;
            let path = fields.next().ok_or_else(|| GitError::Parse {
                context: "diff name-status",
                message: "missing target path for rename/copy".to_owned(),
            })?;
            GitDiffFile {
                change,
                path: PathBuf::from(String::from_utf8(path.to_vec()).map_err(|error| {
                    GitError::Parse {
                        context: "diff name-status",
                        message: error.to_string(),
                    }
                })?),
                previous_path: Some(PathBuf::from(
                    String::from_utf8(previous.to_vec()).map_err(|error| GitError::Parse {
                        context: "diff name-status",
                        message: error.to_string(),
                    })?,
                )),
                binary: false,
                hunks: Vec::new(),
                status,
            }
        } else {
            let path = fields.next().ok_or_else(|| GitError::Parse {
                context: "diff name-status",
                message: "missing path".to_owned(),
            })?;
            GitDiffFile {
                change,
                path: PathBuf::from(String::from_utf8(path.to_vec()).map_err(|error| {
                    GitError::Parse {
                        context: "diff name-status",
                        message: error.to_string(),
                    }
                })?),
                previous_path: None,
                binary: false,
                hunks: Vec::new(),
                status,
            }
        };
        files.push(file);
    }
    Ok(files)
}

fn classify_change(status: &str) -> GitFileChange {
    match status.chars().next() {
        Some('A') => GitFileChange::Added,
        Some('D') => GitFileChange::Deleted,
        Some('M') => GitFileChange::Modified,
        Some('R') => GitFileChange::Renamed,
        Some('C') => GitFileChange::Copied,
        Some('T') => GitFileChange::TypeChanged,
        Some('U') => GitFileChange::Unmerged,
        _ => GitFileChange::Unknown(status.to_owned()),
    }
}

#[derive(Debug, Clone)]
struct ParsedPatchFile {
    binary: bool,
    hunks: Vec<GitDiffHunk>,
}

fn parse_patch_files(stdout: &str) -> Result<Vec<ParsedPatchFile>, GitError> {
    let mut files = Vec::new();
    let mut current_lines = Vec::new();
    for line in stdout.lines() {
        if line.starts_with("diff --git ") && !current_lines.is_empty() {
            files.push(parse_patch_file(&current_lines)?);
            current_lines.clear();
        }
        current_lines.push(line.to_owned());
    }
    if !current_lines.is_empty() {
        files.push(parse_patch_file(&current_lines)?);
    }
    Ok(files)
}

fn parse_patch_file(lines: &[String]) -> Result<ParsedPatchFile, GitError> {
    let mut binary = false;
    let mut hunks = Vec::new();
    let mut current_hunk: Option<Vec<String>> = None;
    let mut prefix_lines = Vec::new();
    let mut seen_hunk = false;
    for line in lines {
        if line.starts_with("Binary files ") || line.starts_with("GIT binary patch") {
            binary = true;
        }
        if line.starts_with("@@ ") {
            seen_hunk = true;
            if let Some(previous) = current_hunk.take() {
                hunks.push(parse_hunk(&prefix_lines, &previous)?);
            }
            current_hunk = Some(vec![line.to_owned()]);
            continue;
        }
        if let Some(hunk) = current_hunk.as_mut() {
            hunk.push(line.to_owned());
        } else if !seen_hunk {
            prefix_lines.push(line.to_owned());
        }
    }
    if let Some(previous) = current_hunk.take() {
        hunks.push(parse_hunk(&prefix_lines, &previous)?);
    }
    Ok(ParsedPatchFile { binary, hunks })
}

fn parse_hunk(prefix_lines: &[String], lines: &[String]) -> Result<GitDiffHunk, GitError> {
    let header = lines.first().cloned().ok_or_else(|| GitError::Parse {
        context: "hunk",
        message: "missing hunk header".to_owned(),
    })?;
    let (old_range, new_range) = parse_hunk_ranges(&header)?;
    let mut patch = String::new();
    let mut parsed_lines = Vec::new();
    for line in prefix_lines {
        if !patch.is_empty() {
            patch.push('\n');
        }
        patch.push_str(line);
    }
    for line in lines {
        if !patch.is_empty() {
            patch.push('\n');
        }
        patch.push_str(line);
        let kind = match line.chars().next() {
            Some('+') => GitDiffLineKind::Addition,
            Some('-') => GitDiffLineKind::Removal,
            Some(' ') => GitDiffLineKind::Context,
            _ => GitDiffLineKind::Meta,
        };
        parsed_lines.push(GitDiffLine {
            kind,
            text: line.clone(),
        });
    }
    patch.push('\n');
    Ok(GitDiffHunk {
        header,
        old_range,
        new_range,
        lines: parsed_lines,
        patch,
    })
}

#[allow(clippy::type_complexity)]
fn parse_hunk_ranges(header: &str) -> Result<((usize, usize), (usize, usize)), GitError> {
    let mut pieces = header.split_whitespace();
    let _marker = pieces.next();
    let old = pieces.next().ok_or_else(|| GitError::Parse {
        context: "hunk",
        message: "missing old range".to_owned(),
    })?;
    let new = pieces.next().ok_or_else(|| GitError::Parse {
        context: "hunk",
        message: "missing new range".to_owned(),
    })?;
    Ok((parse_range(old)?, parse_range(new)?))
}

fn parse_range(range: &str) -> Result<(usize, usize), GitError> {
    let range = range
        .trim_start_matches('@')
        .trim_start_matches('-')
        .trim_start_matches('+');
    let (start, count) = if let Some((start, count)) = range.split_once(',') {
        (start, count)
    } else {
        (range, "1")
    };
    let start = start.parse().map_err(|error| GitError::Parse {
        context: "hunk",
        message: format!("invalid hunk start {start}: {error}"),
    })?;
    let count = count.parse().map_err(|error| GitError::Parse {
        context: "hunk",
        message: format!("invalid hunk count {count}: {error}"),
    })?;
    Ok((start, count))
}

fn fetch_args(request: &GitFetchRequest) -> Vec<OsString> {
    let mut args = vec![OsString::from("fetch")];
    if request.prune {
        args.push(OsString::from("--prune"));
    }
    if request.tags {
        args.push(OsString::from("--tags"));
    }
    if let Some(remote) = &request.remote {
        args.push(OsString::from(remote));
    }
    args.extend(request.refspecs.iter().map(OsString::from));
    args
}

fn pull_args(request: &GitPullRequest) -> Vec<OsString> {
    let mut args = vec![OsString::from("pull")];
    if request.rebase {
        args.push(OsString::from("--rebase"));
    }
    if request.ff_only {
        args.push(OsString::from("--ff-only"));
    }
    if let Some(remote) = &request.remote {
        args.push(OsString::from(remote));
    }
    if let Some(branch) = &request.branch {
        args.push(OsString::from(branch));
    }
    args
}

fn push_args(request: &GitPushRequest) -> Vec<OsString> {
    let mut args = vec![OsString::from("push")];
    if request.set_upstream {
        args.push(OsString::from("--set-upstream"));
    }
    if request.force_with_lease {
        args.push(OsString::from("--force-with-lease"));
    }
    if let Some(remote) = &request.remote {
        args.push(OsString::from(remote));
    }
    args.extend(request.refspecs.iter().map(OsString::from));
    args
}

#[must_use]
pub fn build_fetch_invocation(cwd: impl Into<PathBuf>, request: &GitFetchRequest) -> GitInvocation {
    GitInvocation::new(cwd, fetch_args(request))
}

#[must_use]
pub fn build_pull_invocation(cwd: impl Into<PathBuf>, request: &GitPullRequest) -> GitInvocation {
    GitInvocation::new(cwd, pull_args(request))
}

#[must_use]
pub fn build_push_invocation(cwd: impl Into<PathBuf>, request: &GitPushRequest) -> GitInvocation {
    GitInvocation::new(cwd, push_args(request))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{fs, path::Path};
    use tempfile::TempDir;

    fn git() -> GitClient {
        GitClient::default()
    }

    fn init_repo() -> (TempDir, PathBuf) {
        let temp = TempDir::new().expect("tempdir");
        let root = temp.path().to_path_buf();
        let status = std::process::Command::new("git")
            .args(["init", "-b", "main"])
            .current_dir(&root)
            .status()
            .expect("git init");
        if !status.success() {
            let status = std::process::Command::new("git")
                .arg("init")
                .current_dir(&root)
                .status()
                .expect("git init fallback");
            assert!(status.success(), "git init fallback");
        }
        configure_identity(&root);
        (temp, root)
    }

    fn configure_identity(root: &Path) {
        for (key, value) in [
            ("user.name", "Test User"),
            ("user.email", "test@example.com"),
        ] {
            let status = std::process::Command::new("git")
                .args(["config", key, value])
                .current_dir(root)
                .status()
                .expect("git config");
            assert!(status.success(), "git config {key}");
        }
    }

    fn write_file(root: &Path, relative: &str, contents: &str) {
        let path = root.join(relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("parent");
        }
        fs::write(path, contents).expect("write file");
    }

    fn commit_all(root: &Path, message: &str) {
        let status = std::process::Command::new("git")
            .args(["add", "-A"])
            .current_dir(root)
            .status()
            .expect("git add");
        assert!(status.success(), "git add");
        let status = std::process::Command::new("git")
            .args(["commit", "-m", message])
            .current_dir(root)
            .status()
            .expect("git commit");
        assert!(status.success(), "git commit");
    }

    #[tokio::test]
    async fn repository_detection_and_status_work() {
        let (temp, root) = init_repo();
        let _keep = temp;
        write_file(&root, "src/hello.txt", "hello\n");
        let client = git();
        assert_eq!(
            client.repository_root(&root, None).await.expect("root"),
            Some(root.clone())
        );
        let status = client.status(&root, None).await.expect("status");
        assert_eq!(status.summary.untracked, 1);
        assert!(status.branch_state.branch.is_some());
    }

    #[tokio::test]
    async fn diff_parsing_handles_spaces_and_unicode() {
        let (temp, root) = init_repo();
        let _keep = temp;
        write_file(
            &root,
            "space dir/file name.txt",
            "one\ntwo\nthree\nfour\nfive\nsix\nseven\neight\nnine\nten\n",
        );
        write_file(
            &root,
            "ユニコード/初期.txt",
            "alpha\nbeta\ngamma\ndelta\nepsilon\nzeta\neta\ntheta\niota\nkappa\n",
        );
        commit_all(&root, "base");
        write_file(
            &root,
            "space dir/file name.txt",
            "one\nTWO\nthree\nfour\nfive\nsix\nseven\neight\nNINE\nten\n",
        );
        let files = git()
            .diff(&root, DiffTarget::WorkingTree, &[], None)
            .await
            .expect("diff");
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].path, PathBuf::from("space dir/file name.txt"));
        assert!(files[0].hunks.len() >= 2);
    }

    #[tokio::test]
    async fn stage_and_unstage_file_and_hunk_work() {
        let (temp, root) = init_repo();
        let _keep = temp;
        write_file(
            &root,
            "edit.txt",
            "line 1\nline 2\nline 3\nline 4\nline 5\nline 6\nline 7\nline 8\nline 9\nline 10\n",
        );
        commit_all(&root, "base");
        write_file(
            &root,
            "edit.txt",
            "line 1\nchanged 2\nline 3\nline 4\nline 5\nline 6\nline 7\nline 8\nchanged 9\nline 10\n",
        );
        let client = git();
        let diff = client
            .diff(&root, DiffTarget::WorkingTree, &[], None)
            .await
            .expect("diff");
        assert_eq!(diff.len(), 1);
        assert!(diff[0].hunks.len() >= 2);
        client
            .stage_hunk(&root, &diff[0].hunks[0], None)
            .await
            .expect("stage hunk");
        let staged = client
            .diff(&root, DiffTarget::Index, &[], None)
            .await
            .expect("staged diff");
        assert_eq!(staged.len(), 1);
        assert_eq!(staged[0].hunks.len(), 1);
        client
            .unstage_hunk(&root, &staged[0].hunks[0], None)
            .await
            .expect("unstage hunk");
        let clean_index = client
            .diff(&root, DiffTarget::Index, &[], None)
            .await
            .expect("index diff");
        assert!(clean_index.is_empty());
    }

    #[tokio::test]
    async fn commit_amend_and_log_work() {
        let (temp, root) = init_repo();
        let _keep = temp;
        write_file(&root, "commit.txt", "hello\n");
        commit_all(&root, "first");
        write_file(&root, "commit.txt", "hello again\n");
        commit_all(&root, "second");
        let client = git();
        let log = client.log(&root, 10, None).await.expect("log");
        assert!(log.len() >= 2);
        let request = GitCommitRequest {
            message: "amended".to_owned(),
            amend: true,
            allow_empty: false,
            author_name: None,
        };
        write_file(&root, "commit.txt", "hello amended\n");
        client
            .stage_file(&root, "commit.txt", None)
            .await
            .expect("stage");
        client.commit(&root, &request, None).await.expect("commit");
    }

    #[tokio::test]
    async fn branches_and_stash_work() {
        let (temp, root) = init_repo();
        let _keep = temp;
        write_file(&root, "branch.txt", "base\n");
        commit_all(&root, "base");
        let client = git();
        client
            .branch_create(&root, "feature", Some(OsStr::new("HEAD")), None)
            .await
            .expect("create");
        let branches = client.branches(&root, None).await.expect("branches");
        assert!(branches.iter().any(|branch| branch.name == "feature"));
        client
            .branch_switch(&root, "feature", false, None)
            .await
            .expect("switch");
        write_file(&root, "branch.txt", "changed\n");
        let stash = client
            .stash_create(
                &root,
                &GitStashCreateRequest {
                    message: "wip".to_owned(),
                    include_untracked: false,
                    keep_index: false,
                },
                None,
            )
            .await
            .expect("stash");
        assert!(stash.is_some());
        let stashes = client.stash_list(&root, None).await.expect("stash list");
        assert!(!stashes.is_empty());
    }

    #[tokio::test]
    async fn merge_conflict_detection_reports_paths() {
        let (temp, root) = init_repo();
        let _keep = temp;
        write_file(&root, "conflict.txt", "base\n");
        commit_all(&root, "base");
        let client = git();
        client
            .branch_create(&root, "left", Some(OsStr::new("HEAD")), None)
            .await
            .expect("branch left");
        write_file(&root, "conflict.txt", "main change\n");
        commit_all(&root, "main change");
        client
            .branch_switch(&root, "left", false, None)
            .await
            .expect("switch left");
        write_file(&root, "conflict.txt", "left change\n");
        commit_all(&root, "left change");
        client
            .branch_switch(&root, "main", false, None)
            .await
            .expect("switch main");
        let output = std::process::Command::new("git")
            .args(["merge", "left"])
            .current_dir(&root)
            .output()
            .expect("merge");
        assert!(!output.status.success());
        let conflicts = client.conflict_files(&root, None).await.expect("conflicts");
        assert!(
            conflicts
                .iter()
                .any(|entry| entry.path.ends_with("conflict.txt"))
        );
    }

    #[tokio::test]
    async fn command_failure_propagates_stderr() {
        let (temp, root) = init_repo();
        let _keep = temp;
        let client = git();
        let error = client
            .branch_delete(&root, "missing-branch", false, None)
            .await
            .expect_err("branch delete should fail");
        match error {
            GitError::CommandFailed { stderr, .. } => {
                assert!(!stderr.is_empty());
                assert!(stderr.contains("branch") || stderr.contains("not found"));
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn network_invocations_use_structured_arguments() {
        let fetch = build_fetch_invocation(
            ".",
            &GitFetchRequest {
                remote: Some("origin".to_owned()),
                refspecs: vec!["main".to_owned()],
                prune: true,
                tags: true,
            },
        );
        assert_eq!(
            fetch.args,
            vec![
                OsString::from("fetch"),
                OsString::from("--prune"),
                OsString::from("--tags"),
                OsString::from("origin"),
                OsString::from("main"),
            ]
        );

        let pull = build_pull_invocation(
            ".",
            &GitPullRequest {
                remote: Some("origin".to_owned()),
                branch: Some("main".to_owned()),
                rebase: true,
                ff_only: true,
            },
        );
        assert_eq!(
            pull.args,
            vec![
                OsString::from("pull"),
                OsString::from("--rebase"),
                OsString::from("--ff-only"),
                OsString::from("origin"),
                OsString::from("main"),
            ]
        );

        let push = build_push_invocation(
            ".",
            &GitPushRequest {
                remote: Some("origin".to_owned()),
                refspecs: vec!["HEAD:main".to_owned()],
                set_upstream: true,
                force_with_lease: true,
            },
        );
        assert_eq!(
            push.args,
            vec![
                OsString::from("push"),
                OsString::from("--set-upstream"),
                OsString::from("--force-with-lease"),
                OsString::from("origin"),
                OsString::from("HEAD:main"),
            ]
        );
    }

    #[test]
    fn discard_file_plan_requires_confirmation_for_tracked_changes() {
        let diff = GitDiffFile {
            change: GitFileChange::Modified,
            path: PathBuf::from("sample.txt"),
            previous_path: None,
            binary: false,
            hunks: Vec::new(),
            status: "M".to_owned(),
        };
        let plan = git().discard_file_plan(&diff, DiffTarget::WorkingTree);
        assert!(plan.requires_confirmation);
        assert_eq!(plan.scope, GitDiscardScope::File);
    }
}
