#![allow(clippy::missing_errors_doc)]

use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use editor_types::{CharacterOffset, TextRange};
use thiserror::Error;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    process::Command,
    sync::Notify,
    task::JoinError,
    time,
};

use crate::TextSnapshot;
use crate::protocol::CommandSpec;

#[derive(Debug, Clone)]
pub struct FormatterSpec {
    pub command: CommandSpec,
    pub timeout: Duration,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FormatterOutcome {
    pub edit_range: TextRange,
    pub replacement: String,
    pub stdout: String,
    pub stderr: String,
    pub exit_code: Option<i32>,
}

#[derive(Debug, Error)]
pub enum FormatterError {
    #[error("could not spawn formatter at {path:?}: {source}")]
    Spawn {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("formatter input failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("formatter timed out after {0:?}")]
    TimedOut(Duration),
    #[error("formatter was cancelled")]
    Cancelled,
    #[error("formatter exited with code {code:?}: {stderr}")]
    NonZeroExit { code: Option<i32>, stderr: String },
    #[error("formatter stdout was not valid UTF-8: {0}")]
    InvalidUtf8(#[from] std::string::FromUtf8Error),
    #[error("formatter task failed: {0}")]
    Join(#[from] JoinError),
}

#[derive(Debug, Clone, Default)]
pub struct ProcessCancelToken {
    inner: Arc<CancelState>,
}

#[derive(Debug, Default)]
struct CancelState {
    cancelled: AtomicBool,
    notify: Notify,
}

impl ProcessCancelToken {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn cancel(&self) {
        if !self.inner.cancelled.swap(true, Ordering::SeqCst) {
            self.inner.notify.notify_waiters();
        }
    }

    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.inner.cancelled.load(Ordering::SeqCst)
    }

    pub async fn cancelled(&self) {
        if self.is_cancelled() {
            return;
        }
        self.inner.notify.notified().await;
    }
}

pub struct FormatterRunner;

impl Default for FormatterRunner {
    fn default() -> Self {
        Self::new()
    }
}

impl FormatterRunner {
    #[must_use]
    pub const fn new() -> Self {
        Self
    }

    pub async fn run(
        &self,
        snapshot: &TextSnapshot,
        spec: &FormatterSpec,
        cancel: &ProcessCancelToken,
    ) -> Result<FormatterOutcome, FormatterError> {
        if cancel.is_cancelled() {
            return Err(FormatterError::Cancelled);
        }
        let mut command = Command::new(&spec.command.executable);
        command.args(&spec.command.args);
        for (key, value) in &spec.command.environment {
            command.env(key, value);
        }
        if let Some(current_dir) = &spec.command.current_dir {
            command.current_dir(current_dir);
        }
        command.stdin(std::process::Stdio::piped());
        command.stdout(std::process::Stdio::piped());
        command.stderr(std::process::Stdio::piped());
        let mut child = command.spawn().map_err(|source| FormatterError::Spawn {
            path: spec.command.executable.clone(),
            source,
        })?;
        let mut stdin = child.stdin.take().ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::BrokenPipe,
                "formatter stdin unavailable",
            )
        })?;
        let stdout = child.stdout.take().ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::BrokenPipe,
                "formatter stdout unavailable",
            )
        })?;
        let stderr = child.stderr.take().ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::BrokenPipe,
                "formatter stderr unavailable",
            )
        })?;
        let stdout_task = tokio::spawn(async move {
            let mut reader = stdout;
            let mut buffer = Vec::new();
            reader.read_to_end(&mut buffer).await.map(|_| buffer)
        });
        let stderr_task = tokio::spawn(async move {
            let mut reader = stderr;
            let mut buffer = Vec::new();
            reader.read_to_end(&mut buffer).await.map(|_| buffer)
        });
        stdin.write_all(snapshot.text().as_bytes()).await?;
        drop(stdin);
        let mut wait = Box::pin(child.wait());
        let timeout = time::sleep(spec.timeout);
        tokio::pin!(timeout);
        let exit_status = tokio::select! {
            status = &mut wait => status?,
            () = cancel.cancelled() => {
                drop(wait);
                let _ = child.kill().await;
                return Err(FormatterError::Cancelled);
            }
            () = &mut timeout => {
                drop(wait);
                let _ = child.kill().await;
                return Err(FormatterError::TimedOut(spec.timeout));
            }
        };
        let stdout_bytes = stdout_task.await??;
        let stderr_bytes = stderr_task.await??;
        let stdout = String::from_utf8(stdout_bytes)?;
        let stderr = String::from_utf8_lossy(&stderr_bytes).into_owned();
        if !exit_status.success() {
            return Err(FormatterError::NonZeroExit {
                code: exit_status.code(),
                stderr,
            });
        }
        let edit_range = TextRange {
            start: CharacterOffset(0),
            end: CharacterOffset(snapshot.len_chars()),
        };
        Ok(FormatterOutcome {
            edit_range,
            replacement: stdout.clone(),
            stdout,
            stderr,
            exit_code: exit_status.code(),
        })
    }
}
