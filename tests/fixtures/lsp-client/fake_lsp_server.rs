#![allow(clippy::too_many_lines)]

use std::{
    collections::{HashMap, HashSet},
    env,
    time::Duration,
};

use lsp_client::protocol::{
    self, DidChangeTextDocumentParams, DidOpenTextDocumentParams, IncomingMessage,
    JsonRpcNotification, JsonRpcResponse, PositionEncoding, PublishDiagnosticsParams, RequestId,
};
use serde_json::{Value as JsonValue, json};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt, BufReader},
    time,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Lsp,
    Formatter,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Scenario {
    Normal,
    MalformedInitialize,
    CrashAfterOpen,
    SlowCompletion,
    Stderr,
    FormatterUppercase,
    FormatterSleep,
    FormatterNonZero,
    FormatterInvalidUtf8,
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    if let Err(error) = run().await {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

async fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = env::args().skip(1);
    let kind = match args.next().as_deref() {
        Some("formatter") => Kind::Formatter,
        _ => Kind::Lsp,
    };
    let scenario = match args.next().as_deref() {
        Some("malformed_initialize") => Scenario::MalformedInitialize,
        Some("crash_after_open") => Scenario::CrashAfterOpen,
        Some("slow_completion") => Scenario::SlowCompletion,
        Some("stderr") => Scenario::Stderr,
        Some("formatter_uppercase") => Scenario::FormatterUppercase,
        Some("formatter_sleep") => Scenario::FormatterSleep,
        Some("formatter_nonzero") => Scenario::FormatterNonZero,
        Some("formatter_invalid_utf8") => Scenario::FormatterInvalidUtf8,
        _ => Scenario::Normal,
    };
    match kind {
        Kind::Formatter => run_formatter(scenario).await,
        Kind::Lsp => run_lsp(scenario, args.next()).await,
    }
}

async fn run_formatter(scenario: Scenario) -> Result<(), Box<dyn std::error::Error>> {
    let mut input = String::new();
    tokio::io::stdin().read_to_string(&mut input).await?;
    let mut stdout = tokio::io::stdout();
    match scenario {
        Scenario::FormatterUppercase => {
            stdout.write_all(input.to_uppercase().as_bytes()).await?;
            stdout.flush().await?;
        }
        Scenario::FormatterSleep => {
            time::sleep(Duration::from_millis(600)).await;
            stdout.write_all(input.as_bytes()).await?;
            stdout.flush().await?;
        }
        Scenario::FormatterNonZero => {
            eprintln!("formatter failed");
            std::process::exit(3);
        }
        Scenario::FormatterInvalidUtf8 => {
            stdout.write_all(&[0xFF, 0xFE, 0xFF]).await?;
            stdout.flush().await?;
        }
        _ => {
            stdout.write_all(input.as_bytes()).await?;
            stdout.flush().await?;
        }
    }
    Ok(())
}

async fn run_lsp(
    scenario: Scenario,
    encoding_hint: Option<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut reader = BufReader::new(tokio::io::stdin());
    let mut stdout = tokio::io::stdout();
    let encoding = match encoding_hint.as_deref() {
        Some("utf8") => PositionEncoding::Utf8,
        _ => PositionEncoding::Utf16,
    };
    let mut documents: HashMap<String, String> = HashMap::new();
    let mut cancelled: HashSet<u64> = HashSet::new();
    if matches!(scenario, Scenario::Stderr) {
        eprintln!("fake server stderr line");
    }
    loop {
        let Some(message) = protocol::read_message(&mut reader).await? else {
            break;
        };
        match message {
            IncomingMessage::Notification(notification) => {
                if notification.method == "$/cancelRequest" {
                    if let Some(id) = notification
                        .params
                        .as_ref()
                        .and_then(|params| params.get("id"))
                        .and_then(JsonValue::as_i64)
                        .and_then(|value| u64::try_from(value).ok())
                    {
                        cancelled.insert(id);
                    }
                    continue;
                }
                if notification.method == "textDocument/didOpen" {
                    let params: DidOpenTextDocumentParams = serde_json::from_value(
                        notification.params.clone().unwrap_or(JsonValue::Null),
                    )?;
                    documents.insert(
                        params.text_document.uri.0.clone(),
                        params.text_document.text.clone(),
                    );
                    if matches!(scenario, Scenario::CrashAfterOpen) {
                        eprintln!("crashing after didOpen");
                        std::process::exit(1);
                    }
                    publish_diagnostics(
                        &mut stdout,
                        &params.text_document.uri.0,
                        &params.text_document.text,
                    )
                    .await?;
                }
                if notification.method == "textDocument/didChange" {
                    let params: DidChangeTextDocumentParams = serde_json::from_value(
                        notification.params.clone().unwrap_or(JsonValue::Null),
                    )?;
                    let entry = documents
                        .entry(params.text_document.uri.0.clone())
                        .or_default();
                    for change in params.content_changes {
                        if let Some(range) = change.range {
                            apply_range_change(
                                entry,
                                range.start.character as usize,
                                range.end.character as usize,
                                &change.text,
                            );
                        } else {
                            *entry = change.text;
                        }
                    }
                    publish_diagnostics(&mut stdout, &params.text_document.uri.0, entry).await?;
                }
            }
            IncomingMessage::Request(request) => {
                if request.method == "initialize" {
                    if matches!(scenario, Scenario::MalformedInitialize) {
                        stdout
                            .write_all(b"Content-Length: 9\r\n\r\n{not json")
                            .await?;
                        stdout.flush().await?;
                        return Ok(());
                    }
                    let response = JsonRpcResponse {
                        jsonrpc: "2.0".to_owned(),
                        id: request.id,
                        result: Some(json!({
                            "capabilities": {
                                "completionProvider": {"resolveProvider": true},
                                "hoverProvider": true,
                                "signatureHelpProvider": true,
                                "definitionProvider": true,
                                "declarationProvider": true,
                                "implementationProvider": true,
                                "referencesProvider": true,
                                "documentSymbolProvider": true,
                                "workspaceSymbolProvider": true,
                                "renameProvider": {"prepareProvider": true},
                                "codeActionProvider": true,
                                "documentFormattingProvider": true,
                                "documentRangeFormattingProvider": true,
                                "semanticTokensProvider": {"full": true},
                                "inlayHintProvider": true,
                                "workspace": {"workspaceFolders": true}
                            },
                            "positionEncoding": match encoding {
                                PositionEncoding::Utf8 => "utf-8",
                                PositionEncoding::Utf16 => "utf-16",
                            },
                            "serverInfo": {"name": "fake-lsp", "version": "1"}
                        })),
                        error: None,
                    };
                    protocol::write_message(&mut stdout, &response).await?;
                    continue;
                }
                if request.method == "shutdown" {
                    respond_json(&mut stdout, request.id, JsonValue::Null).await?;
                    continue;
                }
                if request.method == "textDocument/completion" {
                    if matches!(scenario, Scenario::SlowCompletion) {
                        time::sleep(Duration::from_millis(250)).await;
                    }
                    let request_id = request.numeric_id()?;
                    if cancelled.contains(&request_id) {
                        continue;
                    }
                    let current = documents.values().next().cloned().unwrap_or_default();
                    respond_json(
                        &mut stdout,
                        request.id,
                        json!({
                            "items": [{
                                "label": if current.contains("complete") { "completed" } else { "plain" },
                                "kind": 3
                            }]
                        }),
                    )
                    .await?;
                    continue;
                }
                if request.method == "completionItem/resolve" {
                    respond_json(
                        &mut stdout,
                        request.id,
                        json!({"label": "resolved", "detail": "resolved item"}),
                    )
                    .await?;
                    continue;
                }
                if request.method == "textDocument/hover" {
                    respond_json(&mut stdout, request.id, json!({"contents": "hover"})).await?;
                    continue;
                }
                if request.method == "textDocument/signatureHelp" {
                    respond_json(
                        &mut stdout,
                        request.id,
                        json!({
                            "signatures": [{"label": "sig"}],
                            "activeSignature": 0,
                            "activeParameter": 0
                        }),
                    )
                    .await?;
                    continue;
                }
                if request.method == "textDocument/definition"
                    || request.method == "textDocument/declaration"
                    || request.method == "textDocument/implementation"
                    || request.method == "textDocument/references"
                {
                    respond_json(&mut stdout, request.id, json!([location(encoding)])).await?;
                    continue;
                }
                if request.method == "textDocument/documentSymbol" {
                    respond_json(
                        &mut stdout,
                        request.id,
                        json!([{
                            "name": "symbol",
                            "kind": 12,
                            "range": symbol_range(),
                            "selectionRange": symbol_range()
                        }]),
                    )
                    .await?;
                    continue;
                }
                if request.method == "workspace/symbol" {
                    respond_json(
                        &mut stdout,
                        request.id,
                        json!([{
                            "name": "workspace-symbol",
                            "kind": 12,
                            "location": location(encoding)
                        }]),
                    )
                    .await?;
                    continue;
                }
                if request.method == "textDocument/prepareRename" {
                    respond_json(
                        &mut stdout,
                        request.id,
                        json!({
                            "range": symbol_range(),
                            "placeholder": "rename-me"
                        }),
                    )
                    .await?;
                    continue;
                }
                if request.method == "textDocument/rename" {
                    respond_json(
                        &mut stdout,
                        request.id,
                        json!({
                            "changes": {
                                "file:///workspace/main.rs": [{
                                    "range": symbol_range(),
                                    "newText": "renamed"
                                }]
                            }
                        }),
                    )
                    .await?;
                    continue;
                }
                if request.method == "textDocument/codeAction" {
                    respond_json(
                        &mut stdout,
                        request.id,
                        json!([{
                            "title": "fix",
                            "kind": "quickfix",
                            "edit": {
                                "changes": {
                                    "file:///workspace/main.rs": [{
                                        "range": symbol_range(),
                                        "newText": "fixed"
                                    }]
                                }
                            }
                        }]),
                    )
                    .await?;
                    continue;
                }
                if request.method == "textDocument/formatting"
                    || request.method == "textDocument/rangeFormatting"
                {
                    respond_json(
                        &mut stdout,
                        request.id,
                        json!([{
                            "range": symbol_range(),
                            "newText": "FORMATTED"
                        }]),
                    )
                    .await?;
                    continue;
                }
                if request.method == "textDocument/semanticTokens/full" {
                    respond_json(&mut stdout, request.id, json!({"data": [0, 0, 5, 1, 0]})).await?;
                    continue;
                }
                if request.method == "textDocument/inlayHint" {
                    respond_json(
                        &mut stdout,
                        request.id,
                        json!([{
                            "position": {"line": 0, "character": 0},
                            "label": "hint"
                        }]),
                    )
                    .await?;
                    continue;
                }
                if request.method == "workspace/applyEdit" {
                    respond_json(&mut stdout, request.id, json!({"applied": true})).await?;
                    continue;
                }
                respond_json(&mut stdout, request.id, JsonValue::Null).await?;
            }
            IncomingMessage::Response(_) => {}
        }
    }
    Ok(())
}

async fn publish_diagnostics(
    stdout: &mut tokio::io::Stdout,
    uri: &str,
    text: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let diagnostics = if text.contains("diagnostic") {
        vec![json!({
            "range": symbol_range(),
            "severity": 1,
            "message": "diagnostic"
        })]
    } else {
        Vec::new()
    };
    let params = PublishDiagnosticsParams {
        uri: protocol::DocumentUri(uri.to_owned()),
        diagnostics: diagnostics
            .into_iter()
            .map(serde_json::from_value)
            .collect::<Result<Vec<protocol::Diagnostic>, _>>()?,
        version: Some(1),
    };
    let notification = JsonRpcNotification {
        jsonrpc: "2.0".to_owned(),
        method: "textDocument/publishDiagnostics".to_owned(),
        params: Some(serde_json::to_value(params)?),
    };
    protocol::write_message(stdout, &notification).await?;
    Ok(())
}

async fn respond_json(
    stdout: &mut tokio::io::Stdout,
    id: RequestId,
    result: JsonValue,
) -> Result<(), Box<dyn std::error::Error>> {
    protocol::write_message(
        stdout,
        &JsonRpcResponse {
            jsonrpc: "2.0".to_owned(),
            id,
            result: Some(result),
            error: None,
        },
    )
    .await?;
    Ok(())
}

fn location(encoding: PositionEncoding) -> JsonValue {
    json!({
        "uri": "file:///workspace/main.rs",
        "range": symbol_range_for_encoding(encoding)
    })
}

fn symbol_range() -> JsonValue {
    json!({
        "start": {"line": 0, "character": 0},
        "end": {"line": 0, "character": 5}
    })
}

fn symbol_range_for_encoding(encoding: PositionEncoding) -> JsonValue {
    let end = match encoding {
        PositionEncoding::Utf8 | PositionEncoding::Utf16 => 5,
    };
    json!({
        "start": {"line": 0, "character": 0},
        "end": {"line": 0, "character": end}
    })
}

fn apply_range_change(text: &mut String, start: usize, end: usize, replacement: &str) {
    let start = char_to_byte(text, start);
    let end = char_to_byte(text, end);
    text.replace_range(start..end, replacement);
}

fn char_to_byte(text: &str, char_offset: usize) -> usize {
    text.char_indices()
        .nth(char_offset)
        .map_or(text.len(), |(byte, _)| byte)
}

trait RequestIdExt {
    fn numeric_id(&self) -> Result<u64, Box<dyn std::error::Error>>;
}

impl RequestIdExt for protocol::JsonRpcRequest {
    fn numeric_id(&self) -> Result<u64, Box<dyn std::error::Error>> {
        match &self.id {
            RequestId::Number(value) if *value >= 0 => Ok(u64::try_from(*value)?),
            other => Err(format!("unsupported request id: {other:?}").into()),
        }
    }
}
