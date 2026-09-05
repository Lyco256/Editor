use std::{env, path::PathBuf, time::Duration};

use editor_types::LogicalPosition;
use lsp_client::{
    ClientError, ClientEvent, CommandSpec, DidChangeTextDocumentParams,
    DidChangeWorkspaceFoldersParams, DidCloseTextDocumentParams, DidOpenTextDocumentParams,
    DidSaveTextDocumentParams, DocumentUri, FormatterRunner, FormatterSpec, InitializeParams,
    LspClient, Position, PositionEncoding, PositionMapper, ProcessCancelToken,
    TextDocumentContentChangeEvent, TextDocumentIdentifier, TextDocumentItem, TextSnapshot,
    VersionedTextDocumentIdentifier, WorkspaceFolder,
};

#[test]
fn document_uri_percent_encodes_paths() {
    let uri = DocumentUri::from_path(std::path::Path::new("C:/work space/日本語.rs"));
    assert_eq!(
        uri.0,
        "file:///C:/work%20space/%E6%97%A5%E6%9C%AC%E8%AA%9E.rs"
    );
}
use serde_json::json;
use tokio::time;

fn fake_server_bin() -> PathBuf {
    env::var("CARGO_BIN_EXE_lsp_client_fake_server")
        .expect("fake server binary path")
        .into()
}

fn lsp_command(scenario: &str, encoding: &str) -> CommandSpec {
    let mut command = CommandSpec::new(fake_server_bin());
    command.args = vec!["lsp".to_owned(), scenario.to_owned(), encoding.to_owned()];
    command
}

fn formatter_command(scenario: &str) -> CommandSpec {
    let mut command = CommandSpec::new(fake_server_bin());
    command.args = vec!["formatter".to_owned(), scenario.to_owned()];
    command
}

fn initialize_params(encoding: PositionEncoding) -> InitializeParams {
    InitializeParams {
        position_encodings: vec![encoding],
        ..InitializeParams::default()
    }
}

fn text_snapshot(text: &str) -> TextSnapshot {
    TextSnapshot::new(text)
}

async fn initialize_client(scenario: &str, encoding: &str) -> LspClient {
    let client = LspClient::spawn(lsp_command(scenario, encoding))
        .await
        .expect("spawn client");
    let response = client
        .initialize(initialize_params(if encoding == "utf8" {
            PositionEncoding::Utf8
        } else {
            PositionEncoding::Utf16
        }))
        .await
        .expect("initialize");
    assert_eq!(
        response.position_encoding,
        if encoding == "utf8" {
            PositionEncoding::Utf8
        } else {
            PositionEncoding::Utf16
        }
    );
    client.initialized().await.expect("initialized");
    client
}

#[tokio::test(flavor = "current_thread")]
async fn position_mapper_round_trips_unicode_text() {
    let snapshot = text_snapshot("a\n界🙂\ne\u{301}x\n");
    let utf16 = PositionMapper::new(&snapshot, PositionEncoding::Utf16);
    let utf8 = PositionMapper::new(&snapshot, PositionEncoding::Utf8);

    let logical = LogicalPosition {
        line: 1,
        character: 2,
    };
    assert_eq!(
        utf16
            .from_lsp(utf16.to_lsp(logical).expect("utf16 encode"))
            .expect("utf16 round trip"),
        logical
    );
    assert_eq!(
        utf8.from_lsp(utf8.to_lsp(logical).expect("utf8 encode"))
            .expect("utf8 round trip"),
        logical
    );

    let error = utf8.from_lsp(Position {
        line: 1,
        character: 1,
    });
    assert!(error.is_err(), "utf8 positions must not split a scalar");
}

#[tokio::test(flavor = "current_thread")]
#[allow(clippy::too_many_lines)]
async fn lsp_lifecycle_and_requests_round_trip() {
    let client = initialize_client("normal", "utf8").await;
    let mut events = client.subscribe();

    client
        .did_open(DidOpenTextDocumentParams {
            text_document: TextDocumentItem {
                uri: DocumentUri("file:///workspace/main.rs".to_owned()),
                language_id: "rust".to_owned(),
                version: 1,
                text: "diagnostic".to_owned(),
            },
        })
        .await
        .expect("didOpen");

    let diagnostics = time::timeout(Duration::from_secs(2), async {
        loop {
            if let ClientEvent::Diagnostics(diagnostics) = events.recv().await.expect("event") {
                break diagnostics;
            }
        }
    })
    .await
    .expect("diagnostics arrive");
    assert_eq!(diagnostics.diagnostics.len(), 1);

    client
        .did_change(DidChangeTextDocumentParams {
            text_document: VersionedTextDocumentIdentifier {
                uri: DocumentUri("file:///workspace/main.rs".to_owned()),
                version: 2,
            },
            content_changes: vec![TextDocumentContentChangeEvent {
                range: Some(lsp_client::Range {
                    start: Position {
                        line: 0,
                        character: 0,
                    },
                    end: Position {
                        line: 0,
                        character: 10,
                    },
                }),
                range_length: None,
                text: "complete".to_owned(),
            }],
        })
        .await
        .expect("didChange");

    client
        .did_save(DidSaveTextDocumentParams {
            text_document: TextDocumentIdentifier {
                uri: DocumentUri("file:///workspace/main.rs".to_owned()),
            },
            text: Some("complete".to_owned()),
        })
        .await
        .expect("didSave");
    client
        .did_change_workspace_folders(DidChangeWorkspaceFoldersParams {
            event: lsp_client::protocol::WorkspaceFoldersChangeEvent {
                added: vec![WorkspaceFolder {
                    uri: DocumentUri("file:///workspace".to_owned()),
                    name: "workspace".to_owned(),
                }],
                removed: Vec::new(),
            },
        })
        .await
        .expect("workspace folders");

    let completion = client
        .completion(json!({
            "textDocument": {"uri": "file:///workspace/main.rs"},
            "position": {"line": 0, "character": 0}
        }))
        .await
        .expect("completion");
    assert_eq!(completion["items"][0]["label"], "completed");

    let resolved = client
        .completion_resolve(json!({"label": "completed"}))
        .await
        .expect("completion resolve");
    assert_eq!(resolved["detail"], "resolved item");

    assert_eq!(
        client
            .hover(json!({"textDocument": {"uri": "file:///workspace/main.rs"}, "position": {"line": 0, "character": 0}}))
            .await
            .expect("hover")["contents"],
        "hover"
    );
    assert!(client
        .signature_help(json!({"textDocument": {"uri": "file:///workspace/main.rs"}, "position": {"line": 0, "character": 0}}))
        .await
        .expect("signature help")
        .is_object());
    assert!(client
        .definition(json!({"textDocument": {"uri": "file:///workspace/main.rs"}, "position": {"line": 0, "character": 0}}))
        .await
        .expect("definition")
        .is_array());
    assert!(client
        .declaration(json!({"textDocument": {"uri": "file:///workspace/main.rs"}, "position": {"line": 0, "character": 0}}))
        .await
        .expect("declaration")
        .is_array());
    assert!(client
        .implementation(json!({"textDocument": {"uri": "file:///workspace/main.rs"}, "position": {"line": 0, "character": 0}}))
        .await
        .expect("implementation")
        .is_array());
    assert!(client
        .references(json!({"textDocument": {"uri": "file:///workspace/main.rs"}, "position": {"line": 0, "character": 0}}))
        .await
        .expect("references")
        .is_array());
    assert!(
        client
            .document_symbols(json!({"textDocument": {"uri": "file:///workspace/main.rs"}}))
            .await
            .expect("document symbols")
            .is_array()
    );
    assert!(
        client
            .workspace_symbols(json!({"query": "symbol"}))
            .await
            .expect("workspace symbols")
            .is_array()
    );
    assert!(client
        .prepare_rename(json!({"textDocument": {"uri": "file:///workspace/main.rs"}, "position": {"line": 0, "character": 0}}))
        .await
        .expect("prepare rename")
        .is_object());
    assert!(client
        .rename(json!({"textDocument": {"uri": "file:///workspace/main.rs"}, "position": {"line": 0, "character": 0}, "newName": "renamed"}))
        .await
        .expect("rename")
        .is_object());
    assert!(client
        .code_action(json!({"textDocument": {"uri": "file:///workspace/main.rs"}, "range": {"start": {"line": 0, "character": 0}, "end": {"line": 0, "character": 5}}, "context": {"diagnostics": []}}))
        .await
        .expect("code action")
        .is_array());
    assert!(
        client
            .formatting(
                json!({"textDocument": {"uri": "file:///workspace/main.rs"}, "options": {}})
            )
            .await
            .expect("formatting")
            .is_array()
    );
    assert!(client
        .range_formatting(json!({"textDocument": {"uri": "file:///workspace/main.rs"}, "range": {"start": {"line": 0, "character": 0}, "end": {"line": 0, "character": 5}}, "options": {}}))
        .await
        .expect("range formatting")
        .is_array());
    assert!(
        client
            .semantic_tokens(json!({"textDocument": {"uri": "file:///workspace/main.rs"}}))
            .await
            .expect("semantic tokens")
            .is_object()
    );
    assert!(
        client
            .inlay_hints(json!({"textDocument": {"uri": "file:///workspace/main.rs"}}))
            .await
            .expect("inlay hints")
            .is_array()
    );
    assert_eq!(
        client
            .apply_workspace_edit(json!({"edit": {}}))
            .await
            .expect("apply workspace edit")["applied"],
        true
    );

    client
        .did_close(DidCloseTextDocumentParams {
            text_document: TextDocumentIdentifier {
                uri: DocumentUri("file:///workspace/main.rs".to_owned()),
            },
        })
        .await
        .expect("didClose");

    client.shutdown().await.expect("shutdown");
    let stopped = time::timeout(Duration::from_secs(2), async {
        loop {
            if matches!(client.status().await, lsp_client::ClientStatus::Stopped) {
                break;
            }
            time::sleep(Duration::from_millis(25)).await;
        }
    })
    .await;
    assert!(stopped.is_ok(), "client should reach a clean stopped state");
}

#[tokio::test(flavor = "current_thread")]
async fn server_workspace_edit_request_can_be_acknowledged() {
    let client = LspClient::spawn(lsp_command("server_workspace_edit", "utf8"))
        .await
        .expect("spawn client");
    let mut events = client.subscribe();
    client
        .initialize(initialize_params(PositionEncoding::Utf8))
        .await
        .expect("initialize");
    client.initialized().await.expect("initialized");
    let (id, method, params) = time::timeout(Duration::from_secs(2), async {
        loop {
            if let ClientEvent::ServerRequest { id, method, params } =
                events.recv().await.expect("event")
            {
                break (id, method, params);
            }
        }
    })
    .await
    .expect("server request arrives");
    assert_eq!(method, "workspace/applyEdit");
    assert!(params.is_some());
    client
        .respond(id, Some(json!({"applied": true})), None)
        .await
        .expect("server request response");
    client.shutdown().await.expect("shutdown");
    let stopped = time::timeout(Duration::from_secs(2), async {
        loop {
            if matches!(client.status().await, lsp_client::ClientStatus::Stopped) {
                break;
            }
            time::sleep(Duration::from_millis(25)).await;
        }
    })
    .await;
    assert!(stopped.is_ok(), "fake server should observe the response");
}

#[tokio::test(flavor = "current_thread")]
async fn cancellation_suppresses_late_completion_response() {
    let client = initialize_client("slow_completion", "utf16").await;
    let ticket = client
        .start_request::<serde_json::Value>(
            "textDocument/completion",
            json!({
                "textDocument": {"uri": "file:///workspace/main.rs"},
                "position": {"line": 0, "character": 0}
            }),
        )
        .await
        .expect("request");
    client.cancel_request(ticket.id()).await.expect("cancel");
    let result = ticket.wait().await;
    assert!(matches!(result, Err(ClientError::Cancelled)));
}

#[tokio::test(flavor = "current_thread")]
async fn malformed_initialize_response_is_typed_error() {
    let client = LspClient::spawn(lsp_command("malformed_initialize", "utf16"))
        .await
        .expect("spawn client");
    let error = client
        .initialize(initialize_params(PositionEncoding::Utf16))
        .await;
    assert!(matches!(
        error,
        Err(ClientError::Protocol(_) | ClientError::Initialize(_))
    ));
}

#[tokio::test(flavor = "current_thread")]
async fn crashing_server_updates_status() {
    let client = initialize_client("crash_after_open", "utf16").await;
    let mut events = client.subscribe();
    client
        .did_open(DidOpenTextDocumentParams {
            text_document: TextDocumentItem {
                uri: DocumentUri("file:///workspace/main.rs".to_owned()),
                language_id: "rust".to_owned(),
                version: 1,
                text: "diagnostic".to_owned(),
            },
        })
        .await
        .expect("didOpen");
    let event = time::timeout(Duration::from_secs(2), async {
        loop {
            let event = events.recv().await.expect("event");
            if matches!(
                event,
                ClientEvent::Crashed { .. } | ClientEvent::Exited { .. }
            ) {
                break event;
            }
        }
    })
    .await
    .expect("event arrives");
    assert!(matches!(
        event,
        ClientEvent::Crashed { .. } | ClientEvent::Exited { .. }
    ));
    assert!(matches!(
        time::timeout(Duration::from_secs(2), async {
            loop {
                if matches!(
                    client.status().await,
                    lsp_client::ClientStatus::Crashed { .. }
                ) {
                    break;
                }
                time::sleep(Duration::from_millis(25)).await;
            }
        })
        .await,
        Ok(())
    ));
}

#[tokio::test(flavor = "current_thread")]
async fn formatter_runner_covers_success_and_failures() {
    let runner = FormatterRunner::new();
    let snapshot = text_snapshot("hello");

    let success = runner
        .run(
            &snapshot,
            &FormatterSpec {
                command: formatter_command("formatter_uppercase"),
                timeout: Duration::from_secs(2),
            },
            &ProcessCancelToken::new(),
        )
        .await
        .expect("success");
    assert_eq!(success.replacement, "HELLO");

    let timeout_error = runner
        .run(
            &snapshot,
            &FormatterSpec {
                command: formatter_command("formatter_sleep"),
                timeout: Duration::from_millis(100),
            },
            &ProcessCancelToken::new(),
        )
        .await;
    assert!(matches!(
        timeout_error,
        Err(lsp_client::FormatterError::TimedOut(_))
    ));

    let non_zero = runner
        .run(
            &snapshot,
            &FormatterSpec {
                command: formatter_command("formatter_nonzero"),
                timeout: Duration::from_secs(2),
            },
            &ProcessCancelToken::new(),
        )
        .await;
    assert!(matches!(
        non_zero,
        Err(lsp_client::FormatterError::NonZeroExit { .. })
    ));

    let invalid_utf8 = runner
        .run(
            &snapshot,
            &FormatterSpec {
                command: formatter_command("formatter_invalid_utf8"),
                timeout: Duration::from_secs(2),
            },
            &ProcessCancelToken::new(),
        )
        .await;
    assert!(matches!(
        invalid_utf8,
        Err(lsp_client::FormatterError::InvalidUtf8(_))
    ));

    let cancel = ProcessCancelToken::new();
    let cancel_spec = FormatterSpec {
        command: formatter_command("formatter_sleep"),
        timeout: Duration::from_secs(2),
    };
    let cancelled = runner.run(&snapshot, &cancel_spec, &cancel);
    cancel.cancel();
    assert!(matches!(
        cancelled.await,
        Err(lsp_client::FormatterError::Cancelled)
    ));
}
