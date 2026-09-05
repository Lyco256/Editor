//! Process bootstrap and safe fallback startup path.

use std::{
    convert::Infallible,
    env,
    io::IsTerminal,
    path::PathBuf,
    process::{Command, ExitCode, Stdio},
    sync::mpsc::{self, Receiver, Sender},
    thread,
};

use editor_types::TerminalCapabilities;
use terminal_backend::{Framebuffer, TerminalAdapter};

use super::{
    action::Action,
    effect::Effect,
    event::Event,
    runtime::{
        AppRuntime, EffectDispatcher, QueueActionSource, RecordingDispatcher,
        run_interactive_with_state,
    },
    state::AppState,
};

#[derive(Debug, Default)]
struct BootstrapTerminal;

impl TerminalAdapter for BootstrapTerminal {
    type Error = Infallible;

    fn capabilities(&self) -> TerminalCapabilities {
        TerminalCapabilities::default()
    }

    fn enter(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }

    fn render(&mut self, _frame: &Framebuffer) -> Result<(), Self::Error> {
        Ok(())
    }

    fn restore(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
}

#[must_use]
pub fn run() -> ExitCode {
    install_panic_hook();
    let request = match StartupRequest::from_args(env::args_os().skip(1)) {
        Ok(request) => request,
        Err(error) => {
            eprintln!("Editor: {error}");
            return ExitCode::from(2);
        }
    };

    if request.path.is_some() {
        return run_interactive_session(request);
    }

    if std::io::stdout().is_terminal() {
        return run_interactive_session(StartupRequest { path: None });
    }

    // A non-interactive invocation still exercises the complete lifecycle and is useful for
    // redirected/headless launches where crossterm cannot enter raw mode.
    let input = QueueActionSource::new([Action::Quit]);
    let runtime = AppRuntime::new(
        BootstrapTerminal,
        input,
        RecordingDispatcher::default(),
        (80, 24),
    );
    match runtime.run() {
        Ok(_) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("Editor startup failed: {error}");
            ExitCode::FAILURE
        }
    }
}

fn install_panic_hook() {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        terminal_backend::restore_after_panic();
        eprintln!("Editor panic: subsystem=runtime operation=panic message={panic_info}");
        previous(panic_info);
    }));
}

/// Command-line startup request. A single positional path is supported, matching `editor .`
/// and `editor <file>` while leaving room for future flags.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct StartupRequest {
    pub path: Option<PathBuf>,
}

impl StartupRequest {
    /// Parses the supported positional command-line form.
    ///
    /// # Errors
    ///
    /// Returns a usage string when an unsupported option or more than one path is supplied.
    pub fn from_args(args: impl IntoIterator<Item = std::ffi::OsString>) -> Result<Self, String> {
        let mut path = None;
        for arg in args {
            if arg == "--help" || arg == "-h" {
                return Err("usage: editor [PATH]".to_owned());
            }
            if arg.to_string_lossy().starts_with('-') {
                return Err(format!("unknown option: {}", arg.to_string_lossy()));
            }
            if path.replace(PathBuf::from(arg)).is_some() {
                return Err("only one PATH may be provided".to_owned());
            }
        }
        Ok(Self { path })
    }
}

fn run_interactive_session(request: StartupRequest) -> ExitCode {
    let size = terminal_backend::CrosstermBackend::<std::io::Stdout>::size().unwrap_or((80, 24));
    let backend = terminal_backend::CrosstermBackend::stdout();
    let dispatcher = ServiceDispatcher::default();
    let mut state = AppState::default();
    let recovery = config_core::RecoveryStore::for_application("Editor").ok();
    if request.path.is_none() {
        if let Some(store) = &recovery {
            match store.load_latest() {
                Ok(loaded) => {
                    for warning in loaded.warnings {
                        eprintln!("Editor: recovery warning: {}", warning.message);
                    }
                    if let Some(session) = loaded.session {
                        state.restore_session(&session);
                    }
                }
                Err(error) => eprintln!("Editor: recovery unavailable: {error}"),
            }
        }
    }
    if let Some(path) = request.path {
        let message = if path.is_dir() {
            format!("opening workspace {}", path.display())
        } else {
            format!("opening file {}", path.display())
        };
        state.open_startup_path(&path);
        eprintln!("Editor: {message}");
    } else if state.active_path.is_none() {
        if let Ok(path) = env::current_dir() {
            state.open_startup_path(path);
        }
    }
    match run_interactive_with_state(backend, dispatcher, size, state) {
        Ok(final_state) => {
            if let Some(store) = recovery {
                if let Err(error) = store.save(&final_state.session_state()) {
                    eprintln!("Editor: could not persist session: {error}");
                }
            }
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("Editor runtime failed: {error}");
            ExitCode::FAILURE
        }
    }
}

#[derive(Debug)]
struct ServiceDispatcher {
    events: Receiver<Event>,
    sender: Sender<Event>,
}

impl Default for ServiceDispatcher {
    fn default() -> Self {
        let (sender, events) = mpsc::channel();
        Self { events, sender }
    }
}

impl EffectDispatcher for ServiceDispatcher {
    #[allow(clippy::too_many_lines)]
    fn dispatch(&mut self, effect: Effect) {
        let Effect::SaveDocument { path, text } = effect.clone() else {
            if let Effect::RefreshExplorer { request, roots } = effect.clone() {
                let sender = self.sender.clone();
                thread::spawn(move || {
                    let mut entries = Vec::new();
                    let mut failure = None;
                    for root in roots {
                        match workspace_core::canonicalize_path(&root) {
                            Ok(canonical) => match workspace_core::ExplorerTree::new(
                                vec![canonical.clone()],
                                Vec::new(),
                            )
                            .children(canonical.as_path())
                            {
                                Ok(children) => entries.extend(children.into_iter().map(|entry| {
                                    super::event::ExplorerEntryData {
                                        path: entry.path,
                                        depth: u8::try_from(entry.depth).unwrap_or(u8::MAX),
                                    }
                                })),
                                Err(error) => failure = Some(error.to_string()),
                            },
                            Err(error) => failure = Some(error.to_string()),
                        }
                    }
                    let event = failure.map_or_else(
                        || Event::ExplorerUpdated { request, entries },
                        |message| Event::EffectFailed {
                            request,
                            message: editor_types::OutputMessage {
                                subsystem: "workspace".to_owned(),
                                operation: "explorer".to_owned(),
                                level: editor_types::OutputLevel::Error,
                                message,
                            },
                        },
                    );
                    let _ = sender.send(event);
                });
                return;
            }
            if let Effect::RefreshGitStatus { request, root } = effect.clone() {
                let sender = self.sender.clone();
                thread::spawn(move || {
                    let event = match tokio::runtime::Builder::new_current_thread()
                        .enable_all()
                        .build()
                    {
                        Ok(runtime) => match runtime
                            .block_on(vcs_git::GitClient::default().status(&root, None))
                        {
                            Ok(status) => Event::GitStatusUpdated {
                                request,
                                summary: status.summary,
                            },
                            Err(error) => Event::EffectFailed {
                                request,
                                message: editor_types::OutputMessage {
                                    subsystem: "git".to_owned(),
                                    operation: "status".to_owned(),
                                    level: editor_types::OutputLevel::Error,
                                    message: error.to_string(),
                                },
                            },
                        },
                        Err(error) => Event::EffectFailed {
                            request,
                            message: editor_types::OutputMessage {
                                subsystem: "git".to_owned(),
                                operation: "runtime".to_owned(),
                                level: editor_types::OutputLevel::Error,
                                message: error.to_string(),
                            },
                        },
                    };
                    let _ = sender.send(event);
                });
                return;
            }
            if let Effect::ExternalProcess {
                request,
                kind,
                spec,
            } = effect
            {
                let sender = self.sender.clone();
                thread::spawn(move || {
                    let result = Command::new(&spec.executable)
                        .args(&spec.arguments)
                        .stdin(Stdio::null())
                        .stdout(Stdio::null())
                        .stderr(Stdio::piped())
                        .spawn();
                    let event = match result {
                        Ok(mut child)
                            if matches!(
                                kind,
                                super::effect::ExternalProcessKind::LanguageServer
                            ) =>
                        {
                            // Language servers are long-lived; leave the child attached to the
                            // session and report successful startup without waiting for exit.
                            let _ = child.stderr.take();
                            Event::EffectCompleted(request)
                        }
                        Ok(mut child) => match child.wait() {
                            Ok(status) if status.success() => Event::EffectCompleted(request),
                            Ok(status) => Event::EffectFailed {
                                request,
                                message: editor_types::OutputMessage {
                                    subsystem: format!("{kind:?}").to_lowercase(),
                                    operation: "process".to_owned(),
                                    level: editor_types::OutputLevel::Error,
                                    message: format!("{} exited with {status}", spec.executable),
                                },
                            },
                            Err(error) => Event::EffectFailed {
                                request,
                                message: editor_types::OutputMessage {
                                    subsystem: format!("{kind:?}").to_lowercase(),
                                    operation: "process-wait".to_owned(),
                                    level: editor_types::OutputLevel::Error,
                                    message: error.to_string(),
                                },
                            },
                        },
                        Err(error) => Event::EffectFailed {
                            request,
                            message: editor_types::OutputMessage {
                                subsystem: format!("{kind:?}").to_lowercase(),
                                operation: "process-spawn".to_owned(),
                                level: editor_types::OutputLevel::Error,
                                message: error.to_string(),
                            },
                        },
                    };
                    let _ = sender.send(event);
                });
            }
            return;
        };
        let sender = self.sender.clone();
        thread::spawn(move || {
            let event = match workspace_core::save_text_document(
                &path,
                &text,
                &workspace_core::DocumentSaveOptions::default(),
            ) {
                Ok(_) => Event::DocumentSaved { path },
                Err(error) => Event::DocumentSaveFailed {
                    path: path.clone(),
                    message: editor_types::OutputMessage {
                        subsystem: "workspace".to_owned(),
                        operation: "save-file".to_owned(),
                        level: editor_types::OutputLevel::Error,
                        message: format!("could not save {}: {error}", path.display()),
                    },
                },
            };
            let _ = sender.send(event);
        });
    }

    fn poll_events(&mut self) -> Vec<Event> {
        self.events.try_iter().collect()
    }
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;

    use super::StartupRequest;

    #[test]
    fn parses_file_and_directory_as_one_positional_path() {
        let request = StartupRequest::from_args([OsString::from("src/main.rs")])
            .expect("single path is valid");
        assert_eq!(
            request.path.as_deref(),
            Some(std::path::Path::new("src/main.rs"))
        );
    }

    #[test]
    fn rejects_unknown_options_and_multiple_paths() {
        assert!(StartupRequest::from_args([OsString::from("--version")]).is_err());
        assert!(StartupRequest::from_args([OsString::from("a"), OsString::from("b")]).is_err());
    }
}
