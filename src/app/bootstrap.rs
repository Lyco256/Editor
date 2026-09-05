//! Process bootstrap and safe fallback startup path.

use std::{
    convert::Infallible,
    env,
    io::IsTerminal,
    path::{Path, PathBuf},
    process::ExitCode,
    sync::atomic::{AtomicBool, Ordering},
    sync::mpsc::{self, Receiver, Sender},
    sync::{Arc, Mutex},
};

use editor_types::{RequestId, TerminalCapabilities};
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
    let mut backend = terminal_backend::CrosstermBackend::stdout();
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
    if let Some(theme) = load_workspace_theme(&mut state) {
        backend.set_theme(theme);
    }
    apply_workspace_language_configuration(&mut state);
    apply_workspace_snippets(&mut state);
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
    let Some(root) = state.workspace_roots.first().cloned() else {
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

fn apply_workspace_language_configuration(state: &mut AppState) {
    let Some(root) = state.workspace_roots.first().cloned() else {
        return;
    };
    let path = root.join(".vscode").join("language-configuration.json");
    let contents = match std::fs::read_to_string(&path) {
        Ok(contents) => contents,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            apply_extension_language_configuration(state, &root);
            return;
        }
        Err(error) => {
            state.output.push(editor_types::OutputMessage {
                subsystem: "vscode-compat".to_owned(),
                operation: "language-configuration".to_owned(),
                level: editor_types::OutputLevel::Warning,
                message: format!("{}: {error}", path.display()),
            });
            return;
        }
    };
    match vscode_compat::load_language_configuration_json(&path, &contents) {
        Ok((configuration, warnings)) => {
            state.set_language_configuration(Some(&configuration));
            for warning in warnings {
                state.output.push(editor_types::OutputMessage {
                    subsystem: "vscode-compat".to_owned(),
                    operation: "language-configuration".to_owned(),
                    level: editor_types::OutputLevel::Warning,
                    message: format!("{}: {}", path.display(), warning.message),
                });
            }
        }
        Err(error) => state.output.push(editor_types::OutputMessage {
            subsystem: "vscode-compat".to_owned(),
            operation: "language-configuration".to_owned(),
            level: editor_types::OutputLevel::Error,
            message: format!("{}: {error}", path.display()),
        }),
    }
}

fn load_workspace_theme(state: &mut AppState) -> Option<terminal_backend::Theme> {
    let root = state.workspace_roots.first()?.clone();
    let theme_name = state.theme_name.clone();
    let mut candidates = Vec::new();
    let vscode = root.join(".vscode");
    candidates.push(vscode.join("theme.json"));
    let themes = vscode.join("themes");
    if let Ok(entries) = std::fs::read_dir(themes) {
        candidates.extend(
            entries
                .filter_map(Result::ok)
                .map(|entry| entry.path())
                .filter(|path| {
                    path.extension()
                        .is_some_and(|extension| extension == "json")
                }),
        );
    }
    for path in candidates {
        let Ok(contents) = std::fs::read_to_string(&path) else {
            continue;
        };
        match vscode_compat::load_theme_json(&path, &contents) {
            Ok((theme, warnings)) => {
                for warning in warnings {
                    state.output.push(editor_types::OutputMessage {
                        subsystem: "vscode-compat".to_owned(),
                        operation: "theme".to_owned(),
                        level: editor_types::OutputLevel::Warning,
                        message: warning.message,
                    });
                }
                if theme.name.eq_ignore_ascii_case(&theme_name) {
                    return Some(convert_static_theme(&theme));
                }
            }
            Err(error) => state.output.push(editor_types::OutputMessage {
                subsystem: "vscode-compat".to_owned(),
                operation: "theme".to_owned(),
                level: editor_types::OutputLevel::Warning,
                message: format!("{}: {error}", path.display()),
            }),
        }
    }
    let extension_root = vscode.join("extensions");
    let cache_root = std::env::temp_dir().join("editor-vsix-cache");
    for source in extension_candidates(&extension_root) {
        let package = match load_static_extension_candidate(&source, &cache_root) {
            Ok(package) => package,
            Err(error) => {
                state.output.push(editor_types::OutputMessage {
                    subsystem: "vscode-compat".to_owned(),
                    operation: "theme".to_owned(),
                    level: editor_types::OutputLevel::Warning,
                    message: format!("{}: {error}", source.display()),
                });
                continue;
            }
        };
        for warning in &package.warnings {
            state.output.push(editor_types::OutputMessage {
                subsystem: "vscode-compat".to_owned(),
                operation: "extension".to_owned(),
                level: editor_types::OutputLevel::Warning,
                message: warning.message.clone(),
            });
        }
        if let Some(theme) = package
            .themes
            .iter()
            .find(|theme| theme.name.eq_ignore_ascii_case(&theme_name))
        {
            return Some(convert_static_theme(theme));
        }
    }
    None
}

fn convert_static_theme(theme: &vscode_compat::StaticTheme) -> terminal_backend::Theme {
    let mut converted = terminal_backend::Theme::default();
    let mappings = [
        ("editor.foreground", editor_types::StyleRole::EditorText),
        (
            "editor.background",
            editor_types::StyleRole::EditorBackground,
        ),
        (
            "editor.selectionBackground",
            editor_types::StyleRole::Selection,
        ),
        (
            "editor.lineHighlightBackground",
            editor_types::StyleRole::CurrentLine,
        ),
        (
            "editorLineNumber.foreground",
            editor_types::StyleRole::LineNumber,
        ),
        ("editorGutter.background", editor_types::StyleRole::Gutter),
        ("panel.background", editor_types::StyleRole::Panel),
        ("statusBar.background", editor_types::StyleRole::StatusBar),
        ("editorError.foreground", editor_types::StyleRole::Error),
        ("editorWarning.foreground", editor_types::StyleRole::Warning),
        (
            "editorInfo.foreground",
            editor_types::StyleRole::Information,
        ),
        ("editorHint.foreground", editor_types::StyleRole::Hint),
        (
            "gitDecoration.addedResourceForeground",
            editor_types::StyleRole::GitAdded,
        ),
        (
            "gitDecoration.modifiedResourceForeground",
            editor_types::StyleRole::GitModified,
        ),
        (
            "gitDecoration.deletedResourceForeground",
            editor_types::StyleRole::GitDeleted,
        ),
        (
            "editor.findMatchBackground",
            editor_types::StyleRole::SearchMatch,
        ),
    ];
    for (name, role) in mappings {
        let Some(color) = theme.colors.get(name).and_then(theme_color_rgb) else {
            continue;
        };
        converted.set_color(role, color);
    }
    converted
}

fn theme_color_rgb(value: &vscode_compat::ThemeColor) -> Option<terminal_backend::RgbColor> {
    let hex = match value {
        vscode_compat::ThemeColor::Hex(hex) => hex.as_str(),
        vscode_compat::ThemeColor::Style(style) => style.foreground.as_deref()?,
    };
    let hex = hex.strip_prefix('#')?;
    let (red, green, blue) = match hex.len() {
        3 => (
            u8::from_str_radix(&hex[0..1].repeat(2), 16).ok()?,
            u8::from_str_radix(&hex[1..2].repeat(2), 16).ok()?,
            u8::from_str_radix(&hex[2..3].repeat(2), 16).ok()?,
        ),
        6 | 8 => (
            u8::from_str_radix(&hex[0..2], 16).ok()?,
            u8::from_str_radix(&hex[2..4], 16).ok()?,
            u8::from_str_radix(&hex[4..6], 16).ok()?,
        ),
        _ => return None,
    };
    Some(terminal_backend::RgbColor::new(red, green, blue))
}

fn apply_extension_language_configuration(state: &mut AppState, root: &Path) {
    let Some(active_path) = state.active_path.clone() else {
        return;
    };
    let language_id = syntax_engine::SyntaxLanguage::from_path(&active_path)
        .map(|language| language.name().to_owned());
    let extension_root = root.join(".vscode").join("extensions");
    let cache_root = std::env::temp_dir().join("editor-vsix-cache");
    for source in extension_candidates(&extension_root) {
        let package = match load_static_extension_candidate(&source, &cache_root) {
            Ok(package) => package,
            Err(error) => {
                state.output.push(editor_types::OutputMessage {
                    subsystem: "vscode-compat".to_owned(),
                    operation: "extension".to_owned(),
                    level: editor_types::OutputLevel::Warning,
                    message: format!("{}: {error}", source.display()),
                });
                continue;
            }
        };
        let Some(language) = package.languages.iter().find(|language| {
            language_id.as_deref().is_some_and(|id| {
                language.id.eq_ignore_ascii_case(id)
                    || language
                        .aliases
                        .iter()
                        .any(|alias| alias.eq_ignore_ascii_case(id))
            }) || static_language_matches_path(language, &active_path)
        }) else {
            continue;
        };
        if let Some(configuration) = language.configuration.as_ref() {
            state.set_language_configuration(Some(configuration));
            for warning in package.warnings {
                state.output.push(editor_types::OutputMessage {
                    subsystem: "vscode-compat".to_owned(),
                    operation: "extension".to_owned(),
                    level: editor_types::OutputLevel::Warning,
                    message: warning.message,
                });
            }
            return;
        }
    }
}

fn apply_workspace_snippets(state: &mut AppState) {
    let Some(root) = state.workspace_roots.first().cloned() else {
        return;
    };
    let language_id = state
        .active_path
        .as_deref()
        .and_then(syntax_engine::SyntaxLanguage::from_path)
        .map(|language| language.name().to_owned());
    let active_path = state.active_path.clone();
    let snippets_root = root.join(".vscode").join("snippets");
    if let Ok(entries) = std::fs::read_dir(snippets_root) {
        let mut files = entries
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| {
                path.extension()
                    .is_some_and(|extension| extension == "json")
            })
            .collect::<Vec<_>>();
        files.sort();
        for path in files {
            load_snippet_file(state, &path, language_id.as_deref());
        }
    }
    let extension_root = root.join(".vscode").join("extensions");
    let cache_root = std::env::temp_dir().join("editor-vsix-cache");
    for source in extension_candidates(&extension_root) {
        let package = match load_static_extension_candidate(&source, &cache_root) {
            Ok(package) => package,
            Err(error) => {
                state.output.push(editor_types::OutputMessage {
                    subsystem: "vscode-compat".to_owned(),
                    operation: "snippet".to_owned(),
                    level: editor_types::OutputLevel::Warning,
                    message: format!("{}: {error}", source.display()),
                });
                continue;
            }
        };
        for snippet_file in package.snippets {
            if snippet_file.language.as_deref().is_none_or(|language| {
                language_id
                    .as_deref()
                    .is_some_and(|active| active.eq_ignore_ascii_case(language))
                    || active_path.as_deref().is_some_and(|path| {
                        package.languages.iter().any(|language_spec| {
                            (language_spec.id.eq_ignore_ascii_case(language)
                                || language_spec
                                    .aliases
                                    .iter()
                                    .any(|alias| alias.eq_ignore_ascii_case(language)))
                                && static_language_matches_path(language_spec, path)
                        })
                    })
            }) {
                state.snippets.extend(snippet_file.snippets);
            }
        }
    }
}

fn static_language_matches_path(language: &vscode_compat::StaticLanguage, path: &Path) -> bool {
    if let Some(file_name) = path.file_name().and_then(|name| name.to_str()) {
        if language
            .filenames
            .iter()
            .any(|candidate| candidate.eq_ignore_ascii_case(file_name))
        {
            return true;
        }
    }
    let Some(extension) = path.extension().and_then(|extension| extension.to_str()) else {
        return false;
    };
    let extension = format!(".{extension}");
    language
        .extensions
        .iter()
        .any(|candidate| candidate.eq_ignore_ascii_case(&extension))
}

fn extension_candidates(root: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(root) else {
        return Vec::new();
    };
    let mut candidates = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.is_dir()
                || path
                    .extension()
                    .is_some_and(|extension| extension.eq_ignore_ascii_case("vsix"))
        })
        .collect::<Vec<_>>();
    candidates.sort();
    candidates
}

fn load_static_extension_candidate(
    source: &Path,
    cache_root: &Path,
) -> Result<vscode_compat::StaticExtensionPackage, vscode_compat::CompatibilityError> {
    if source.is_dir() {
        vscode_compat::load_static_extension_directory(source)
    } else {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        source.to_string_lossy().hash(&mut hasher);
        let archive_cache = cache_root.join(format!("{:016x}", hasher.finish()));
        vscode_compat::load_static_extension_vsix(source, archive_cache)
    }
}

fn load_snippet_file(state: &mut AppState, path: &Path, language_id: Option<&str>) {
    let contents = match std::fs::read_to_string(path) {
        Ok(contents) => contents,
        Err(error) => {
            state.output.push(editor_types::OutputMessage {
                subsystem: "vscode-compat".to_owned(),
                operation: "snippet".to_owned(),
                level: editor_types::OutputLevel::Warning,
                message: format!("{}: {error}", path.display()),
            });
            return;
        }
    };
    match vscode_compat::load_snippet_json(path, &contents) {
        Ok((snippet_file, warnings)) => {
            for warning in warnings {
                state.output.push(editor_types::OutputMessage {
                    subsystem: "vscode-compat".to_owned(),
                    operation: "snippet".to_owned(),
                    level: editor_types::OutputLevel::Warning,
                    message: warning.message,
                });
            }
            if snippet_file.language.as_deref().is_none_or(|language| {
                language_id.is_some_and(|active| active.eq_ignore_ascii_case(language))
            }) {
                state.snippets.extend(snippet_file.snippets);
            }
        }
        Err(error) => state.output.push(editor_types::OutputMessage {
            subsystem: "vscode-compat".to_owned(),
            operation: "snippet".to_owned(),
            level: editor_types::OutputLevel::Warning,
            message: format!("{}: {error}", path.display()),
        }),
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

fn lsp_workspace_edit_paths(edit: &serde_json::Value) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    if let Some(changes) = edit.get("changes").and_then(serde_json::Value::as_object) {
        paths.extend(changes.keys().map(|uri| lsp_uri_path(uri)));
    }
    if let Some(document_changes) = edit
        .get("documentChanges")
        .and_then(serde_json::Value::as_array)
    {
        paths.extend(document_changes.iter().flat_map(|change| {
            if change.get("kind").is_some() {
                match change.get("kind").and_then(serde_json::Value::as_str) {
                    Some("rename") => change
                        .get("oldUri")
                        .and_then(serde_json::Value::as_str)
                        .into_iter()
                        .chain(change.get("newUri").and_then(serde_json::Value::as_str))
                        .map(lsp_uri_path)
                        .collect::<Vec<_>>(),
                    _ => change
                        .get("uri")
                        .and_then(serde_json::Value::as_str)
                        .into_iter()
                        .map(lsp_uri_path)
                        .collect::<Vec<_>>(),
                }
            } else {
                change
                    .get("textDocument")
                    .and_then(|document| document.get("uri"))
                    .and_then(serde_json::Value::as_str)
                    .into_iter()
                    .map(lsp_uri_path)
                    .collect::<Vec<_>>()
            }
        }));
    }
    paths.sort_by_key(|path| workspace_core::canonical_workspace_key(path));
    paths.dedup_by(|left, right| workspace_core::path_eq(left, right));
    paths
}

fn lsp_workspace_text_paths(edit: &serde_json::Value) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    if let Some(changes) = edit.get("changes").and_then(serde_json::Value::as_object) {
        paths.extend(changes.keys().map(|uri| lsp_uri_path(uri)));
    }
    if let Some(document_changes) = edit
        .get("documentChanges")
        .and_then(serde_json::Value::as_array)
    {
        paths.extend(document_changes.iter().filter_map(|change| {
            change
                .get("textDocument")
                .and_then(|document| document.get("uri"))
                .and_then(serde_json::Value::as_str)
                .map(lsp_uri_path)
        }));
    }
    paths.sort_by_key(|path| workspace_core::canonical_workspace_key(path));
    paths.dedup_by(|left, right| workspace_core::path_eq(left, right));
    paths
}

fn lsp_path_within_roots(path: &Path, roots: &[PathBuf]) -> bool {
    let candidate = if path.exists() {
        std::fs::canonicalize(path).ok()
    } else {
        path.parent().and_then(|parent| {
            std::fs::canonicalize(parent)
                .ok()
                .and_then(|parent| path.file_name().map(|name| parent.join(name)))
        })
    };
    let Some(candidate) = candidate else {
        return false;
    };
    roots
        .iter()
        .any(|root| std::fs::canonicalize(root).is_ok_and(|root| candidate.starts_with(root)))
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum LspResourceOperation {
    Create {
        path: PathBuf,
        overwrite: bool,
        ignore_if_exists: bool,
    },
    Rename {
        old_path: PathBuf,
        new_path: PathBuf,
        overwrite: bool,
        ignore_if_exists: bool,
    },
    Delete {
        path: PathBuf,
        recursive: bool,
        ignore_if_not_exists: bool,
    },
}

#[derive(Debug, Clone)]
struct ResourceBackup {
    path: PathBuf,
    entries: Option<Vec<ResourceEntry>>,
}

type ResourceEntry = (PathBuf, bool, Vec<u8>);

fn snapshot_visit(
    path: &Path,
    root: &Path,
    entries: &mut Vec<ResourceEntry>,
) -> Result<(), String> {
    let metadata = std::fs::metadata(path)
        .map_err(|error| format!("could not inspect {}: {error}", path.display()))?;
    let relative = path.strip_prefix(root).unwrap_or(Path::new(""));
    if metadata.is_dir() {
        entries.push((relative.to_path_buf(), true, Vec::new()));
        for child in std::fs::read_dir(path)
            .map_err(|error| format!("could not enumerate {}: {error}", path.display()))?
        {
            let child = child
                .map_err(|error| format!("could not enumerate {}: {error}", path.display()))?;
            snapshot_visit(&child.path(), root, entries)?;
        }
    } else {
        entries.push((
            relative.to_path_buf(),
            false,
            std::fs::read(path)
                .map_err(|error| format!("could not back up {}: {error}", path.display()))?,
        ));
    }
    Ok(())
}

fn snapshot_resource(path: &Path) -> Result<Option<Vec<ResourceEntry>>, String> {
    if !path.exists() {
        return Ok(None);
    }
    let mut entries = Vec::new();
    snapshot_visit(path, path, &mut entries)?;
    Ok(Some(entries))
}

fn remove_resource(path: &Path) -> Result<(), std::io::Error> {
    if path.is_dir() {
        std::fs::remove_dir_all(path)
    } else {
        std::fs::remove_file(path)
    }
}

fn restore_resource_backups(backups: &[ResourceBackup]) -> Result<(), String> {
    let mut failures = Vec::new();
    for backup in backups.iter().rev() {
        if backup.path.exists() {
            if let Err(error) = remove_resource(&backup.path) {
                failures.push(format!(
                    "could not remove {}: {error}",
                    backup.path.display()
                ));
                continue;
            }
        }
        let Some(entries) = &backup.entries else {
            continue;
        };
        for (relative, is_directory, bytes) in entries {
            let destination = if relative.as_os_str().is_empty() {
                backup.path.clone()
            } else {
                backup.path.join(relative)
            };
            if *is_directory {
                if let Err(error) = std::fs::create_dir_all(&destination) {
                    failures.push(format!(
                        "could not restore {}: {error}",
                        destination.display()
                    ));
                }
            } else {
                if let Some(parent) = destination.parent() {
                    if let Err(error) = std::fs::create_dir_all(parent) {
                        failures.push(format!("could not restore {}: {error}", parent.display()));
                        continue;
                    }
                }
                if let Err(error) = std::fs::write(&destination, bytes) {
                    failures.push(format!(
                        "could not restore {}: {error}",
                        destination.display()
                    ));
                }
            }
        }
    }
    if failures.is_empty() {
        Ok(())
    } else {
        Err(failures.join("; "))
    }
}

fn lsp_resource_operations(edit: &serde_json::Value) -> Result<Vec<LspResourceOperation>, String> {
    let Some(changes) = edit
        .get("documentChanges")
        .and_then(serde_json::Value::as_array)
    else {
        return Ok(Vec::new());
    };
    let mut operations = Vec::new();
    for change in changes {
        let Some(kind) = change.get("kind").and_then(serde_json::Value::as_str) else {
            continue;
        };
        let options = change.get("options");
        let bool_option = |name: &str| {
            options
                .and_then(|value| value.get(name))
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false)
        };
        let operation = match kind {
            "create" => LspResourceOperation::Create {
                path: lsp_uri_path(
                    change
                        .get("uri")
                        .and_then(serde_json::Value::as_str)
                        .ok_or_else(|| "create operation omitted uri".to_owned())?,
                ),
                overwrite: bool_option("overwrite"),
                ignore_if_exists: bool_option("ignoreIfExists"),
            },
            "rename" => LspResourceOperation::Rename {
                old_path: lsp_uri_path(
                    change
                        .get("oldUri")
                        .and_then(serde_json::Value::as_str)
                        .ok_or_else(|| "rename operation omitted oldUri".to_owned())?,
                ),
                new_path: lsp_uri_path(
                    change
                        .get("newUri")
                        .and_then(serde_json::Value::as_str)
                        .ok_or_else(|| "rename operation omitted newUri".to_owned())?,
                ),
                overwrite: bool_option("overwrite"),
                ignore_if_exists: bool_option("ignoreIfExists"),
            },
            "delete" => LspResourceOperation::Delete {
                path: lsp_uri_path(
                    change
                        .get("uri")
                        .and_then(serde_json::Value::as_str)
                        .ok_or_else(|| "delete operation omitted uri".to_owned())?,
                ),
                recursive: bool_option("recursive"),
                ignore_if_not_exists: bool_option("ignoreIfNotExists"),
            },
            other => return Err(format!("unsupported workspace resource operation: {other}")),
        };
        operations.push(operation);
    }
    Ok(operations)
}

fn lsp_uri_path(uri: &str) -> PathBuf {
    if let Some(path) = uri.strip_prefix("file:///") {
        let decoded = percent_decode_uri_path(path);
        #[cfg(windows)]
        {
            return PathBuf::from(decoded.replace('/', std::path::MAIN_SEPARATOR_STR));
        }
        #[cfg(not(windows))]
        {
            return PathBuf::from(format!(
                "/{}",
                decoded.replace('/', std::path::MAIN_SEPARATOR_STR)
            ));
        }
    }
    if let Some(path) = uri.strip_prefix("file://") {
        let decoded = percent_decode_uri_path(path);
        #[cfg(windows)]
        {
            return PathBuf::from(format!(
                "\\\\{}",
                decoded.replace('/', std::path::MAIN_SEPARATOR_STR)
            ));
        }
        #[cfg(not(windows))]
        {
            return PathBuf::from(format!(
                "//{}",
                decoded.replace('/', std::path::MAIN_SEPARATOR_STR)
            ));
        }
    }
    PathBuf::from(percent_decode_uri_path(uri).replace('/', std::path::MAIN_SEPARATOR_STR))
}

fn percent_decode_uri_path(path: &str) -> String {
    let bytes = path.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' && index + 2 < bytes.len() {
            let high = bytes[index + 1].to_ascii_lowercase();
            let low = bytes[index + 2].to_ascii_lowercase();
            let digit = |value: u8| match value {
                b'0'..=b'9' => Some(value - b'0'),
                b'a'..=b'f' => Some(value - b'a' + 10),
                _ => None,
            };
            if let (Some(high), Some(low)) = (digit(high), digit(low)) {
                decoded.push(high * 16 + low);
                index += 3;
                continue;
            }
        }
        decoded.push(bytes[index]);
        index += 1;
    }
    String::from_utf8_lossy(&decoded).into_owned()
}

fn lsp_workspace_edits_for_path(
    edit: &serde_json::Value,
    path: &std::path::Path,
) -> Vec<serde_json::Value> {
    let mut edits = Vec::new();
    if let Some(changes) = edit.get("changes").and_then(serde_json::Value::as_object) {
        for (uri, document_edits) in changes {
            if workspace_core::path_eq(&lsp_uri_path(uri), path) {
                edits.extend(document_edits.as_array().into_iter().flatten().cloned());
            }
        }
    }
    if let Some(document_changes) = edit
        .get("documentChanges")
        .and_then(serde_json::Value::as_array)
    {
        for document_change in document_changes {
            let Some(uri) = document_change
                .get("textDocument")
                .and_then(|document| document.get("uri"))
                .and_then(serde_json::Value::as_str)
            else {
                continue;
            };
            if workspace_core::path_eq(&lsp_uri_path(uri), path) {
                edits.extend(
                    document_change
                        .get("edits")
                        .and_then(serde_json::Value::as_array)
                        .into_iter()
                        .flatten()
                        .cloned(),
                );
            }
        }
    }
    edits
}

#[allow(clippy::too_many_lines)]
fn apply_lsp_workspace_edit(
    edit: &serde_json::Value,
    roots: &[PathBuf],
    encoding: lsp_client::protocol::PositionEncoding,
) -> Result<Vec<workspace_core::TextDocument>, String> {
    let paths = lsp_workspace_edit_paths(edit);
    if paths.is_empty() {
        return Err("workspace edit did not contain document or resource changes".to_owned());
    }
    for path in &paths {
        if !lsp_path_within_roots(path, roots) {
            return Err(format!(
                "workspace edit targeted an untrusted path: {}",
                path.display()
            ));
        }
    }
    let resources = lsp_resource_operations(edit)?;
    let mut backups = Vec::<ResourceBackup>::new();
    for operation in &resources {
        let operation_paths = match operation {
            LspResourceOperation::Create { path, .. }
            | LspResourceOperation::Delete { path, .. } => vec![path.clone()],
            LspResourceOperation::Rename {
                old_path, new_path, ..
            } => vec![old_path.clone(), new_path.clone()],
        };
        for path in operation_paths {
            if backups
                .iter()
                .any(|existing| workspace_core::path_eq(&existing.path, &path))
            {
                continue;
            }
            backups.push(ResourceBackup {
                entries: snapshot_resource(&path)?,
                path,
            });
        }
    }
    for operation in &resources {
        let result = match operation {
            LspResourceOperation::Create {
                path,
                overwrite,
                ignore_if_exists,
            } => {
                if path.exists() {
                    if *ignore_if_exists {
                        Ok(())
                    } else if !overwrite {
                        Err(format!("create target already exists: {}", path.display()))
                    } else if path.is_dir() {
                        Err(format!(
                            "create target is a directory and cannot be overwritten as a file: {}",
                            path.display()
                        ))
                    } else {
                        std::fs::remove_file(path).map_err(|error| error.to_string())?;
                        std::fs::write(path, []).map_err(|error| error.to_string())
                    }
                } else {
                    if let Some(parent) = path.parent() {
                        std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
                    }
                    std::fs::write(path, []).map_err(|error| error.to_string())
                }
            }
            LspResourceOperation::Rename {
                old_path,
                new_path,
                overwrite,
                ignore_if_exists,
            } => {
                if !old_path.exists() {
                    Err(format!(
                        "rename source does not exist: {}",
                        old_path.display()
                    ))
                } else if new_path.exists() && *ignore_if_exists {
                    Ok(())
                } else if new_path.exists() && !overwrite {
                    Err(format!(
                        "rename target already exists: {}",
                        new_path.display()
                    ))
                } else {
                    if new_path.exists() {
                        remove_resource(new_path).map_err(|error| error.to_string())?;
                    }
                    std::fs::rename(old_path, new_path).map_err(|error| error.to_string())
                }
            }
            LspResourceOperation::Delete {
                path,
                ignore_if_not_exists,
                recursive,
            } => {
                if !path.exists() && *ignore_if_not_exists {
                    Ok(())
                } else if !path.exists() {
                    Err(format!("delete target does not exist: {}", path.display()))
                } else if path.is_dir() && !recursive {
                    Err(format!(
                        "delete target is a directory and recursive=false: {}",
                        path.display()
                    ))
                } else {
                    remove_resource(path).map_err(|error| error.to_string())
                }
            }
        };
        if let Err(error) = result {
            return Err(match restore_resource_backups(&backups) {
                Ok(()) => format!("workspace resource operation failed: {error}"),
                Err(rollback_error) => format!(
                    "workspace resource operation failed: {error}; rollback failed: {rollback_error}"
                ),
            });
        }
    }
    let text_result = (|| {
        let mut prepared = Vec::new();
        for path in lsp_workspace_text_paths(edit) {
            let document = workspace_core::load_text_document(
                &path,
                &workspace_core::DocumentLoadOptions::default(),
            )
            .map_err(|error| format!("could not load {}: {error}", path.display()))?;
            let edits = lsp_workspace_edits_for_path(edit, &path);
            if edits.is_empty() {
                return Err(format!(
                    "workspace edit contained no text edits for {}",
                    path.display()
                ));
            }
            let snapshot = lsp_client::TextSnapshot::new(document.text.clone());
            let mut converted = Vec::new();
            for value in edits {
                let range = value
                    .get("range")
                    .ok_or_else(|| "workspace edit omitted a range".to_owned())?;
                let start: lsp_client::protocol::Position = serde_json::from_value(
                    range
                        .get("start")
                        .cloned()
                        .ok_or_else(|| "workspace edit omitted a start position".to_owned())?,
                )
                .map_err(|error| format!("invalid workspace edit start position: {error}"))?;
                let end: lsp_client::protocol::Position = serde_json::from_value(
                    range
                        .get("end")
                        .cloned()
                        .ok_or_else(|| "workspace edit omitted an end position".to_owned())?,
                )
                .map_err(|error| format!("invalid workspace edit end position: {error}"))?;
                let start = lsp_client::PositionMapper::new(&snapshot, encoding)
                    .from_lsp(start)
                    .map_err(|error| format!("could not map workspace edit start: {error}"))?;
                let end = lsp_client::PositionMapper::new(&snapshot, encoding)
                    .from_lsp(end)
                    .map_err(|error| format!("could not map workspace edit end: {error}"))?;
                let buffer = editor_core::TextBuffer::new(&document.text);
                let start = buffer
                    .position_to_offset(start)
                    .map_err(|error| format!("invalid workspace edit start: {error}"))?;
                let end = buffer
                    .position_to_offset(end)
                    .map_err(|error| format!("invalid workspace edit end: {error}"))?;
                let replacement = value
                    .get("newText")
                    .and_then(serde_json::Value::as_str)
                    .ok_or_else(|| "workspace edit omitted replacement text".to_owned())?;
                converted.push(editor_core::Edit::replace(
                    editor_types::TextRange { start, end },
                    replacement,
                ));
            }
            let transaction = editor_core::Transaction::new(converted)
                .map_err(|error| format!("invalid workspace edit transaction: {error}"))?;
            let mut buffer = editor_core::TextBuffer::new(&document.text);
            buffer
                .apply_transaction(transaction)
                .map_err(|error| format!("could not apply workspace edit: {error}"))?;
            let mut updated = document;
            updated.text = buffer.to_string();
            workspace_core::save_text_document(
                &updated.path,
                &updated.text,
                &workspace_core::DocumentSaveOptions {
                    encoding: updated.encoding.clone(),
                    with_bom: updated.had_bom,
                    line_endings: updated.line_endings,
                    ..workspace_core::DocumentSaveOptions::default()
                },
            )
            .map_err(|error| format!("could not save {}: {error}", updated.path.display()))?;
            prepared.push(updated);
        }
        Ok::<_, String>(prepared)
    })();
    match text_result {
        Ok(prepared) => Ok(prepared),
        Err(error) => Err(match restore_resource_backups(&backups) {
            Ok(()) => error,
            Err(rollback_error) => format!("{error}; rollback failed: {rollback_error}"),
        }),
    }
}

struct ServiceDispatcher {
    events: Receiver<Event>,
    sender: Sender<Event>,
    shutdown: Arc<AtomicBool>,
    runtime: tokio::runtime::Runtime,
    syntax: Arc<Mutex<syntax_engine::SyntaxEngine>>,
    searches: Arc<Mutex<std::collections::HashMap<u64, workspace_core::SearchCancellation>>>,
    lsp_clients: Arc<Mutex<std::collections::HashMap<RequestId, lsp_client::LspClient>>>,
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
            lsp_clients: Arc::new(Mutex::new(std::collections::HashMap::new())),
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
            if let Effect::RefreshExplorer {
                request,
                roots,
                expanded,
            } = effect.clone()
            {
                let sender = self.sender.clone();
                self.runtime.spawn_blocking(move || {
                    let mut entries = Vec::new();
                    let mut failure = None;
                    for root in roots {
                        match workspace_core::canonicalize_path(&root) {
                            Ok(canonical) => {
                                let tree = workspace_core::ExplorerTree::new(
                                    vec![canonical.clone()],
                                    Vec::new(),
                                );
                                if let Err(error) = collect_explorer_entries(
                                    &tree,
                                    canonical.as_path(),
                                    1,
                                    &expanded,
                                    &mut entries,
                                ) {
                                    failure = Some(error.to_string());
                                }
                            }
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
                let persistent = self
                    .lsp_clients
                    .lock()
                    .ok()
                    .and_then(|clients| clients.values().next().cloned());
                if let Some(client) = persistent {
                    self.runtime.spawn(async move {
                        match client.request_json(&method, params).await {
                            Ok(result) => {
                                let _ = sender.send(Event::LspResponse {
                                    request,
                                    version,
                                    method,
                                    result,
                                });
                            }
                            Err(error) => {
                                let _ = sender.send(Event::EffectFailed {
                                    request,
                                    message: editor_types::OutputMessage {
                                        subsystem: "lsp".to_owned(),
                                        operation: "request".to_owned(),
                                        level: editor_types::OutputLevel::Error,
                                        message: error.to_string(),
                                    },
                                });
                            }
                        }
                    });
                    return;
                }
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
            if let Effect::LspNotification {
                request,
                method,
                params,
            } = effect.clone()
            {
                let sender = self.sender.clone();
                let client = self
                    .lsp_clients
                    .lock()
                    .ok()
                    .and_then(|clients| clients.values().next().cloned());
                self.runtime.spawn(async move {
                    let result = match client {
                        Some(client) => client
                            .notify_json(&method, params)
                            .await
                            .map_err(|error| error.to_string()),
                        None => Err("language server session is no longer available".to_owned()),
                    };
                    let event = result.map_or_else(
                        |message| Event::EffectFailed {
                            request,
                            message: editor_types::OutputMessage {
                                subsystem: "lsp".to_owned(),
                                operation: "notification".to_owned(),
                                level: editor_types::OutputLevel::Warning,
                                message,
                            },
                        },
                        |()| Event::EffectCompleted(request),
                    );
                    let _ = sender.send(event);
                });
                return;
            }
            if let Effect::LspServerResponse {
                request,
                id,
                result,
                error,
            } = effect.clone()
            {
                let sender = self.sender.clone();
                let clients = self.lsp_clients.clone();
                self.runtime.spawn(async move {
                    let client = clients
                        .lock()
                        .ok()
                        .and_then(|map| map.get(&request).cloned());
                    let Some(client) = client else {
                        let _ = sender.send(Event::EffectFailed {
                            request,
                            message: editor_types::OutputMessage {
                                subsystem: "lsp".to_owned(),
                                operation: "server-response".to_owned(),
                                level: editor_types::OutputLevel::Error,
                                message: "language server session is no longer available"
                                    .to_owned(),
                            },
                        });
                        return;
                    };
                    if let Err(error) = client.respond(id, result, error).await {
                        let _ = sender.send(Event::EffectFailed {
                            request,
                            message: editor_types::OutputMessage {
                                subsystem: "lsp".to_owned(),
                                operation: "server-response".to_owned(),
                                level: editor_types::OutputLevel::Error,
                                message: error.to_string(),
                            },
                        });
                    }
                });
                return;
            }
            if let Effect::LspWorkspaceEdit {
                request,
                id,
                edit,
                roots,
                encoding,
            } = effect.clone()
            {
                let sender = self.sender.clone();
                self.runtime.spawn_blocking(move || {
                    let outcome = apply_lsp_workspace_edit(&edit, &roots, encoding);
                    let event = match outcome {
                        Ok(documents) => Event::LspWorkspaceEditCompleted {
                            request,
                            id,
                            applied: true,
                            failure_reason: None,
                            documents,
                        },
                        Err(message) => Event::LspWorkspaceEditCompleted {
                            request,
                            id,
                            applied: false,
                            failure_reason: Some(message),
                            documents: Vec::new(),
                        },
                    };
                    let _ = sender.send(event);
                });
                return;
            }
            if let Effect::LspApplyWorkspaceEdit {
                request,
                edit,
                roots,
                encoding,
            } = effect.clone()
            {
                let sender = self.sender.clone();
                self.runtime.spawn_blocking(move || {
                    let event = match apply_lsp_workspace_edit(&edit, &roots, encoding) {
                        Ok(documents) => Event::LspWorkspaceEditApplied {
                            request,
                            applied: true,
                            failure_reason: None,
                            documents,
                        },
                        Err(message) => Event::LspWorkspaceEditApplied {
                            request,
                            applied: false,
                            failure_reason: Some(message),
                            documents: Vec::new(),
                        },
                    };
                    let _ = sender.send(event);
                });
                return;
            }
            if let Effect::StopLanguageServer { request } = effect.clone() {
                let sender = self.sender.clone();
                let clients = self.lsp_clients.clone();
                self.runtime.spawn(async move {
                    let sessions = clients
                        .lock()
                        .map(|mut clients| {
                            clients
                                .drain()
                                .map(|(_, client)| client)
                                .collect::<Vec<_>>()
                        })
                        .unwrap_or_default();
                    let mut failure = None;
                    for client in sessions {
                        if let Err(error) = client.shutdown().await {
                            failure = Some(error.to_string());
                        }
                    }
                    let event = failure.map_or(Event::EffectCompleted(request), |message| {
                        Event::EffectFailed {
                            request,
                            message: editor_types::OutputMessage {
                                subsystem: "lsp".to_owned(),
                                operation: "shutdown".to_owned(),
                                level: editor_types::OutputLevel::Warning,
                                message,
                            },
                        }
                    });
                    let _ = sender.send(event);
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
                                .await?;
                            let branches = client.branches(&root, None).await?;
                            let stashes = client.stash_list(&root, None).await?;
                            let history = client.log(&root, 50, None).await?;
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
                    let clients = self.lsp_clients.clone();
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
                                    if let Ok(mut sessions) = clients.lock() {
                                        sessions.insert(request, client.clone());
                                    }
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
                                            lsp_client::ClientEvent::Notification { .. } => {}
                                            lsp_client::ClientEvent::ServerRequest {
                                                id,
                                                method,
                                                params,
                                            } => {
                                                let _ =
                                                    event_sender.send(Event::LspServerRequest {
                                                        request,
                                                        id,
                                                        method,
                                                        params,
                                                    });
                                            }
                                        }
                                    }
                                    if let Ok(mut sessions) = clients.lock() {
                                        sessions.remove(&request);
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

fn collect_explorer_entries(
    tree: &workspace_core::ExplorerTree,
    directory: &Path,
    depth: u8,
    expanded: &[PathBuf],
    output: &mut Vec<super::event::ExplorerEntryData>,
) -> Result<(), workspace_core::FileOperationError> {
    for entry in tree.children(directory)? {
        let is_directory = entry.kind == workspace_core::ExplorerEntryKind::Directory;
        output.push(super::event::ExplorerEntryData {
            path: entry.path.clone(),
            depth,
            is_directory,
        });
        if is_directory
            && expanded
                .iter()
                .any(|path| workspace_core::path_eq(path, &entry.path))
        {
            collect_explorer_entries(tree, &entry.path, depth.saturating_add(1), expanded, output)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;

    use super::{
        StartupRequest, apply_lsp_workspace_edit, apply_workspace_language_configuration,
        apply_workspace_settings, apply_workspace_snippets, convert_static_theme,
        load_workspace_theme, lsp_uri_path,
    };

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

    #[test]
    fn workspace_language_configuration_is_applied_to_smart_editing() {
        let directory = tempfile::tempdir().expect("workspace");
        let vscode = directory.path().join(".vscode");
        std::fs::create_dir_all(&vscode).expect("settings directory");
        std::fs::write(
            vscode.join("language-configuration.json"),
            r#"{"brackets":[["<",">"]],"autoClosingPairs":[["<",">"]],"indentationRules":{"increaseIndentPattern":"\\{$"}}"#,
        )
        .expect("language configuration");
        let mut state = super::AppState::default();
        state.workspace_roots.push(directory.path().to_path_buf());
        apply_workspace_language_configuration(&mut state);
        assert_eq!(state.pair_config.len(), 1);
        assert_eq!(state.pair_config[0].open, "<");
        assert_eq!(state.pair_config[0].close, ">");
        assert_eq!(
            state
                .indentation_rules
                .as_ref()
                .and_then(|rules| rules.increase_indent_pattern.as_deref()),
            Some(r"\{$")
        );
    }

    #[test]
    fn workspace_snippets_are_loaded_for_active_language() {
        let directory = tempfile::tempdir().expect("workspace");
        let snippets = directory.path().join(".vscode").join("snippets");
        std::fs::create_dir_all(&snippets).expect("snippet directory");
        std::fs::write(
            snippets.join("rust.json"),
            r#"{"main":{"prefix":"main","body":["fn main() {","\\t$0","}"]}}"#,
        )
        .expect("snippet file");
        let mut state = super::AppState::default();
        state.workspace_roots.push(directory.path().to_path_buf());
        state.active_path = Some(directory.path().join("main.rs"));
        apply_workspace_snippets(&mut state);
        assert_eq!(state.snippets.len(), 1);
        assert_eq!(state.snippets[0].prefix, vec!["main".to_owned()]);
    }

    #[test]
    fn static_vsix_theme_is_loaded_without_executing_extension_code() {
        let directory = tempfile::tempdir().expect("workspace");
        let extensions = directory.path().join(".vscode").join("extensions");
        std::fs::create_dir_all(&extensions).expect("extension directory");
        let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("fixtures")
            .join("vscode-compat")
            .join("static-extension.vsix");
        std::fs::copy(&fixture, extensions.join("static-compat-fixture.vsix"))
            .expect("copy VSIX fixture");
        let mut state = super::AppState::default();
        state.workspace_roots.push(directory.path().to_path_buf());
        state.theme_name = "Fixture Dark".to_owned();
        let theme = load_workspace_theme(&mut state).expect("VSIX theme");
        assert_eq!(
            theme.color(editor_types::StyleRole::EditorText),
            terminal_backend::RgbColor::new(0xd4, 0xd4, 0xd4)
        );
    }

    #[test]
    fn static_extension_language_mapping_applies_to_custom_extensions() {
        let directory = tempfile::tempdir().expect("workspace");
        let extensions = directory.path().join(".vscode").join("extensions");
        std::fs::create_dir_all(&extensions).expect("extension directory");
        let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("fixtures")
            .join("vscode-compat")
            .join("static-extension.vsix");
        std::fs::copy(&fixture, extensions.join("static-compat-fixture.vsix"))
            .expect("copy VSIX fixture");
        let mut state = super::AppState::default();
        state.workspace_roots.push(directory.path().to_path_buf());
        state.active_path = Some(directory.path().join("main.toy"));
        state.buffer = editor_core::TextBuffer::new("value");
        apply_workspace_language_configuration(&mut state);
        let _ = state.apply_action(crate::app::action::Action::Input(
            editor_types::InputEvent::Key(editor_types::KeyEvent {
                code: editor_types::KeyCode::Character('/'),
                modifiers: editor_types::Modifiers::from_modifiers([
                    editor_types::Modifier::Control,
                ]),
                repeat: false,
            }),
        ));
        assert_eq!(state.active_text, "// value");
        apply_workspace_snippets(&mut state);
        assert!(
            state
                .snippets
                .iter()
                .any(|snippet| snippet.prefix.iter().any(|prefix| prefix == "for"))
        );
    }

    #[test]
    fn static_theme_maps_known_semantic_roles() {
        let (theme, _) = vscode_compat::load_theme_json(
            std::path::Path::new("theme.json"),
            r##"{
                "name": "Custom",
                "type": "dark",
                "colors": {
                    "editor.foreground": "#010203",
                    "editor.background": "#0a0b0c"
                }
            }"##,
        )
        .expect("theme");
        let converted = convert_static_theme(&theme);
        assert_eq!(
            converted.color(editor_types::StyleRole::EditorText),
            terminal_backend::RgbColor::new(1, 2, 3)
        );
        assert_eq!(
            converted.color(editor_types::StyleRole::EditorBackground),
            terminal_backend::RgbColor::new(10, 11, 12)
        );
    }

    #[test]
    fn lsp_file_uri_round_trips_absolute_paths() {
        let path = if cfg!(windows) {
            std::path::PathBuf::from(r"C:\workspace\space file.rs")
        } else {
            std::path::PathBuf::from("/workspace/space file.rs")
        };
        let uri = lsp_client::protocol::DocumentUri::from_path(&path);
        assert!(workspace_core::path_eq(&lsp_uri_path(&uri.0), &path));
    }

    #[test]
    fn server_workspace_edit_worker_applies_unopened_document_atomically() {
        let directory = tempfile::tempdir().expect("workspace");
        let path = directory.path().join("server.rs");
        std::fs::write(&path, "old\n").expect("source");
        let uri = format!("file:///{}", path.to_string_lossy().replace('\\', "/"));
        let edit = serde_json::json!({
            "changes": {
                uri: [{
                    "range": {"start": {"line": 0, "character": 0}, "end": {"line": 0, "character": 3}},
                    "newText": "new"
                }]
            }
        });
        let documents = apply_lsp_workspace_edit(
            &edit,
            &[directory.path().to_path_buf()],
            lsp_client::protocol::PositionEncoding::Utf16,
        )
        .expect("workspace edit");
        assert_eq!(documents.len(), 1);
        assert_eq!(documents[0].text, "new\n");
        assert_eq!(
            std::fs::read_to_string(path).expect("saved source"),
            "new\n"
        );
    }

    #[test]
    fn server_workspace_edit_worker_applies_file_resource_operations() {
        let directory = tempfile::tempdir().expect("workspace");
        let source = directory.path().join("old.txt");
        let target = directory.path().join("new.txt");
        std::fs::write(&source, "content").expect("source");
        let uri = |path: &std::path::Path| {
            format!("file:///{}", path.to_string_lossy().replace('\\', "/"))
        };
        let edit = serde_json::json!({
            "documentChanges": [
                {"kind": "rename", "oldUri": uri(&source), "newUri": uri(&target)},
                {"kind": "create", "uri": uri(&directory.path().join("created.txt"))}
            ]
        });
        let documents = apply_lsp_workspace_edit(
            &edit,
            &[directory.path().to_path_buf()],
            lsp_client::protocol::PositionEncoding::Utf16,
        )
        .expect("resource operations");
        assert!(documents.is_empty());
        assert!(!source.exists());
        assert_eq!(
            std::fs::read_to_string(&target).expect("renamed source"),
            "content"
        );
        assert!(directory.path().join("created.txt").is_file());
    }

    #[test]
    fn server_workspace_edit_worker_applies_directory_resource_operations() {
        let directory = tempfile::tempdir().expect("workspace");
        let source = directory.path().join("old-dir");
        let target = directory.path().join("new-dir");
        std::fs::create_dir_all(source.join("nested")).expect("source directory");
        std::fs::write(source.join("nested").join("file.txt"), "content").expect("file");
        let uri = |path: &std::path::Path| {
            format!("file:///{}", path.to_string_lossy().replace('\\', "/"))
        };
        let rename = serde_json::json!({
            "documentChanges": [{
                "kind": "rename", "oldUri": uri(&source), "newUri": uri(&target)
            }]
        });
        apply_lsp_workspace_edit(
            &rename,
            &[directory.path().to_path_buf()],
            lsp_client::protocol::PositionEncoding::Utf16,
        )
        .expect("directory rename");
        assert!(!source.exists());
        assert_eq!(
            std::fs::read_to_string(target.join("nested").join("file.txt")).expect("renamed file"),
            "content"
        );
        let delete = serde_json::json!({
            "documentChanges": [{
                "kind": "delete", "uri": uri(&target), "options": {"recursive": true}
            }]
        });
        apply_lsp_workspace_edit(
            &delete,
            &[directory.path().to_path_buf()],
            lsp_client::protocol::PositionEncoding::Utf16,
        )
        .expect("directory delete");
        assert!(!target.exists());
    }

    #[test]
    fn server_workspace_edit_create_does_not_overwrite_directory() {
        let directory = tempfile::tempdir().expect("workspace");
        let target = directory.path().join("existing");
        std::fs::create_dir(&target).expect("directory");
        let uri = lsp_client::protocol::DocumentUri::from_path(&target).0;
        let edit = serde_json::json!({
            "documentChanges": [{
                "kind": "create", "uri": uri, "options": {"overwrite": true}
            }]
        });
        let error = apply_lsp_workspace_edit(
            &edit,
            &[directory.path().to_path_buf()],
            lsp_client::protocol::PositionEncoding::Utf16,
        )
        .expect_err("directory must not be replaced by CreateFile");
        assert!(error.contains("cannot be overwritten"));
        assert!(target.is_dir());
    }

    #[test]
    fn server_workspace_edit_worker_rejects_resource_operation_outside_root() {
        let directory = tempfile::tempdir().expect("workspace");
        let outside = tempfile::tempdir().expect("outside");
        let edit = serde_json::json!({
            "documentChanges": [{
                "kind": "create",
                "uri": format!("file:///{}", outside.path().join("escape.txt").to_string_lossy().replace('\\', "/"))
            }]
        });
        let error = apply_lsp_workspace_edit(
            &edit,
            &[directory.path().to_path_buf()],
            lsp_client::protocol::PositionEncoding::Utf16,
        )
        .expect_err("outside operation must be rejected");
        assert!(error.contains("untrusted path"));
        assert!(!outside.path().join("escape.txt").exists());
    }

    #[test]
    fn server_workspace_edit_resource_failure_rolls_back_prior_changes() {
        let directory = tempfile::tempdir().expect("workspace");
        let created = directory.path().join("created.txt");
        let missing = directory.path().join("missing.txt");
        let edit = serde_json::json!({
            "documentChanges": [
                {"kind": "create", "uri": format!("file:///{}", created.to_string_lossy().replace('\\', "/"))},
                {"kind": "delete", "uri": format!("file:///{}", missing.to_string_lossy().replace('\\', "/"))}
            ]
        });
        assert!(
            apply_lsp_workspace_edit(
                &edit,
                &[directory.path().to_path_buf()],
                lsp_client::protocol::PositionEncoding::Utf16,
            )
            .is_err()
        );
        assert!(!created.exists());
    }
}
