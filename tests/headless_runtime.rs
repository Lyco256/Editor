use std::convert::Infallible;
use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
};

use editor::app::{
    action::Action,
    effect::Effect,
    event::Event,
    runtime::{AppRuntime, EffectDispatcher, QueueActionSource, RecordingDispatcher},
};
use editor_types::TerminalCapabilities;
use terminal_backend::{Framebuffer, TerminalAdapter};

fn fake_lsp_server_bin() -> PathBuf {
    if let Ok(path) = std::env::var("CARGO_BIN_EXE_lsp_client_fake_server") {
        return path.into();
    }
    let direct =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/debug/lsp_client_fake_server.exe");
    if direct.is_file() {
        return direct;
    }
    let deps = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/debug/deps");
    std::fs::read_dir(deps)
        .expect("workspace test dependencies should be readable")
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .find(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| {
                    name.starts_with("lsp_client_fake_server-")
                        && std::path::Path::new(name)
                            .extension()
                            .is_some_and(|extension| extension.eq_ignore_ascii_case("exe"))
                })
        })
        .expect("fake LSP server binary should be built for workspace acceptance tests")
}

#[derive(Debug, Default)]
struct FakeTerminal {
    entered: bool,
    restored: bool,
    frames: usize,
}

#[derive(Debug, Default)]
struct SaveDispatcher {
    events: Vec<Event>,
}

#[derive(Debug, Default)]
struct SyntaxDispatcher {
    events: Vec<Event>,
}

impl EffectDispatcher for SyntaxDispatcher {
    fn dispatch(&mut self, effect: Effect) {
        if let Effect::RefreshSyntax {
            request,
            document,
            version,
            path,
            text,
            large_file,
        } = effect
        {
            let mut engine = syntax_engine::SyntaxEngine::default();
            let update = engine.open_document(
                syntax_engine::OpenDocument {
                    descriptor: editor_core::DocumentDescriptor {
                        id: document,
                        version,
                        large_file_mode: large_file,
                    },
                    path: path.as_deref(),
                    language_override: None,
                    text: &text,
                },
                None,
            );
            self.events.push(Event::SyntaxUpdated { request, update });
        }
    }

    fn poll_events(&mut self) -> Vec<Event> {
        std::mem::take(&mut self.events)
    }
}

#[derive(Debug, Default)]
struct CapturingTerminal {
    entered: bool,
    restored: bool,
    shared_frame: Arc<Mutex<Option<Framebuffer>>>,
}

impl TerminalAdapter for CapturingTerminal {
    type Error = Infallible;

    fn capabilities(&self) -> TerminalCapabilities {
        TerminalCapabilities::default()
    }

    fn enter(&mut self) -> Result<(), Self::Error> {
        self.entered = true;
        Ok(())
    }

    fn render(&mut self, frame: &Framebuffer) -> Result<(), Self::Error> {
        if let Ok(mut captured) = self.shared_frame.lock() {
            *captured = Some(frame.clone());
        }
        Ok(())
    }

    fn restore(&mut self) -> Result<(), Self::Error> {
        self.restored = true;
        Ok(())
    }
}

impl EffectDispatcher for SaveDispatcher {
    fn dispatch(&mut self, effect: Effect) {
        let save_as = matches!(&effect, Effect::SaveDocumentAs { .. });
        if let Effect::SaveDocument {
            path,
            text,
            encoding,
            with_bom,
            line_endings,
        }
        | Effect::SaveDocumentAs {
            path,
            text,
            encoding,
            with_bom,
            line_endings,
        } = effect
        {
            workspace_core::save_text_document(
                &path,
                &text,
                &workspace_core::DocumentSaveOptions {
                    encoding,
                    with_bom,
                    line_endings,
                    ..workspace_core::DocumentSaveOptions::default()
                },
            )
            .expect("save effect should write atomically");
            self.events.push(if save_as {
                Event::DocumentSavedAs { path }
            } else {
                Event::DocumentSaved { path }
            });
        } else if let Effect::RefreshExplorer { request, roots } = effect {
            let mut entries = Vec::new();
            for root in roots {
                let canonical = workspace_core::canonicalize_path(&root).expect("root");
                let children =
                    workspace_core::ExplorerTree::new(vec![canonical.clone()], Vec::new())
                        .children(canonical.as_path())
                        .expect("children");
                entries.extend(children.into_iter().map(|entry| {
                    editor::app::event::ExplorerEntryData {
                        path: entry.path,
                        depth: u8::try_from(entry.depth).unwrap_or(u8::MAX),
                    }
                }));
            }
            self.events
                .push(Event::ExplorerUpdated { request, entries });
        }
    }

    fn poll_events(&mut self) -> Vec<Event> {
        std::mem::take(&mut self.events)
    }
}

impl TerminalAdapter for FakeTerminal {
    type Error = Infallible;

    fn capabilities(&self) -> TerminalCapabilities {
        TerminalCapabilities::default()
    }

    fn enter(&mut self) -> Result<(), Self::Error> {
        self.entered = true;
        Ok(())
    }

    fn render(&mut self, _frame: &Framebuffer) -> Result<(), Self::Error> {
        assert!(self.entered);
        assert!(!self.restored);
        self.frames += 1;
        Ok(())
    }

    fn restore(&mut self) -> Result<(), Self::Error> {
        assert!(self.entered);
        self.restored = true;
        Ok(())
    }
}

#[test]
fn starts_renders_and_quits_cleanly_with_fake_adapters() {
    let runtime = AppRuntime::new(
        FakeTerminal::default(),
        QueueActionSource::new([Action::Quit]),
        RecordingDispatcher::default(),
        (80, 24),
    );
    let final_state = runtime.run().expect("headless runtime should be clean");
    assert!(!final_state.running);
    assert!(final_state.frame_number >= 1);
}

#[test]
fn startup_file_is_loaded_without_launching_external_effects() {
    let directory = tempfile::tempdir().expect("temporary directory is available");
    let path = directory.path().join("hello.rs");
    std::fs::write(&path, "fn main() {}\n").expect("fixture is writable");

    let mut state = editor::app::state::AppState::default();
    state.open_startup_path(&path);
    state.workspace_trusted = true;
    assert_eq!(state.active_path.as_deref(), Some(path.as_path()));
    assert_eq!(state.active_text, "fn main() {}\n");
    assert!(state.output.is_empty());
}

#[test]
fn root_headless_syntax_effect_renders_a_highlighted_frame() {
    let directory = tempfile::tempdir().expect("workspace");
    let path = directory.path().join("main.rs");
    std::fs::write(&path, "fn main() {}\n").expect("fixture");
    let mut state = editor::app::state::AppState::default();
    state.open_startup_path(&path);
    let input = Action::Input(editor_types::InputEvent::Key(editor_types::KeyEvent {
        code: editor_types::KeyCode::Character(' '),
        modifiers: editor_types::Modifiers::default(),
        repeat: false,
    }));
    let captured = Arc::new(Mutex::new(None));
    let runtime = AppRuntime::with_state(
        CapturingTerminal {
            shared_frame: Arc::clone(&captured),
            ..CapturingTerminal::default()
        },
        QueueActionSource::new([input, Action::Quit]),
        SyntaxDispatcher::default(),
        (80, 24),
        state,
    );
    let final_state = runtime.run().expect("syntax runtime should finish");
    assert!(final_state.syntax_snapshot().ticket.is_some());
    assert!(
        final_state
            .syntax_snapshot()
            .highlights
            .iter()
            .any(|span| span.role == editor_types::StyleRole::SyntaxKeyword)
    );
    let frame = captured
        .lock()
        .expect("captured frame lock")
        .clone()
        .expect("headless runtime should render a frame");
    assert!((0..80).any(|column| {
        (0..24).any(|row| {
            frame
                .get(column, row)
                .is_ok_and(|cell| cell.foreground == editor_types::StyleRole::SyntaxKeyword)
        })
    }));
}

#[test]
fn ten_mib_edit_transaction_stays_within_interactive_budget() {
    let text = "x".repeat(10 * 1024 * 1024);
    let mut buffer = editor_core::TextBuffer::new(&text);
    let transaction = editor_core::Transaction::new(vec![editor_core::Edit::insert(
        editor_core::CharacterOffset(0),
        "y",
    )])
    .expect("10 MiB edit transaction should be valid");
    let started = std::time::Instant::now();
    buffer
        .apply_transaction(transaction)
        .expect("10 MiB edit should be valid");
    let elapsed = started.elapsed();
    println!("10 MiB edit transaction: {:.2?}", elapsed);
    assert!(elapsed < std::time::Duration::from_millis(250));
}

#[test]
fn headless_action_loop_preserves_editing_state_between_frames() {
    use editor_types::{InputEvent, KeyCode, KeyEvent, Modifiers};

    let action = Action::Input(InputEvent::Key(KeyEvent {
        code: KeyCode::Character('x'),
        modifiers: Modifiers::default(),
        repeat: false,
    }));
    let runtime = AppRuntime::new(
        FakeTerminal::default(),
        QueueActionSource::new([
            action,
            Action::Invoke(editor_types::CommandId::new("editor.undo")),
            Action::Quit,
        ]),
        RecordingDispatcher::default(),
        (80, 24),
    );
    let final_state = runtime
        .run()
        .expect("headless editing loop should be clean");
    assert_eq!(final_state.active_text, "");
    assert!(!final_state.active_dirty);
    assert!(!final_state.running);
}

#[test]
fn edit_save_and_reopen_round_trip_through_root_effects() {
    use editor_types::{InputEvent, KeyCode, KeyEvent, Modifiers};

    let directory = tempfile::tempdir().expect("temporary directory is available");
    let path = directory.path().join("roundtrip.txt");
    std::fs::write(&path, "before").expect("fixture is writable");
    let mut state = editor::app::state::AppState::default();
    state.open_startup_path(&path);
    let runtime = AppRuntime::with_state(
        FakeTerminal::default(),
        QueueActionSource::new([
            Action::Input(InputEvent::Key(KeyEvent {
                code: KeyCode::Character('!'),
                modifiers: Modifiers::default(),
                repeat: false,
            })),
            Action::Invoke(editor_types::CommandId::new("editor.save")),
            Action::Quit,
        ]),
        SaveDispatcher::default(),
        (80, 24),
        state,
    );
    let final_state = runtime.run().expect("save runtime should be clean");
    assert!(!final_state.active_dirty);
    assert_eq!(final_state.active_text, "!before");
    let mut reopened = editor::app::state::AppState::default();
    reopened.open_startup_path(&path);
    assert_eq!(reopened.active_text, "!before");
}

#[test]
fn session_snapshot_restores_unsaved_text_after_restart() {
    use editor_types::{InputEvent, KeyCode, KeyEvent, Modifiers};

    let directory = tempfile::tempdir().expect("temporary directory is available");
    let path = directory.path().join("session.txt");
    std::fs::write(&path, "disk").expect("fixture is writable");
    let mut state = editor::app::state::AppState::default();
    state.open_startup_path(&path);
    let _ = state.apply_action(Action::Input(InputEvent::Key(KeyEvent {
        code: KeyCode::Character('!'),
        modifiers: Modifiers::default(),
        repeat: false,
    })));
    let session = state.session_state();
    let mut restored = editor::app::state::AppState::default();
    restored.restore_session(&session);
    assert_eq!(restored.active_text, "!disk");
    assert!(restored.active_dirty);
    assert_eq!(restored.active_path.as_deref(), Some(path.as_path()));
}

#[test]
fn adding_workspace_roots_updates_explorer_projection() {
    let first = tempfile::tempdir().expect("first root");
    let second = tempfile::tempdir().expect("second root");
    std::fs::write(first.path().join("one.txt"), "one").expect("first file");
    std::fs::write(second.path().join("two.txt"), "two").expect("second file");
    let runtime = AppRuntime::new(
        FakeTerminal::default(),
        QueueActionSource::new([
            Action::AddWorkspaceRoot(first.path().to_path_buf()),
            Action::AddWorkspaceRoot(second.path().to_path_buf()),
            Action::Quit,
        ]),
        SaveDispatcher::default(),
        (80, 24),
    );
    let state = runtime.run().expect("workspace runtime");
    assert_eq!(state.workspace_roots.len(), 2);
    assert!(
        state
            .explorer_entries
            .iter()
            .any(|entry| entry.label == "one.txt")
    );
    assert!(
        state
            .explorer_entries
            .iter()
            .any(|entry| entry.label == "two.txt")
    );
}

#[test]
fn git_status_projection_routes_dashboard_mutations_through_root_effects() {
    let directory = tempfile::tempdir().expect("workspace");
    let root = directory.path().to_path_buf();
    let mut state = editor::app::state::AppState::default();
    state.workspace_roots.push(root.clone());
    let _ = state.apply_action(Action::SetWorkspaceTrust(true));
    state.apply_event(Event::GitStatusUpdated {
        request: editor_types::RequestId(1),
        root: root.clone(),
        summary: editor_types::GitStatusSummary::default(),
        entries: vec![vcs_git::GitStatusEntry {
            status: " M".to_owned(),
            path: std::path::PathBuf::from("src/main.rs"),
            previous_path: None,
            staged: false,
            unstaged: true,
            untracked: false,
            conflicted: false,
        }],
        branch_state: Some("main".to_owned()),
        head: Some("deadbeef".to_owned()),
        conflicts: Vec::new(),
        diff_files: Vec::new(),
        branches: Vec::new(),
        stashes: Vec::new(),
        history: Vec::new(),
    });

    let transition = state.apply_git_action(app_ui::git::GitAction::StageFile(
        std::path::PathBuf::from("src/main.rs"),
    ));
    assert!(matches!(
        transition.effects.first(),
        Some(Effect::ExternalProcess { spec, .. })
            if spec.arguments.iter().any(|argument| argument == "add")
                && spec.arguments.iter().any(|argument| argument == "--")
    ));
    assert_eq!(
        state.git_status,
        Some(editor_types::GitStatusSummary::default())
    );
}

#[tokio::test(flavor = "current_thread")]
#[allow(clippy::too_many_lines)]
async fn root_lsp_effect_and_response_lifecycle_updates_language_views() {
    let directory = tempfile::tempdir().expect("workspace");
    let path = directory.path().join("main.rs");
    std::fs::write(&path, "complete").expect("fixture");
    let mut state = editor::app::state::AppState::default();
    state.open_startup_path(&path);
    state.workspace_trusted = true;
    let version = 0;
    let command = {
        let mut command = lsp_client::CommandSpec::new(fake_lsp_server_bin());
        command.args = vec!["lsp".to_owned(), "normal".to_owned(), "utf8".to_owned()];
        command
    };
    let client = lsp_client::LspClient::spawn(command)
        .await
        .expect("fake LSP should spawn");
    let initialized = client
        .initialize(lsp_client::InitializeParams {
            position_encodings: vec![lsp_client::PositionEncoding::Utf8],
            ..lsp_client::InitializeParams::default()
        })
        .await
        .expect("fake LSP should initialize");
    assert_eq!(
        initialized.position_encoding,
        lsp_client::PositionEncoding::Utf8
    );
    client
        .initialized()
        .await
        .expect("initialized notification");
    let mut client_events = client.subscribe();
    client
        .did_open(lsp_client::DidOpenTextDocumentParams {
            text_document: lsp_client::TextDocumentItem {
                uri: lsp_client::DocumentUri("file:///workspace/main.rs".to_owned()),
                language_id: "rust".to_owned(),
                version: 1,
                text: "complete".to_owned(),
            },
        })
        .await
        .expect("didOpen");

    if let Ok(Ok(lsp_client::ClientEvent::Diagnostics(params))) =
        tokio::time::timeout(std::time::Duration::from_millis(250), client_events.recv()).await
    {
        state.apply_event(Event::LanguageDiagnostics {
            request: editor_types::RequestId(21),
            params,
        });
        assert!(state.language_model().diagnostics.current().is_some());
    }

    let root_effect = Effect::LspRequest {
        request: editor_types::RequestId(22),
        version,
        spec: editor::app::effect::ProcessSpec {
            executable: fake_lsp_server_bin().display().to_string(),
            arguments: vec!["lsp".to_owned(), "normal".to_owned(), "utf8".to_owned()],
        },
        method: "textDocument/completion".to_owned(),
        params: serde_json::json!({}),
    };
    let transition = state.apply_action(Action::RequestEffect(root_effect));
    assert!(matches!(
        transition.effects.first(),
        Some(Effect::LspRequest { .. })
    ));

    let completion = client
        .completion(serde_json::json!({
            "textDocument": {"uri": "file:///workspace/main.rs"},
            "position": {"line": 0, "character": 0}
        }))
        .await
        .expect("completion response");
    state.apply_event(Event::LspResponse {
        request: editor_types::RequestId(23),
        version,
        method: "textDocument/completion".to_owned(),
        result: completion,
    });
    assert_eq!(
        state
            .language_model()
            .completion
            .current()
            .map(|view| view.list.rows.len()),
        Some(1)
    );

    let semantic = client
        .semantic_tokens(serde_json::json!({
            "textDocument": {"uri": "file:///workspace/main.rs"}
        }))
        .await
        .expect("semantic token response");
    state.apply_event(Event::LspResponse {
        request: editor_types::RequestId(24),
        version,
        method: "textDocument/semanticTokens/full".to_owned(),
        result: semantic,
    });
    assert_eq!(
        state
            .language_model()
            .semantic
            .current()
            .map(|spans| spans.spans.len()),
        Some(1)
    );

    let request_params = serde_json::json!({
        "textDocument": {"uri": "file:///workspace/main.rs"},
        "position": {"line": 0, "character": 0}
    });
    for (request_id, method, result) in [
        (
            25,
            "textDocument/hover",
            client.hover(request_params.clone()).await.expect("hover"),
        ),
        (
            26,
            "textDocument/signatureHelp",
            client
                .signature_help(request_params.clone())
                .await
                .expect("signature help"),
        ),
        (
            27,
            "textDocument/definition",
            client
                .definition(request_params.clone())
                .await
                .expect("definition"),
        ),
        (
            28,
            "textDocument/references",
            client
                .references(request_params.clone())
                .await
                .expect("references"),
        ),
        (
            29,
            "textDocument/rename",
            client.rename(request_params.clone()).await.expect("rename"),
        ),
        (
            30,
            "textDocument/codeAction",
            client
                .code_action(request_params.clone())
                .await
                .expect("code action"),
        ),
        (
            31,
            "textDocument/inlayHint",
            client
                .inlay_hints(request_params.clone())
                .await
                .expect("inlay hints"),
        ),
        (
            32,
            "textDocument/documentSymbol",
            client
                .document_symbols(request_params.clone())
                .await
                .expect("document symbols"),
        ),
    ] {
        state.apply_event(Event::LspResponse {
            request: editor_types::RequestId(request_id),
            version,
            method: method.to_owned(),
            result,
        });
    }
    assert!(state.language_model().hover.current().is_some());
    assert!(state.language_model().signature.current().is_some());
    assert!(state.language_model().go_to.current().is_some());
    assert!(state.language_model().references.current().is_some());
    assert!(state.language_model().rename.current().is_some());
    assert!(state.language_model().code_actions.current().is_some());
    assert!(state.language_model().inlay_hints.current().is_some());
    assert!(state.language_model().symbols.current().is_some());

    let formatting = client
        .formatting(request_params)
        .await
        .expect("formatting response");
    state.apply_event(Event::LspResponse {
        request: editor_types::RequestId(33),
        version,
        method: "textDocument/formatting".to_owned(),
        result: formatting,
    });
    assert!(state.language_model().formatting.current().is_some());
    client.shutdown().await.expect("fake LSP should shut down");
}

#[tokio::test(flavor = "current_thread")]
#[allow(clippy::too_many_lines)]
async fn root_fake_server_cancellation_and_crash_restart_are_observable() {
    let slow_command = {
        let mut command = lsp_client::CommandSpec::new(fake_lsp_server_bin());
        command.args = vec![
            "lsp".to_owned(),
            "slow_completion".to_owned(),
            "utf16".to_owned(),
        ];
        command
    };
    let slow = lsp_client::LspClient::spawn(slow_command)
        .await
        .expect("slow fake server should spawn");
    slow.initialize(lsp_client::InitializeParams::default())
        .await
        .expect("slow fake server should initialize");
    slow.initialized().await.expect("initialized notification");
    let ticket = slow
        .start_request::<serde_json::Value>(
            "textDocument/completion",
            serde_json::json!({"position": {"line": 0, "character": 0}}),
        )
        .await
        .expect("completion ticket");
    slow.cancel_request(ticket.id())
        .await
        .expect("cancel notification");
    assert!(matches!(
        ticket.wait().await,
        Err(lsp_client::ClientError::Cancelled)
    ));
    slow.shutdown()
        .await
        .expect("slow fake LSP should shut down");

    let crash_command = {
        let mut command = lsp_client::CommandSpec::new(fake_lsp_server_bin());
        command.args = vec![
            "lsp".to_owned(),
            "crash_after_open".to_owned(),
            "utf16".to_owned(),
        ];
        command
    };
    let crash = lsp_client::LspClient::spawn(crash_command)
        .await
        .expect("crash fake server should spawn");
    crash
        .initialize(lsp_client::InitializeParams::default())
        .await
        .expect("crash fake server should initialize");
    crash.initialized().await.expect("initialized notification");
    let mut events = crash.subscribe();
    crash
        .did_open(lsp_client::DidOpenTextDocumentParams {
            text_document: lsp_client::TextDocumentItem {
                uri: lsp_client::DocumentUri("file:///workspace/crash.rs".to_owned()),
                language_id: "rust".to_owned(),
                version: 1,
                text: "crash".to_owned(),
            },
        })
        .await
        .expect("didOpen crash document");
    let terminal_event = tokio::time::timeout(std::time::Duration::from_secs(2), async {
        loop {
            let event = events.recv().await.expect("crash event stream");
            if matches!(
                event,
                lsp_client::ClientEvent::Crashed { .. } | lsp_client::ClientEvent::Exited { .. }
            ) {
                break event;
            }
        }
    })
    .await
    .expect("crash event should arrive");
    assert!(matches!(
        terminal_event,
        lsp_client::ClientEvent::Crashed { .. } | lsp_client::ClientEvent::Exited { .. }
    ));
    drop(crash);
    let restart_command = {
        let mut command = lsp_client::CommandSpec::new(fake_lsp_server_bin());
        command.args = vec!["lsp".to_owned(), "normal".to_owned(), "utf16".to_owned()];
        command
    };
    let restarted = lsp_client::LspClient::spawn(restart_command)
        .await
        .expect("replacement LSP should spawn after crash");
    restarted
        .initialize(lsp_client::InitializeParams::default())
        .await
        .expect("replacement LSP should initialize");
    restarted
        .initialized()
        .await
        .expect("replacement initialized notification");
    restarted
        .did_open(lsp_client::DidOpenTextDocumentParams {
            text_document: lsp_client::TextDocumentItem {
                uri: lsp_client::DocumentUri("file:///workspace/crash.rs".to_owned()),
                language_id: "rust".to_owned(),
                version: 2,
                text: "crash".to_owned(),
            },
        })
        .await
        .expect("didOpen after restart");
    let result = restarted
        .completion(serde_json::json!({
            "position": {"line": 0, "character": 0}
        }))
        .await
        .expect("replacement LSP should answer requests");
    assert!(result.get("items").is_some());
    restarted
        .shutdown()
        .await
        .expect("replacement LSP should shut down");
}

#[test]
fn multiple_tabs_and_active_tab_survive_session_restore() {
    use editor_types::{InputEvent, KeyCode, KeyEvent, Modifiers};
    let directory = tempfile::tempdir().expect("workspace");
    let first = directory.path().join("first.txt");
    let second = directory.path().join("second.txt");
    std::fs::write(&first, "one").expect("first");
    std::fs::write(&second, "two").expect("second");
    let mut state = editor::app::state::AppState::default();
    state.open_startup_path(&first);
    state.open_tab(&second).expect("open second tab");
    let _ = state.apply_action(Action::Input(InputEvent::Key(KeyEvent {
        code: KeyCode::Character('!'),
        modifiers: Modifiers::default(),
        repeat: false,
    })));
    let session = state.session_state();
    let mut restored = editor::app::state::AppState::default();
    restored.restore_session(&session);
    assert_eq!(restored.tab_count(), 2);
    assert_eq!(restored.active_tab_index(), 1);
    assert_eq!(restored.active_text, "!two");
}

#[test]
fn split_layout_and_multiple_selections_survive_session_restore() {
    use editor_core::{CharacterOffset, Selection, SelectionSet};

    let directory = tempfile::tempdir().expect("workspace");
    let first = directory.path().join("first.txt");
    let second = directory.path().join("second.txt");
    std::fs::write(&first, "one two").expect("first");
    std::fs::write(&second, "three four").expect("second");
    let mut state = editor::app::state::AppState::default();
    state.open_startup_path(&first);
    state.open_tab(&second).expect("open second tab");
    state
        .set_active_selections(
            SelectionSet::new(
                vec![
                    Selection::new(CharacterOffset(0), CharacterOffset(5)),
                    Selection::new(CharacterOffset(6), CharacterOffset(10)),
                ],
                1,
            )
            .expect("valid selections"),
        )
        .expect("set selections");
    let _ = state.apply_action(Action::SplitPane {
        axis: app_ui::shell::SplitAxis::Vertical,
        ratio_percent: 60,
    });

    let session = state.session_state();
    assert!(matches!(
        session.split_layout,
        config_core::SplitLayout::Split { .. }
    ));
    assert_eq!(session.editors[1].selections.len(), 2);

    let mut restored = editor::app::state::AppState::default();
    restored.restore_session(&session);
    let restored_session = restored.session_state();
    assert!(matches!(
        restored_session.split_layout,
        config_core::SplitLayout::Split {
            axis: config_core::SplitAxis::Vertical,
            ..
        }
    ));
    assert_eq!(restored_session.editors[1].selections.len(), 2);
}

#[test]
fn missing_original_file_keeps_recovered_unsaved_text() {
    let missing = std::env::temp_dir().join("editor-recovery-missing-file.txt");
    let session = config_core::SessionState {
        editors: vec![config_core::EditorSession {
            editor_id: "tab-0".to_owned(),
            original_path: Some(missing),
            unsaved_text: Some("recovered".to_owned()),
            dirty: true,
            ..config_core::EditorSession::default()
        }],
        tab_order: vec!["tab-0".to_owned()],
        active_editor: Some("tab-0".to_owned()),
        ..config_core::SessionState::default()
    };
    let mut restored = editor::app::state::AppState::default();
    restored.restore_session(&session);
    assert_eq!(restored.active_text, "recovered");
    assert!(restored.active_dirty);
}

#[test]
fn encoding_and_line_endings_survive_root_save_and_session_round_trip() {
    let directory = tempfile::tempdir().expect("workspace");
    let path = directory.path().join("unicode.txt");
    let bytes = workspace_core::encode_text_document(
        "日本語\r\n",
        &workspace_core::EncodingKind::Utf16Le,
        true,
        workspace_core::DecodePolicy::Strict,
    )
    .expect("encode fixture");
    std::fs::write(&path, bytes).expect("write fixture");

    let mut state = editor::app::state::AppState::default();
    state.open_startup_path(&path);
    let session = state.session_state();
    assert_eq!(
        session.editors[0].original_encoding,
        Some(config_core::EncodingKind::Utf16Le)
    );
    assert!(session.editors[0].original_bom);
    assert_eq!(
        session.editors[0].original_line_ending,
        Some(config_core::LineEndings::Crlf)
    );

    let mut restored = editor::app::state::AppState::default();
    restored.restore_session(&session);
    let transition = restored.apply_action(Action::Input(editor_types::InputEvent::Key(
        editor_types::KeyEvent {
            code: editor_types::KeyCode::Character('!'),
            modifiers: editor_types::Modifiers::default(),
            repeat: false,
        },
    )));
    let save = transition
        .effects
        .into_iter()
        .find(|effect| matches!(effect, Effect::SaveDocument { .. }));
    assert!(save.is_none(), "typing alone does not save automatically");
    let save = restored.apply_action(Action::Invoke(editor_types::CommandId::new("editor.save")));
    let Effect::SaveDocument {
        encoding,
        with_bom,
        line_endings,
        ..
    } = save.effects.into_iter().next().expect("save effect")
    else {
        panic!("expected save effect");
    };
    assert_eq!(encoding, workspace_core::EncodingKind::Utf16Le);
    assert!(with_bom);
    assert_eq!(line_endings, workspace_core::LineEndings::Crlf);
}

#[test]
fn recovery_checkpoint_is_written_after_an_edit() {
    use editor_types::{InputEvent, KeyCode, KeyEvent, Modifiers};

    let app_data = tempfile::tempdir().expect("application data");
    let store = config_core::RecoveryStore::from_app_data_base(app_data.path(), "Editor")
        .expect("recovery store");
    let input = Action::Input(InputEvent::Key(KeyEvent {
        code: KeyCode::Character('x'),
        modifiers: Modifiers::default(),
        repeat: false,
    }));
    let runtime = AppRuntime::with_recovery_store(
        FakeTerminal::default(),
        QueueActionSource::new([input]),
        RecordingDispatcher::default(),
        (80, 24),
        editor::app::state::AppState::default(),
        store.clone(),
    );
    let final_state = runtime.run().expect("checkpoint runtime");
    assert_eq!(final_state.active_text, "x");
    let loaded = store.load_latest().expect("load checkpoint");
    assert_eq!(
        loaded.session.expect("session").editors[0]
            .unsaved_text
            .as_deref(),
        Some("x")
    );
}

#[test]
fn project_replacement_plan_runs_as_a_background_effect() {
    let directory = tempfile::tempdir().expect("workspace");
    let path = directory.path().join("replace.txt");
    std::fs::write(&path, "old old").expect("seed");
    let plan = workspace_core::ReplacementPlan {
        files: vec![workspace_core::search::FileReplacementPlan {
            path: path.clone(),
            edits: vec![
                workspace_core::ReplacementEdit {
                    range: 4..7,
                    replacement: "new".to_owned(),
                },
                workspace_core::ReplacementEdit {
                    range: 0..3,
                    replacement: "new".to_owned(),
                },
            ],
        }],
    };
    let mut state = editor::app::state::AppState::default();
    let transition = state.apply_action(Action::ApplyReplacementPlan(plan));
    let Effect::ApplyReplacementPlan { request, plan } = transition.effects[0].clone() else {
        panic!("replacement should dispatch a typed effect");
    };
    let report = workspace_core::apply_replacement_plan(&plan).expect("replace");
    state.apply_event(Event::ReplacementApplied { request, report });
    assert_eq!(std::fs::read_to_string(path).expect("read"), "new new");
}

#[test]
fn save_as_retargets_only_after_atomic_write_and_close_protects_dirty_tabs() {
    use editor_types::{InputEvent, KeyCode, KeyEvent, Modifiers};

    let directory = tempfile::tempdir().expect("workspace");
    let target = directory.path().join("saved.txt");
    let input = Action::Input(InputEvent::Key(KeyEvent {
        code: KeyCode::Character('x'),
        modifiers: Modifiers::default(),
        repeat: false,
    }));
    let mut state = editor::app::state::AppState::default();
    let _ = state.apply_action(input.clone());
    let runtime = AppRuntime::with_state(
        FakeTerminal::default(),
        QueueActionSource::new([Action::SaveAs(target.clone()), Action::Quit]),
        SaveDispatcher::default(),
        (80, 24),
        state,
    );
    let final_state = runtime.run().expect("save-as runtime");
    assert!(!final_state.active_dirty);
    assert_eq!(final_state.active_path.as_deref(), Some(target.as_path()));
    assert_eq!(std::fs::read_to_string(target).expect("saved file"), "x");

    let mut dirty = editor::app::state::AppState::default();
    let _ = dirty.apply_action(input);
    let _ = dirty.apply_action(Action::CloseTab(0));
    assert!(dirty.active_dirty);
    assert!(
        dirty
            .output
            .iter()
            .any(|message| message.operation == "close-tab")
    );
}
