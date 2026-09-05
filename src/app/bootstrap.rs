//! Process bootstrap and safe fallback startup path.

use std::{
    convert::Infallible,
    env,
    io::IsTerminal,
    path::PathBuf,
    process::ExitCode,
    sync::atomic::{AtomicBool, Ordering},
    sync::mpsc::{self, Receiver, Sender},
    sync::{Arc, Mutex},
};

use editor_types::TerminalCapabilities;
use terminal_backend::{Framebuffer, TerminalAdapter};

use super::{
    action::Action,
    effect::Effect,
    event::Event,
    runtime::{
        AppRuntime, EffectDispatcher, QueueActionSource, RecordingDispatcher,
        run_interactive_with_state_and_recovery,
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
    let dispatcher = match ServiceDispatcher::try_new() {
        Ok(dispatcher) => dispatcher,
        Err(error) => {
            eprintln!("Editor: background runtime unavailable: {error}");
            return ExitCode::FAILURE;
        }
    };
    let mut state = AppState::default();
    let recovery = config_core::RecoveryStore::for_application("Editor").ok();
    let trust_path = recovery.as_ref().and_then(|store| {
        store
            .directory()
            .parent()
            .map(|path| path.join("trust.json"))
    });
    if let Some(path) = &trust_path {
        if let Ok(store) = workspace_core::load_trust_store(path) {
            state.set_trust_store(store);
        }
    }
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
    apply_workspace_settings(&mut state);
    state.schedule_syntax_refresh();
    match run_interactive_with_state_and_recovery(
        backend,
        dispatcher,
        size,
        state,
        recovery.clone(),
    ) {
        Ok(final_state) => {
            if let Some(store) = recovery {
                if let Err(error) = store.save(&final_state.session_state()) {
                    eprintln!("Editor: could not persist session: {error}");
                }
            }
            if let Some(path) = trust_path {
                if let Err(error) = final_state.trust_store.save(&path) {
                    eprintln!("Editor: could not persist workspace trust: {error}");
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

fn apply_workspace_settings(state: &mut AppState) {
    let Some(root) = state.workspace_roots.first() else {
        return;
    };
    let path = root.join(".vscode").join("settings.json");
    let loaded = match std::fs::read_to_string(&path) {
        Ok(contents) => match vscode_compat::load_vscode_settings_jsonc(&contents) {
            Ok(parsed) => config_core::LoadSettingsResult {
                layer: vscode_settings_layer(parsed.layer),
                issues: parsed
                    .warnings
                    .into_iter()
                    .map(|warning| config_core::SettingsIssue {
                        path: path.clone(),
                        message: warning.message,
                    })
                    .collect(),
            },
            Err(error) => config_core::LoadSettingsResult {
                layer: config_core::SettingsLayer::default(),
                issues: vec![config_core::SettingsIssue {
                    path: path.clone(),
                    message: error.to_string(),
                }],
            },
        },
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            config_core::LoadSettingsResult {
                layer: config_core::SettingsLayer::default(),
                issues: Vec::new(),
            }
        }
        Err(error) => config_core::LoadSettingsResult {
            layer: config_core::SettingsLayer::default(),
            issues: vec![config_core::SettingsIssue {
                path: path.clone(),
                message: error.to_string(),
            }],
        },
    };
    let settings = config_core::SettingsStack {
        workspace: loaded.layer,
        ..config_core::SettingsStack::default()
    }
    .resolve();
    state.apply_settings(&settings);
    for issue in loaded.issues {
        state.output.push(editor_types::OutputMessage {
            subsystem: "settings".to_owned(),
            operation: "load".to_owned(),
            level: editor_types::OutputLevel::Warning,
            message: format!("{}: {}", issue.path.display(), issue.message),
        });
    }
}

fn vscode_settings_layer(layer: vscode_compat::SettingsLayer) -> config_core::SettingsLayer {
    config_core::SettingsLayer {
        line_numbers: layer.line_numbers,
        tab_size: layer.tab_size,
        insert_spaces: layer.insert_spaces,
        word_wrap: layer.word_wrap.map(|value| match value {
            vscode_compat::WordWrap::Off => config_core::WordWrap::Off,
            vscode_compat::WordWrap::On => config_core::WordWrap::On,
            vscode_compat::WordWrap::WordWrapColumn => config_core::WordWrap::WordWrapColumn,
            vscode_compat::WordWrap::Bounded => config_core::WordWrap::Bounded,
        }),
        auto_closing_pairs: layer.auto_closing_pairs.map(|value| match value {
            vscode_compat::AutoClosingPairs::Never => config_core::AutoClosingPairs::Never,
            vscode_compat::AutoClosingPairs::LanguageDefined => {
                config_core::AutoClosingPairs::LanguageDefined
            }
            vscode_compat::AutoClosingPairs::BeforeWhitespace => {
                config_core::AutoClosingPairs::BeforeWhitespace
            }
            vscode_compat::AutoClosingPairs::Always => config_core::AutoClosingPairs::Always,
        }),
        format_on_save: layer.format_on_save,
        format_on_paste: layer.format_on_paste,
        theme: layer.theme,
        search_excludes: layer.search_excludes,
        files_excludes: layer.files_excludes,
        large_file_threshold: layer.large_file_threshold,
        encoding_fallback: layer.encoding_fallback,
        line_ending: layer.line_ending.map(|value| match value {
            vscode_compat::LineEndingPreference::Preserve => {
                config_core::LineEndingPreference::Preserve
            }
            vscode_compat::LineEndingPreference::Lf => config_core::LineEndingPreference::Lf,
            vscode_compat::LineEndingPreference::Crlf => config_core::LineEndingPreference::Crlf,
        }),
        language_server_commands: layer.language_server_commands,
        external_formatter_commands: layer.external_formatter_commands,
        keybindings: layer.keybindings.map(|entries| {
            entries
                .into_iter()
                .map(|entry| config_core::Keybinding {
                    key: entry.key,
                    command: entry.command,
                    when: entry.when,
                    unknown: entry.unknown,
                })
                .collect()
        }),
        unknown: layer.unknown,
    }
}

struct ServiceDispatcher {
    events: Receiver<Event>,
    sender: Sender<Event>,
    shutdown: Arc<AtomicBool>,
    runtime: tokio::runtime::Runtime,
    syntax: Arc<Mutex<syntax_engine::SyntaxEngine>>,
    searches: Arc<Mutex<std::collections::HashMap<u64, workspace_core::SearchCancellation>>>,
}

impl ServiceDispatcher {
    fn try_new() -> Result<Self, String> {
        let (sender, events) = mpsc::channel();
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .map_err(|error| error.to_string())?;
        Ok(Self {
            events,
            sender,
            shutdown: Arc::new(AtomicBool::new(false)),
            runtime,
            syntax: Arc::new(Mutex::new(syntax_engine::SyntaxEngine::default())),
            searches: Arc::new(Mutex::new(std::collections::HashMap::new())),
        })
    }
}

impl Drop for ServiceDispatcher {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::SeqCst);
    }
}

impl EffectDispatcher for ServiceDispatcher {
    #[allow(clippy::too_many_lines)]
    fn dispatch(&mut self, effect: Effect) {
        let Effect::SaveDocument {
            path,
            text,
            encoding,
            with_bom,
            line_endings,
        } = effect.clone()
        else {
            if let Effect::SaveDocumentAs {
                path,
                text,
                encoding,
                with_bom,
                line_endings,
            } = effect.clone()
            {
                let sender = self.sender.clone();
                self.runtime.spawn_blocking(move || {
                    let event = match workspace_core::save_text_document(
                        &path,
                        &text,
                        &workspace_core::DocumentSaveOptions {
                            encoding,
                            with_bom,
                            line_endings,
                            ..workspace_core::DocumentSaveOptions::default()
                        },
                    ) {
                        Ok(_) => Event::DocumentSavedAs { path },
                        Err(error) => Event::DocumentSaveFailed {
                            path: path.clone(),
                            message: editor_types::OutputMessage {
                                subsystem: "workspace".to_owned(),
                                operation: "save-as".to_owned(),
                                level: editor_types::OutputLevel::Error,
                                message: format!("could not save {}: {error}", path.display()),
                            },
                        },
                    };
                    let _ = sender.send(event);
                });
                return;
            }
            if let Effect::FileOperation { request, plan } = effect.clone() {
                let sender = self.sender.clone();
                self.runtime.spawn_blocking(move || {
                    let result = match &plan {
                        workspace_core::FileOperationPlan::CreateFile { path } => {
                            workspace_core::create_file(path, &[])
                        }
                        workspace_core::FileOperationPlan::Rename(rename) => {
                            workspace_core::rename_path(&rename.source, &rename.target)
                        }
                        workspace_core::FileOperationPlan::Move(move_plan) => {
                            workspace_core::move_path(&move_plan.source, &move_plan.target)
                        }
                        workspace_core::FileOperationPlan::Delete(delete) => {
                            workspace_core::delete_file(&delete.path)
                        }
                    };
                    let event = match result {
                        Ok(()) => Event::FileOperationCompleted { request, plan },
                        Err(error) => Event::FileOperationFailed {
                            request,
                            message: editor_types::OutputMessage {
                                subsystem: "workspace".to_owned(),
                                operation: "file-operation".to_owned(),
                                level: editor_types::OutputLevel::Error,
                                message: error.to_string(),
                            },
                        },
                    };
                    let _ = sender.send(event);
                });
                return;
            }
            if let Effect::RefreshSyntax {
                request,
                document,
                version,
                path,
                text,
                large_file,
            } = effect.clone()
            {
                let sender = self.sender.clone();
                let syntax = self.syntax.clone();
                self.runtime.spawn_blocking(move || {
                    let descriptor = editor_core::DocumentDescriptor {
                        id: document,
                        version,
                        large_file_mode: large_file,
                    };
                    let update = match syntax.lock() {
                        Ok(mut engine) => engine.open_document(
                            syntax_engine::OpenDocument {
                                descriptor,
                                path: path.as_deref(),
                                language_override: None,
                                text: &text,
                            },
                            None,
                        ),
                        Err(_) => syntax_engine::SyntaxUpdate {
                            status: syntax_engine::SyntaxStatus::Cancelled,
                            snapshot: syntax_engine::SyntaxSnapshot::default(),
                            changed_ranges: Vec::new(),
                        },
                    };
                    let _ = sender.send(Event::SyntaxUpdated { request, update });
                });
                return;
            }
            if let Effect::RefreshExplorer { request, roots } = effect.clone() {
                let sender = self.sender.clone();
                self.runtime.spawn_blocking(move || {
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
            if let Effect::SearchWorkspace {
                session_id,
                options,
            } = effect.clone()
            {
                let sender = self.sender.clone();
                let searches = self.searches.clone();
                self.runtime.spawn_blocking(move || {
                    match workspace_core::search_workspace(options) {
                        Ok(session) => {
                            if let Ok(mut active) = searches.lock() {
                                active.insert(session_id, session.cancellation());
                            }
                            while let Some(event) = session.recv() {
                                match event {
                                    workspace_core::SearchEvent::Match(hit) => {
                                        let _ = sender.send(Event::SearchResult {
                                            session_id,
                                            result: app_ui::workspace::SearchResult {
                                                path: hit.path,
                                                line_number: hit.line_number,
                                                line_text: hit.line_text,
                                                matched_text: hit.matched_text,
                                            },
                                        });
                                    }
                                    workspace_core::SearchEvent::Finished { .. } => {
                                        let _ = sender.send(Event::SearchFinished { session_id });
                                        break;
                                    }
                                    workspace_core::SearchEvent::Cancelled => {
                                        let _ = sender.send(Event::SearchCancelled { session_id });
                                        break;
                                    }
                                    workspace_core::SearchEvent::Error(message) => {
                                        let _ = sender.send(Event::SearchFailed {
                                            session_id,
                                            message: editor_types::OutputMessage {
                                                subsystem: "workspace".to_owned(),
                                                operation: "search".to_owned(),
                                                level: editor_types::OutputLevel::Error,
                                                message,
                                            },
                                        });
                                        break;
                                    }
                                }
                            }
                            if let Ok(mut active) = searches.lock() {
                                active.remove(&session_id);
                            }
                        }
                        Err(error) => {
                            let _ = sender.send(Event::SearchFailed {
                                session_id,
                                message: editor_types::OutputMessage {
                                    subsystem: "workspace".to_owned(),
                                    operation: "search".to_owned(),
                                    level: editor_types::OutputLevel::Error,
                                    message: error.to_string(),
                                },
                            });
                        }
                    }
                });
                return;
            }
            if let Effect::CancelSearch { session_id } = effect.clone() {
                if let Ok(active) = self.searches.lock() {
                    if let Some(cancellation) = active.get(&session_id) {
                        cancellation.cancel();
                    }
                }
                return;
            }
            if let Effect::LspRequest {
                request,
                version,
                spec,
                method,
                params,
            } = effect.clone()
            {
                let sender = self.sender.clone();
                let method_for_event = method.clone();
                self.runtime.spawn_blocking(move || {
                    let result = tokio::runtime::Builder::new_current_thread()
                        .enable_all()
                        .build()
                        .map_err(|error| error.to_string())
                        .and_then(|runtime| {
                            runtime.block_on(async move {
                                let command = lsp_client::CommandSpec {
                                    executable: PathBuf::from(spec.executable),
                                    args: spec.arguments,
                                    environment: std::collections::BTreeMap::new(),
                                    current_dir: None,
                                };
                                let client = lsp_client::LspClient::spawn(command)
                                    .await
                                    .map_err(|error| error.to_string())?;
                                client
                                    .initialize(lsp_client::protocol::InitializeParams::default())
                                    .await
                                    .map_err(|error| error.to_string())?;
                                client
                                    .initialized()
                                    .await
                                    .map_err(|error| error.to_string())?;
                                let response = client
                                    .request_json(&method, params)
                                    .await
                                    .map_err(|error| error.to_string());
                                let _ = client.shutdown().await;
                                response
                            })
                        });
                    match result {
                        Ok(result) => {
                            let _ = sender.send(Event::LspResponse {
                                request,
                                version,
                                method: method_for_event,
                                result,
                            });
                        }
                        Err(message) => {
                            let _ = sender.send(Event::EffectFailed {
                                request,
                                message: editor_types::OutputMessage {
                                    subsystem: "lsp".to_owned(),
                                    operation: "request".to_owned(),
                                    level: editor_types::OutputLevel::Error,
                                    message,
                                },
                            });
                        }
                    }
                });
                return;
            }
            if let Effect::GitHunk {
                request,
                root,
                hunk,
                reverse,
            } = effect.clone()
            {
                let sender = self.sender.clone();
                self.runtime.spawn_blocking(move || {
                    let outcome = tokio::runtime::Builder::new_current_thread()
                        .enable_all()
                        .build()
                        .map_err(|error| error.to_string())
                        .and_then(|runtime| {
                            runtime
                                .block_on(async {
                                    if reverse {
                                        vcs_git::GitClient::default()
                                            .unstage_hunk(&root, &hunk, None)
                                            .await
                                    } else {
                                        vcs_git::GitClient::default()
                                            .stage_hunk(&root, &hunk, None)
                                            .await
                                    }
                                })
                                .map_err(|error| error.to_string())
                        });
                    let event = outcome.map_or_else(
                        |message| Event::EffectFailed {
                            request,
                            message: editor_types::OutputMessage {
                                subsystem: "git".to_owned(),
                                operation: if reverse {
                                    "unstage-hunk".to_owned()
                                } else {
                                    "stage-hunk".to_owned()
                                },
                                level: editor_types::OutputLevel::Error,
                                message,
                            },
                        },
                        |_| Event::EffectCompleted(request),
                    );
                    let _ = sender.send(event);
                });
                return;
            }
            if let Effect::GitDiscard {
                request,
                root,
                plan,
                confirmed,
            } = effect.clone()
            {
                let sender = self.sender.clone();
                self.runtime.spawn_blocking(move || {
                    let outcome = tokio::runtime::Builder::new_current_thread()
                        .enable_all()
                        .build()
                        .map_err(|error| error.to_string())
                        .and_then(|runtime| {
                            runtime
                                .block_on(
                                    vcs_git::GitClient::default()
                                        .execute_discard(&root, &plan, confirmed, None),
                                )
                                .map_err(|error| error.to_string())
                        });
                    let event = outcome.map_or_else(
                        |message| Event::EffectFailed {
                            request,
                            message: editor_types::OutputMessage {
                                subsystem: "git".to_owned(),
                                operation: "discard".to_owned(),
                                level: editor_types::OutputLevel::Error,
                                message,
                            },
                        },
                        |_| Event::EffectCompleted(request),
                    );
                    let _ = sender.send(event);
                });
                return;
            }
            if let Effect::ClipboardWrite { request, text, cut } = effect.clone() {
                let sender = self.sender.clone();
                self.runtime.spawn_blocking(move || {
                    let event = match terminal_backend::SystemClipboard::new() {
                        Ok(mut clipboard) => {
                            match terminal_backend::Clipboard::write_text(&mut clipboard, &text) {
                                Ok(()) => Event::ClipboardWritten { request, cut },
                                Err(error) => Event::ClipboardFailed {
                                    request,
                                    message: editor_types::OutputMessage {
                                        subsystem: "clipboard".to_owned(),
                                        operation: "write".to_owned(),
                                        level: editor_types::OutputLevel::Error,
                                        message: error.to_string(),
                                    },
                                },
                            }
                        }
                        Err(error) => Event::ClipboardFailed {
                            request,
                            message: editor_types::OutputMessage {
                                subsystem: "clipboard".to_owned(),
                                operation: "connect".to_owned(),
                                level: editor_types::OutputLevel::Error,
                                message: error.to_string(),
                            },
                        },
                    };
                    let _ = sender.send(event);
                });
                return;
            }
            if let Effect::ClipboardRead { request } = effect.clone() {
                let sender = self.sender.clone();
                self.runtime.spawn_blocking(move || {
                    let event = match terminal_backend::SystemClipboard::new() {
                        Ok(mut clipboard) => {
                            match terminal_backend::Clipboard::read_text(&mut clipboard) {
                                Ok(text) => Event::ClipboardRead { request, text },
                                Err(error) => Event::ClipboardFailed {
                                    request,
                                    message: editor_types::OutputMessage {
                                        subsystem: "clipboard".to_owned(),
                                        operation: "read".to_owned(),
                                        level: editor_types::OutputLevel::Error,
                                        message: error.to_string(),
                                    },
                                },
                            }
                        }
                        Err(error) => Event::ClipboardFailed {
                            request,
                            message: editor_types::OutputMessage {
                                subsystem: "clipboard".to_owned(),
                                operation: "connect".to_owned(),
                                level: editor_types::OutputLevel::Error,
                                message: error.to_string(),
                            },
                        },
                    };
                    let _ = sender.send(event);
                });
                return;
            }
            if let Effect::FormatDocument {
                request,
                text,
                spec,
                timeout_ms,
            } = effect.clone()
            {
                let sender = self.sender.clone();
                self.runtime.spawn_blocking(move || {
                    let event = match tokio::runtime::Builder::new_current_thread()
                        .enable_all()
                        .build()
                    {
                        Ok(runtime) => {
                            let snapshot = lsp_client::TextSnapshot::new(text.clone());
                            let command = lsp_client::CommandSpec {
                                executable: PathBuf::from(spec.executable),
                                args: spec.arguments,
                                environment: std::collections::BTreeMap::new(),
                                current_dir: None,
                            };
                            let formatter = lsp_client::FormatterSpec {
                                command,
                                timeout: std::time::Duration::from_millis(timeout_ms),
                            };
                            match runtime.block_on(lsp_client::FormatterRunner::new().run(
                                &snapshot,
                                &formatter,
                                &lsp_client::ProcessCancelToken::new(),
                            )) {
                                Ok(outcome) => Event::DocumentFormatted {
                                    request,
                                    replacement: outcome.replacement,
                                },
                                Err(error) => Event::DocumentFormatFailed {
                                    request,
                                    message: editor_types::OutputMessage {
                                        subsystem: "formatter".to_owned(),
                                        operation: "format".to_owned(),
                                        level: editor_types::OutputLevel::Error,
                                        message: error.to_string(),
                                    },
                                },
                            }
                        }
                        Err(error) => Event::DocumentFormatFailed {
                            request,
                            message: editor_types::OutputMessage {
                                subsystem: "formatter".to_owned(),
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
            if let Effect::ApplyReplacementPlan { request, plan } = effect.clone() {
                let sender = self.sender.clone();
                self.runtime.spawn_blocking(move || {
                    let event = match workspace_core::apply_replacement_plan(&plan) {
                        Ok(report) => Event::ReplacementApplied { request, report },
                        Err(error) => Event::ReplacementFailed {
                            request,
                            message: editor_types::OutputMessage {
                                subsystem: "workspace".to_owned(),
                                operation: "replace".to_owned(),
                                level: editor_types::OutputLevel::Error,
                                message: error.to_string(),
                            },
                        },
                    };
                    let _ = sender.send(event);
                });
                return;
            }
            if let Effect::RefreshGitStatus { request, root } = effect.clone() {
                let sender = self.sender.clone();
                self.runtime.spawn_blocking(move || {
                    let event = match tokio::runtime::Builder::new_current_thread()
                        .enable_all()
                        .build()
                    {
                        Ok(runtime) => match runtime.block_on(async {
                            let client = vcs_git::GitClient::default();
                            let status = client.status(&root, None).await?;
                            let diff_files = client
                                .diff(&root, vcs_git::DiffTarget::WorkingTree, &[], None)
                                .await
                                .unwrap_or_default();
                            let branches = client.branches(&root, None).await.unwrap_or_default();
                            let stashes = client.stash_list(&root, None).await.unwrap_or_default();
                            let history = client.log(&root, 50, None).await.unwrap_or_default();
                            Ok::<_, vcs_git::GitError>((
                                status, diff_files, branches, stashes, history,
                            ))
                        }) {
                            Ok((status, diff_files, branches, stashes, history)) => {
                                Event::GitStatusUpdated {
                                    request,
                                    root: status.root.clone(),
                                    summary: status.summary,
                                    entries: status.entries,
                                    branch_state: status.branch_state.branch,
                                    head: status.branch_state.head,
                                    conflicts: status
                                        .conflicts
                                        .into_iter()
                                        .map(|path| vcs_git::GitConflictFile {
                                            path,
                                            stages: vec![1, 2, 3],
                                        })
                                        .collect(),
                                    diff_files,
                                    branches,
                                    stashes,
                                    history,
                                }
                            }
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
                let shutdown = self.shutdown.clone();
                if matches!(kind, super::effect::ExternalProcessKind::LanguageServer) {
                    self.runtime.spawn_blocking(move || {
                        let event_sender = sender.clone();
                        let shutdown = shutdown.clone();
                        let result = tokio::runtime::Builder::new_current_thread()
                            .enable_all()
                            .build()
                            .map_err(|error| error.to_string())
                            .and_then(|runtime| {
                                let event_sender = event_sender.clone();
                                runtime.block_on(async move {
                                    let command = lsp_client::CommandSpec {
                                        executable: PathBuf::from(spec.executable),
                                        args: spec.arguments,
                                        environment: std::collections::BTreeMap::new(),
                                        current_dir: None,
                                    };
                                    let client = lsp_client::LspClient::spawn(command)
                                        .await
                                        .map_err(|error| error.to_string())?;
                                    let initialize = client
                                        .initialize(
                                            lsp_client::protocol::InitializeParams::default(),
                                        )
                                        .await
                                        .map_err(|error| error.to_string())?;
                                    client
                                        .initialized()
                                        .await
                                        .map_err(|error| error.to_string())?;
                                    let mut events = client.subscribe();
                                    let _ = event_sender.send(Event::LanguageServerReady {
                                        request,
                                        encoding: initialize.position_encoding,
                                    });
                                    let _ = event_sender.send(Event::EffectCompleted(request));
                                    while !shutdown.load(Ordering::SeqCst) {
                                        let Ok(event) = tokio::time::timeout(
                                            std::time::Duration::from_millis(100),
                                            events.recv(),
                                        )
                                        .await
                                        .unwrap_or(Err(
                                            tokio::sync::broadcast::error::RecvError::Closed,
                                        )) else {
                                            let _ = client.shutdown().await;
                                            break;
                                        };
                                        match event {
                                            lsp_client::ClientEvent::StderrLine(line) => {
                                                let _ = event_sender.send(Event::Output(
                                                    editor_types::OutputMessage {
                                                        subsystem: "lsp".to_owned(),
                                                        operation: "stderr".to_owned(),
                                                        level: editor_types::OutputLevel::Warning,
                                                        message: line,
                                                    },
                                                ));
                                            }
                                            lsp_client::ClientEvent::ProtocolError(message) => {
                                                let _ = event_sender.send(Event::Output(
                                                    editor_types::OutputMessage {
                                                        subsystem: "lsp".to_owned(),
                                                        operation: "protocol".to_owned(),
                                                        level: editor_types::OutputLevel::Error,
                                                        message,
                                                    },
                                                ));
                                            }
                                            lsp_client::ClientEvent::Crashed { message } => {
                                                let _ = event_sender.send(Event::EffectFailed {
                                                    request,
                                                    message: editor_types::OutputMessage {
                                                        subsystem: "lsp".to_owned(),
                                                        operation: "crash".to_owned(),
                                                        level: editor_types::OutputLevel::Error,
                                                        message,
                                                    },
                                                });
                                                break;
                                            }
                                            lsp_client::ClientEvent::Exited { status } => {
                                                let _ = event_sender.send(Event::EffectFailed {
                                                    request,
                                                    message: editor_types::OutputMessage {
                                                        subsystem: "lsp".to_owned(),
                                                        operation: "exit".to_owned(),
                                                        level: editor_types::OutputLevel::Error,
                                                        message: format!(
                                                            "language server exited with {status:?}"
                                                        ),
                                                    },
                                                });
                                                break;
                                            }
                                            lsp_client::ClientEvent::Diagnostics(params) => {
                                                let _ =
                                                    event_sender.send(Event::LanguageDiagnostics {
                                                        request,
                                                        params,
                                                    });
                                            }
                                            lsp_client::ClientEvent::Notification { .. }
                                            | lsp_client::ClientEvent::ServerRequest { .. } => {}
                                        }
                                    }
                                    Ok::<(), String>(())
                                })
                            });
                        if let Err(message) = result {
                            let _ = sender.send(Event::EffectFailed {
                                request,
                                message: editor_types::OutputMessage {
                                    subsystem: "lsp".to_owned(),
                                    operation: "startup".to_owned(),
                                    level: editor_types::OutputLevel::Error,
                                    message,
                                },
                            });
                        }
                    });
                    return;
                }
                self.runtime.spawn(async move {
                    let result = tokio::process::Command::new(&spec.executable)
                        .args(&spec.arguments)
                        .output()
                        .await;
                    let event = match result {
                        Ok(output) if output.status.success() => {
                            if !output.stdout.is_empty() || !output.stderr.is_empty() {
                                let _ = sender.send(Event::Output(editor_types::OutputMessage {
                                    subsystem: format!("{kind:?}").to_lowercase(),
                                    operation: "process-output".to_owned(),
                                    level: editor_types::OutputLevel::Information,
                                    message: format!(
                                        "{}{}",
                                        String::from_utf8_lossy(&output.stdout),
                                        String::from_utf8_lossy(&output.stderr)
                                    ),
                                }));
                            }
                            Event::EffectCompleted(request)
                        }
                        Ok(output) => Event::EffectFailed {
                            request,
                            message: editor_types::OutputMessage {
                                subsystem: format!("{kind:?}").to_lowercase(),
                                operation: "process".to_owned(),
                                level: editor_types::OutputLevel::Error,
                                message: format!(
                                    "{} exited with {}: {}",
                                    spec.executable,
                                    output.status,
                                    String::from_utf8_lossy(&output.stderr)
                                ),
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
        self.runtime.spawn_blocking(move || {
            let event = match workspace_core::save_text_document(
                &path,
                &text,
                &workspace_core::DocumentSaveOptions {
                    encoding,
                    with_bom,
                    line_endings,
                    ..workspace_core::DocumentSaveOptions::default()
                },
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

    use super::{StartupRequest, apply_workspace_settings};

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

    #[test]
    fn workspace_jsonc_settings_are_loaded_into_root_state() {
        let directory = tempfile::tempdir().expect("workspace");
        let vscode = directory.path().join(".vscode");
        std::fs::create_dir_all(&vscode).expect("settings directory");
        std::fs::write(
            vscode.join("settings.json"),
            "{\"editor.formatOnSave\": true, \"editor.tabSize\": 2}",
        )
        .expect("settings file");
        let mut state = super::AppState::default();
        state.workspace_roots.push(directory.path().to_path_buf());
        apply_workspace_settings(&mut state);
        assert!(state.format_on_save);
        assert_eq!(state.tab_width, 2);
    }
}
