#![allow(clippy::missing_errors_doc)]
#![allow(clippy::struct_excessive_bools)]

use std::{
    collections::HashMap,
    marker::PhantomData,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
};

use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::{Value as JsonValue, json};
use thiserror::Error;
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    process::{Child, ChildStdin, ChildStdout, Command},
    sync::{Mutex, RwLock, broadcast, oneshot},
};

use editor_types::LogicalPosition;

use crate::TextSnapshot;
use crate::position::PositionMapper;
use crate::protocol::{
    self, CommandSpec, DidChangeTextDocumentParams, DidChangeWorkspaceFoldersParams,
    DidCloseTextDocumentParams, DidOpenTextDocumentParams, DidSaveTextDocumentParams,
    IncomingMessage, InitializeError, InitializeParams, InitializeResponse, JsonRpcNotification,
    JsonRpcRequest, JsonRpcResponse, PositionEncoding, PublishDiagnosticsParams, RequestId,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TypedRequestId(u64);

impl TypedRequestId {
    #[must_use]
    pub const fn as_u64(self) -> u64 {
        self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClientStatus {
    Starting,
    Running,
    Stopping,
    Stopped,
    Crashed { message: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct NegotiatedCapabilities {
    pub completion: bool,
    pub completion_resolve: bool,
    pub hover: bool,
    pub signature_help: bool,
    pub definition: bool,
    pub declaration: bool,
    pub implementation: bool,
    pub references: bool,
    pub document_symbols: bool,
    pub workspace_symbols: bool,
    pub rename: bool,
    pub prepare_rename: bool,
    pub code_action: bool,
    pub document_formatting: bool,
    pub range_formatting: bool,
    pub semantic_tokens: bool,
    pub inlay_hints: bool,
    pub workspace_folders: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClientEvent {
    Notification {
        method: String,
        params: Option<JsonValue>,
    },
    Diagnostics(PublishDiagnosticsParams),
    ServerRequest {
        id: RequestId,
        method: String,
        params: Option<JsonValue>,
    },
    StderrLine(String),
    ProtocolError(String),
    Exited {
        status: Option<i32>,
    },
    Crashed {
        message: String,
    },
}

#[derive(Debug, Error)]
pub enum ClientError {
    #[error("could not serialize request parameters: {0}")]
    Serialize(#[from] serde_json::Error),
    #[error("could not send message to the server: {0}")]
    Transport(#[source] std::io::Error),
    #[error(transparent)]
    Protocol(#[from] protocol::ProtocolError),
    #[error("initialize response was invalid: {0}")]
    Initialize(#[from] InitializeError),
    #[error("the request was cancelled")]
    Cancelled,
    #[error("the server exited before the request completed")]
    ServerExited,
    #[error("the server crashed: {message}")]
    Crashed { message: String },
    #[error("the server returned an error response {code}: {message}")]
    ServerError {
        code: i64,
        message: String,
        data: Option<JsonValue>,
    },
    #[error("the response payload could not be decoded: {0}")]
    Decode(serde_json::Error),
}

pub type Subscription = broadcast::Receiver<ClientEvent>;
pub type PendingRequest<T> = RequestTicket<T>;

#[derive(Debug)]
pub struct RequestTicket<T> {
    id: TypedRequestId,
    receiver: oneshot::Receiver<Result<JsonValue, ClientError>>,
    marker: PhantomData<T>,
}

impl<T> RequestTicket<T> {
    #[must_use]
    pub const fn id(&self) -> TypedRequestId {
        self.id
    }

    pub async fn wait(self) -> Result<T, ClientError>
    where
        T: DeserializeOwned,
    {
        let payload = self
            .receiver
            .await
            .map_err(|_| ClientError::ServerExited)??;
        serde_json::from_value(payload).map_err(ClientError::Decode)
    }
}

struct PendingEntry {
    sender: oneshot::Sender<Result<JsonValue, ClientError>>,
}

struct ProcessState {
    generation: u64,
    writer: Option<ChildStdin>,
    child: Option<Child>,
    shutdown_requested: bool,
    exit_requested: bool,
}

struct Inner {
    command: CommandSpec,
    process: Mutex<ProcessState>,
    pending: Mutex<HashMap<u64, PendingEntry>>,
    next_request_id: AtomicU64,
    status: RwLock<ClientStatus>,
    position_encoding: RwLock<PositionEncoding>,
    capabilities: RwLock<NegotiatedCapabilities>,
    events: broadcast::Sender<ClientEvent>,
    crashed: AtomicBool,
}

#[derive(Clone)]
pub struct LspClient {
    inner: Arc<Inner>,
}

impl LspClient {
    pub async fn spawn(command: CommandSpec) -> Result<Self, ClientError> {
        let (events, _) = broadcast::channel(128);
        let inner = Arc::new(Inner {
            command,
            process: Mutex::new(ProcessState {
                generation: 0,
                writer: None,
                child: None,
                shutdown_requested: false,
                exit_requested: false,
            }),
            pending: Mutex::new(HashMap::new()),
            next_request_id: AtomicU64::new(1),
            status: RwLock::new(ClientStatus::Starting),
            position_encoding: RwLock::new(PositionEncoding::Utf16),
            capabilities: RwLock::new(NegotiatedCapabilities::default()),
            events,
            crashed: AtomicBool::new(false),
        });
        let client = Self {
            inner: Arc::clone(&inner),
        };
        client.spawn_process(0).await?;
        Ok(client)
    }

    #[must_use]
    pub fn subscribe(&self) -> Subscription {
        self.inner.events.subscribe()
    }

    pub async fn status(&self) -> ClientStatus {
        self.inner.status.read().await.clone()
    }

    pub async fn negotiated_position_encoding(&self) -> PositionEncoding {
        *self.inner.position_encoding.read().await
    }

    pub async fn negotiated_capabilities(&self) -> NegotiatedCapabilities {
        self.inner.capabilities.read().await.clone()
    }

    pub async fn initialize(
        &self,
        params: InitializeParams,
    ) -> Result<InitializeResponse, ClientError> {
        let requested_encodings = params.position_encodings.clone();
        let response = self.request_json("initialize", params).await?;
        let parsed = InitializeResponse::from_json(&response, &requested_encodings)
            .map_err(ClientError::Initialize)?;
        *self.inner.position_encoding.write().await = parsed.position_encoding;
        *self.inner.capabilities.write().await = parse_capabilities(&parsed.capabilities);
        *self.inner.status.write().await = ClientStatus::Running;
        Ok(parsed)
    }

    pub async fn initialized(&self) -> Result<(), ClientError> {
        self.notify_json("initialized", JsonValue::Null).await
    }

    pub async fn shutdown(&self) -> Result<(), ClientError> {
        *self.inner.status.write().await = ClientStatus::Stopping;
        let _ = self.request_json("shutdown", JsonValue::Null).await?;
        self.notify_json("exit", JsonValue::Null).await?;
        let mut process = self.inner.process.lock().await;
        process.shutdown_requested = true;
        process.exit_requested = true;
        if let Some(writer) = process.writer.as_mut() {
            let _ = writer.shutdown().await;
        }
        process.writer = None;
        Ok(())
    }

    pub async fn restart(&self) -> Result<(), ClientError> {
        self.shutdown().await?;
        self.inner.fail_all_pending_server_exited().await;
        let generation = {
            let mut process = self.inner.process.lock().await;
            process.generation = process.generation.saturating_add(1);
            process.shutdown_requested = false;
            process.exit_requested = false;
            process.generation
        };
        self.spawn_process(generation).await
    }

    pub async fn cancel_request(&self, id: TypedRequestId) -> Result<(), ClientError> {
        self.notify_json("$/cancelRequest", json!({ "id": id.as_u64() }))
            .await?;
        if let Some(entry) = self.inner.pending.lock().await.remove(&id.as_u64()) {
            let _ = entry.sender.send(Err(ClientError::Cancelled));
        }
        Ok(())
    }

    pub async fn request<T>(&self, method: &str, params: impl Serialize) -> Result<T, ClientError>
    where
        T: DeserializeOwned,
    {
        self.request_json(method, params)
            .await
            .and_then(|payload| serde_json::from_value(payload).map_err(ClientError::Decode))
    }

    pub async fn request_json(
        &self,
        method: &str,
        params: impl Serialize,
    ) -> Result<JsonValue, ClientError> {
        self.start_request::<JsonValue>(method, params)
            .await?
            .wait()
            .await
    }

    pub async fn start_request<T>(
        &self,
        method: &str,
        params: impl Serialize,
    ) -> Result<RequestTicket<T>, ClientError> {
        let payload = serde_json::to_value(params)?;
        let id = TypedRequestId(self.inner.next_request_id.fetch_add(1, Ordering::Relaxed));
        let (sender, receiver) = oneshot::channel();
        self.inner
            .pending
            .lock()
            .await
            .insert(id.as_u64(), PendingEntry { sender });
        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_owned(),
            id: RequestId::Number(i64::try_from(id.as_u64()).unwrap_or(i64::MAX)),
            method: method.to_owned(),
            params: match payload {
                JsonValue::Null => None,
                value => Some(value),
            },
        };
        let write_result = self.send_message(request).await;
        if let Err(error) = write_result {
            let _ = self.inner.pending.lock().await.remove(&id.as_u64());
            return Err(error);
        }
        Ok(RequestTicket {
            id,
            receiver,
            marker: PhantomData,
        })
    }

    pub async fn notify_json(
        &self,
        method: &str,
        params: impl Serialize,
    ) -> Result<(), ClientError> {
        let payload = serde_json::to_value(params)?;
        let notification = JsonRpcNotification {
            jsonrpc: "2.0".to_owned(),
            method: method.to_owned(),
            params: match payload {
                JsonValue::Null => None,
                value => Some(value),
            },
        };
        self.send_message(notification).await
    }

    pub async fn did_open(&self, params: DidOpenTextDocumentParams) -> Result<(), ClientError> {
        self.notify_json("textDocument/didOpen", params).await
    }

    pub async fn did_change(&self, params: DidChangeTextDocumentParams) -> Result<(), ClientError> {
        self.notify_json("textDocument/didChange", params).await
    }

    pub async fn did_save(&self, params: DidSaveTextDocumentParams) -> Result<(), ClientError> {
        self.notify_json("textDocument/didSave", params).await
    }

    pub async fn did_close(&self, params: DidCloseTextDocumentParams) -> Result<(), ClientError> {
        self.notify_json("textDocument/didClose", params).await
    }

    pub async fn did_change_workspace_folders(
        &self,
        params: DidChangeWorkspaceFoldersParams,
    ) -> Result<(), ClientError> {
        self.notify_json("workspace/didChangeWorkspaceFolders", params)
            .await
    }

    pub async fn completion(&self, params: impl Serialize) -> Result<JsonValue, ClientError> {
        self.request_json("textDocument/completion", params).await
    }

    pub async fn completion_resolve(
        &self,
        params: impl Serialize,
    ) -> Result<JsonValue, ClientError> {
        self.request_json("completionItem/resolve", params).await
    }

    pub async fn hover(&self, params: impl Serialize) -> Result<JsonValue, ClientError> {
        self.request_json("textDocument/hover", params).await
    }

    pub async fn signature_help(&self, params: impl Serialize) -> Result<JsonValue, ClientError> {
        self.request_json("textDocument/signatureHelp", params)
            .await
    }

    pub async fn definition(&self, params: impl Serialize) -> Result<JsonValue, ClientError> {
        self.request_json("textDocument/definition", params).await
    }

    pub async fn declaration(&self, params: impl Serialize) -> Result<JsonValue, ClientError> {
        self.request_json("textDocument/declaration", params).await
    }

    pub async fn implementation(&self, params: impl Serialize) -> Result<JsonValue, ClientError> {
        self.request_json("textDocument/implementation", params)
            .await
    }

    pub async fn references(&self, params: impl Serialize) -> Result<JsonValue, ClientError> {
        self.request_json("textDocument/references", params).await
    }

    pub async fn document_symbols(&self, params: impl Serialize) -> Result<JsonValue, ClientError> {
        self.request_json("textDocument/documentSymbol", params)
            .await
    }

    pub async fn workspace_symbols(
        &self,
        params: impl Serialize,
    ) -> Result<JsonValue, ClientError> {
        self.request_json("workspace/symbol", params).await
    }

    pub async fn rename(&self, params: impl Serialize) -> Result<JsonValue, ClientError> {
        self.request_json("textDocument/rename", params).await
    }

    pub async fn prepare_rename(&self, params: impl Serialize) -> Result<JsonValue, ClientError> {
        self.request_json("textDocument/prepareRename", params)
            .await
    }

    pub async fn code_action(&self, params: impl Serialize) -> Result<JsonValue, ClientError> {
        self.request_json("textDocument/codeAction", params).await
    }

    pub async fn formatting(&self, params: impl Serialize) -> Result<JsonValue, ClientError> {
        self.request_json("textDocument/formatting", params).await
    }

    pub async fn range_formatting(&self, params: impl Serialize) -> Result<JsonValue, ClientError> {
        self.request_json("textDocument/rangeFormatting", params)
            .await
    }

    pub async fn semantic_tokens(&self, params: impl Serialize) -> Result<JsonValue, ClientError> {
        self.request_json("textDocument/semanticTokens/full", params)
            .await
    }

    pub async fn inlay_hints(&self, params: impl Serialize) -> Result<JsonValue, ClientError> {
        self.request_json("textDocument/inlayHint", params).await
    }

    pub async fn apply_workspace_edit(
        &self,
        params: impl Serialize,
    ) -> Result<JsonValue, ClientError> {
        self.request_json("workspace/applyEdit", params).await
    }

    pub async fn map_position(
        &self,
        snapshot: &TextSnapshot,
        position: LogicalPosition,
    ) -> Result<protocol::Position, ClientError> {
        PositionMapper::new(snapshot, self.negotiated_position_encoding().await)
            .to_lsp(position)
            .map_err(|error| ClientError::Protocol(error.into()))
    }

    pub async fn unmap_position(
        &self,
        snapshot: &TextSnapshot,
        position: protocol::Position,
    ) -> Result<LogicalPosition, ClientError> {
        PositionMapper::new(snapshot, self.negotiated_position_encoding().await)
            .from_lsp(position)
            .map_err(|error| ClientError::Protocol(error.into()))
    }

    async fn send_message<M: Serialize>(&self, message: M) -> Result<(), ClientError> {
        let frame = protocol::encode_message(&message).map_err(ClientError::Protocol)?;
        let mut process = self.inner.process.lock().await;
        let writer = process
            .writer
            .as_mut()
            .ok_or_else(|| ClientError::Crashed {
                message: "LSP process stdin is not available".to_owned(),
            })?;
        writer
            .write_all(&frame)
            .await
            .map_err(ClientError::Transport)?;
        writer.flush().await.map_err(ClientError::Transport)
    }

    async fn spawn_process(&self, generation: u64) -> Result<(), ClientError> {
        let mut command = Command::new(&self.inner.command.executable);
        command.args(&self.inner.command.args);
        for (key, value) in &self.inner.command.environment {
            command.env(key, value);
        }
        if let Some(current_dir) = &self.inner.command.current_dir {
            command.current_dir(current_dir);
        }
        command.stdin(std::process::Stdio::piped());
        command.stdout(std::process::Stdio::piped());
        command.stderr(std::process::Stdio::piped());
        let mut child = command.spawn().map_err(ClientError::Transport)?;
        let stdin = child.stdin.take().ok_or_else(|| ClientError::Crashed {
            message: "LSP process did not expose stdin".to_owned(),
        })?;
        let stdout = child.stdout.take().ok_or_else(|| ClientError::Crashed {
            message: "LSP process did not expose stdout".to_owned(),
        })?;
        let stderr = child.stderr.take().ok_or_else(|| ClientError::Crashed {
            message: "LSP process did not expose stderr".to_owned(),
        })?;
        {
            let mut process = self.inner.process.lock().await;
            process.generation = generation;
            process.writer = Some(stdin);
            process.child = Some(child);
            process.shutdown_requested = false;
            process.exit_requested = false;
        }
        *self.inner.status.write().await = ClientStatus::Starting;
        self.spawn_stdout_task(stdout, generation);
        self.spawn_stderr_task(stderr, generation);
        Ok(())
    }

    fn spawn_stdout_task(&self, stdout: ChildStdout, generation: u64) {
        let inner = Arc::clone(&self.inner);
        tokio::spawn(async move {
            let mut reader = BufReader::new(stdout);
            loop {
                match protocol::read_message(&mut reader).await {
                    Ok(Some(message)) => {
                        if !inner.generation_matches(generation).await {
                            break;
                        }
                        if let Err(error) = inner.handle_message(message, generation).await {
                            inner.set_crashed_and_fail_pending(error, generation).await;
                            break;
                        }
                    }
                    Ok(None) => {
                        if inner.generation_matches(generation).await {
                            inner.handle_process_exit(generation).await;
                        }
                        break;
                    }
                    Err(error) => {
                        if inner.generation_matches(generation).await {
                            let client_error = match error {
                                protocol::MessageIoError::Io(io_error) => {
                                    ClientError::Transport(io_error)
                                }
                                protocol::MessageIoError::Protocol(protocol_error) => {
                                    ClientError::Protocol(protocol_error)
                                }
                            };
                            inner
                                .set_crashed_and_fail_pending(client_error, generation)
                                .await;
                        }
                        break;
                    }
                }
            }
        });
    }

    fn spawn_stderr_task(&self, stderr: tokio::process::ChildStderr, generation: u64) {
        let inner = Arc::clone(&self.inner);
        tokio::spawn(async move {
            let mut reader = BufReader::new(stderr);
            let mut buffer = Vec::new();
            loop {
                buffer.clear();
                match reader.read_until(b'\n', &mut buffer).await {
                    Ok(0) => break,
                    Ok(_) => {
                        if !inner.generation_matches(generation).await {
                            break;
                        }
                        let line = String::from_utf8_lossy(&buffer).trim_end().to_owned();
                        let _ = inner.events.send(ClientEvent::StderrLine(line));
                    }
                    Err(error) => {
                        let _ = inner
                            .events
                            .send(ClientEvent::ProtocolError(error.to_string()));
                        break;
                    }
                }
            }
        });
    }
}

impl Inner {
    async fn generation_matches(&self, generation: u64) -> bool {
        self.process.lock().await.generation == generation
    }

    async fn handle_message(
        self: &Arc<Self>,
        message: IncomingMessage,
        generation: u64,
    ) -> Result<(), ClientError> {
        match message {
            IncomingMessage::Notification(notification) => {
                self.handle_notification(notification, generation).await
            }
            IncomingMessage::Request(request) => {
                self.handle_server_request(request, generation).await;
                Ok(())
            }
            IncomingMessage::Response(response) => self.handle_response(response, generation).await,
        }
    }

    async fn handle_notification(
        self: &Arc<Self>,
        notification: JsonRpcNotification,
        generation: u64,
    ) -> Result<(), ClientError> {
        if !self.generation_matches(generation).await {
            return Ok(());
        }
        if notification.method == "textDocument/publishDiagnostics" {
            let params = notification.params.clone().ok_or_else(|| {
                ClientError::Protocol(protocol::ProtocolError::InvalidHeader(
                    "diagnostics notification was missing params".to_owned(),
                ))
            })?;
            let diagnostics: PublishDiagnosticsParams = serde_json::from_value(params)?;
            let _ = self.events.send(ClientEvent::Diagnostics(diagnostics));
        } else {
            let _ = self.events.send(ClientEvent::Notification {
                method: notification.method,
                params: notification.params,
            });
        }
        Ok(())
    }

    async fn handle_server_request(self: &Arc<Self>, request: JsonRpcRequest, generation: u64) {
        if !self.generation_matches(generation).await {
            return;
        }
        let _ = self.events.send(ClientEvent::ServerRequest {
            id: request.id,
            method: request.method,
            params: request.params,
        });
    }

    async fn handle_response(
        self: &Arc<Self>,
        response: JsonRpcResponse,
        generation: u64,
    ) -> Result<(), ClientError> {
        if !self.generation_matches(generation).await {
            return Ok(());
        }
        let request_id = match response.id {
            RequestId::Number(value) if value >= 0 => u64::try_from(value).map_err(|_| {
                ClientError::Protocol(protocol::ProtocolError::UnsupportedResponseId(
                    value.to_string(),
                ))
            })?,
            other => {
                return Err(ClientError::Protocol(
                    protocol::ProtocolError::UnsupportedResponseId(format!("{other:?}")),
                ));
            }
        };
        let entry = self.pending.lock().await.remove(&request_id);
        let Some(entry) = entry else {
            return Ok(());
        };
        if let Some(error) = response.error {
            let _ = entry.sender.send(Err(ClientError::ServerError {
                code: error.code,
                message: error.message,
                data: error.data,
            }));
            return Ok(());
        }
        let Some(result) = response.result else {
            let _ = entry.sender.send(Err(ClientError::Protocol(
                protocol::ProtocolError::InvalidHeader(
                    "response was missing both result and error".to_owned(),
                ),
            )));
            return Ok(());
        };
        let _ = entry.sender.send(Ok(result));
        Ok(())
    }

    async fn handle_process_exit(self: &Arc<Self>, generation: u64) {
        let (status, clean_shutdown) = {
            let mut process = self.process.lock().await;
            if process.generation != generation {
                return;
            }
            let status = if let Some(child) = process.child.as_mut() {
                child.wait().await.ok()
            } else {
                None
            };
            (status, process.shutdown_requested && process.exit_requested)
        };
        if let Some(code) = status.and_then(|status| status.code()) {
            let _ = self.events.send(ClientEvent::Exited { status: Some(code) });
        } else {
            let _ = self.events.send(ClientEvent::Exited { status: None });
        }
        if self.crashed.load(Ordering::Relaxed) {
            return;
        }
        let mut status_guard = self.status.write().await;
        *status_guard = if clean_shutdown {
            ClientStatus::Stopped
        } else {
            ClientStatus::Crashed {
                message: "language server exited unexpectedly".to_owned(),
            }
        };
        if matches!(*status_guard, ClientStatus::Crashed { .. }) {
            let _ = self.events.send(ClientEvent::Crashed {
                message: "language server exited unexpectedly".to_owned(),
            });
            self.crashed.store(true, Ordering::Relaxed);
            drop(status_guard);
            self.fail_all_pending_server_exited().await;
        }
    }

    async fn fail_all_pending_server_exited(&self) {
        let mut pending = self.pending.lock().await;
        for (_, entry) in pending.drain() {
            let _ = entry.sender.send(Err(ClientError::ServerExited));
        }
    }

    async fn set_crashed_and_fail_pending(&self, error: ClientError, generation: u64) {
        if !self.generation_matches(generation).await {
            return;
        }
        self.crashed.store(true, Ordering::Relaxed);
        let message = error.to_string();
        *self.status.write().await = ClientStatus::Crashed {
            message: message.clone(),
        };
        let _ = self
            .events
            .send(ClientEvent::ProtocolError(message.clone()));
        let _ = self.events.send(ClientEvent::Crashed { message });
        let mut pending = self.pending.lock().await;
        let mut pending = pending.drain();
        if let Some((_, entry)) = pending.next() {
            let _ = entry.sender.send(Err(error));
        }
        for (_, entry) in pending {
            let _ = entry.sender.send(Err(ClientError::ServerExited));
        }
    }
}

fn parse_capabilities(raw: &JsonValue) -> NegotiatedCapabilities {
    let mut capabilities = NegotiatedCapabilities::default();
    let Some(object) = raw.as_object() else {
        return capabilities;
    };
    capabilities.completion = flag_or_object(object.get("completionProvider"));
    capabilities.completion_resolve = object
        .get("completionProvider")
        .and_then(JsonValue::as_object)
        .is_some_and(|value| {
            value
                .get("resolveProvider")
                .and_then(JsonValue::as_bool)
                .unwrap_or(false)
        });
    capabilities.hover = flag_or_object(object.get("hoverProvider"));
    capabilities.signature_help = flag_or_object(object.get("signatureHelpProvider"));
    capabilities.definition = flag_or_object(object.get("definitionProvider"));
    capabilities.declaration = flag_or_object(object.get("declarationProvider"));
    capabilities.implementation = flag_or_object(object.get("implementationProvider"));
    capabilities.references = flag_or_object(object.get("referencesProvider"));
    capabilities.document_symbols = flag_or_object(object.get("documentSymbolProvider"));
    capabilities.workspace_symbols = flag_or_object(object.get("workspaceSymbolProvider"));
    capabilities.rename = flag_or_object(object.get("renameProvider"));
    capabilities.prepare_rename = object
        .get("renameProvider")
        .and_then(JsonValue::as_object)
        .is_some_and(|value| {
            value
                .get("prepareProvider")
                .and_then(JsonValue::as_bool)
                .unwrap_or(false)
        });
    capabilities.code_action = flag_or_object(object.get("codeActionProvider"));
    capabilities.document_formatting = flag_or_object(object.get("documentFormattingProvider"));
    capabilities.range_formatting = flag_or_object(object.get("documentRangeFormattingProvider"));
    capabilities.semantic_tokens = object.get("semanticTokensProvider").is_some();
    capabilities.inlay_hints = flag_or_object(object.get("inlayHintProvider"));
    capabilities.workspace_folders = object
        .get("workspace")
        .and_then(JsonValue::as_object)
        .and_then(|workspace| workspace.get("workspaceFolders"))
        .and_then(JsonValue::as_bool)
        .unwrap_or(false);
    capabilities
}

fn flag_or_object(value: Option<&JsonValue>) -> bool {
    match value {
        None | Some(JsonValue::Null) => false,
        Some(JsonValue::Bool(value)) => *value,
        Some(_) => true,
    }
}
