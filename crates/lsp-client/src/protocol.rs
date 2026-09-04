#![allow(clippy::missing_errors_doc)]
#![allow(clippy::missing_panics_doc)]

use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use serde_json::{Value as JsonValue, json};
use thiserror::Error;
use tokio::io::{
    AsyncBufRead, AsyncBufReadExt, AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt,
};

use editor_types::{CharacterOffset, LogicalPosition, TextRange};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum PositionEncoding {
    Utf8,
    #[default]
    Utf16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Position {
    pub line: u32,
    pub character: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Range {
    pub start: Position,
    pub end: Position,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentUri(pub String);

impl DocumentUri {
    #[must_use]
    pub fn from_path(path: impl AsRef<Path>) -> Self {
        let path = path.as_ref().to_string_lossy().replace('\\', "/");
        let trimmed = path.strip_prefix('/').unwrap_or(&path);
        Self(format!("file:///{trimmed}"))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceFolder {
    pub uri: DocumentUri,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandSpec {
    pub executable: PathBuf,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub environment: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current_dir: Option<PathBuf>,
}

impl CommandSpec {
    #[must_use]
    pub fn new(executable: impl Into<PathBuf>) -> Self {
        Self {
            executable: executable.into(),
            args: Vec::new(),
            environment: BTreeMap::new(),
            current_dir: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TextDocumentIdentifier {
    pub uri: DocumentUri,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionedTextDocumentIdentifier {
    pub uri: DocumentUri,
    pub version: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TextDocumentItem {
    pub uri: DocumentUri,
    #[serde(rename = "languageId")]
    pub language_id: String,
    pub version: i32,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TextDocumentContentChangeEvent {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub range: Option<Range>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub range_length: Option<u32>,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TextDocumentPositionParams {
    pub text_document: TextDocumentIdentifier,
    pub position: Position,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DidOpenTextDocumentParams {
    pub text_document: TextDocumentItem,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DidChangeTextDocumentParams {
    pub text_document: VersionedTextDocumentIdentifier,
    pub content_changes: Vec<TextDocumentContentChangeEvent>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DidSaveTextDocumentParams {
    pub text_document: TextDocumentIdentifier,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DidCloseTextDocumentParams {
    pub text_document: TextDocumentIdentifier,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceFoldersChangeParams {
    pub event: WorkspaceFoldersChangeEvent,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceFoldersChangeEvent {
    #[serde(default)]
    pub added: Vec<WorkspaceFolder>,
    #[serde(default)]
    pub removed: Vec<WorkspaceFolder>,
}

pub type DidChangeWorkspaceFoldersParams = WorkspaceFoldersChangeParams;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Diagnostic {
    pub range: Range,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub severity: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<JsonValue>,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticSeverity {
    Error,
    Warning,
    Information,
    Hint,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PublishDiagnosticsParams {
    pub uri: DocumentUri,
    #[serde(default)]
    pub diagnostics: Vec<Diagnostic>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<i32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerInfo {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InitializeParams {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub process_id: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub root_uri: Option<DocumentUri>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub workspace_folders: Vec<WorkspaceFolder>,
    #[serde(default)]
    pub client_capabilities: JsonValue,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub position_encodings: Vec<PositionEncoding>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub initialization_options: Option<JsonValue>,
}

impl Default for InitializeParams {
    fn default() -> Self {
        Self {
            process_id: None,
            root_uri: None,
            workspace_folders: Vec::new(),
            client_capabilities: json!({}),
            position_encodings: vec![PositionEncoding::Utf16],
            initialization_options: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct InitializeResponse {
    pub capabilities: JsonValue,
    pub position_encoding: PositionEncoding,
    pub server_info: Option<ServerInfo>,
}

#[derive(Debug, Error)]
pub enum InitializeError {
    #[error("initialize response was not an object")]
    NotAnObject,
    #[error("initialize response was missing the capabilities object")]
    MissingCapabilities,
    #[error("server advertised an unsupported position encoding: {0}")]
    UnsupportedPositionEncoding(String),
}

impl InitializeResponse {
    pub fn from_json(
        value: &JsonValue,
        requested_encodings: &[PositionEncoding],
    ) -> Result<Self, InitializeError> {
        let object = value.as_object().ok_or(InitializeError::NotAnObject)?;
        let capabilities = object
            .get("capabilities")
            .cloned()
            .ok_or(InitializeError::MissingCapabilities)?;
        let server_info = object
            .get("serverInfo")
            .and_then(|info| serde_json::from_value::<ServerInfo>(info.clone()).ok());
        let position_encoding = pick_position_encoding(object, &capabilities, requested_encodings);
        Ok(Self {
            capabilities,
            position_encoding,
            server_info,
        })
    }
}

fn pick_position_encoding(
    object: &serde_json::Map<String, JsonValue>,
    capabilities: &JsonValue,
    requested_encodings: &[PositionEncoding],
) -> PositionEncoding {
    if let Some(encoding) = object
        .get("positionEncoding")
        .or_else(|| capabilities.get("positionEncoding"))
        .and_then(position_encoding_from_value)
    {
        return encoding;
    }
    if let Some(encoding) = requested_encodings.first().copied() {
        return encoding;
    }
    PositionEncoding::Utf16
}

fn position_encoding_from_value(value: &JsonValue) -> Option<PositionEncoding> {
    let encoding = value.as_str()?;
    match encoding {
        "utf-8" | "utf8" | "utf-8-sig" | "utf-8-bom" => Some(PositionEncoding::Utf8),
        "utf-16" | "utf16" | "utf-16le" | "utf-16-be" => Some(PositionEncoding::Utf16),
        _ => None,
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JsonRpcErrorObject {
    pub code: i64,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<JsonValue>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum RequestId {
    Number(i64),
    String(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub id: RequestId,
    pub method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<JsonValue>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JsonRpcNotification {
    pub jsonrpc: String,
    pub method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<JsonValue>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    pub id: RequestId,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<JsonValue>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcErrorObject>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IncomingMessage {
    Request(JsonRpcRequest),
    Notification(JsonRpcNotification),
    Response(JsonRpcResponse),
}

#[derive(Debug, Error)]
pub enum ProtocolError {
    #[error("message is missing a Content-Length header")]
    MissingContentLength,
    #[error("invalid Content-Length header: {0}")]
    InvalidContentLength(String),
    #[error("malformed message header: {0}")]
    InvalidHeader(String),
    #[error("failed to decode JSON payload: {0}")]
    InvalidJson(#[from] serde_json::Error),
    #[error("unexpected end of stream while reading an LSP frame")]
    UnexpectedEndOfStream,
    #[error("response id was not a non-negative integer: {0}")]
    UnsupportedResponseId(String),
}

#[derive(Debug, Error)]
pub enum MessageIoError {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Protocol(#[from] ProtocolError),
}

#[must_use]
pub fn encode_message(message: &impl Serialize) -> Vec<u8> {
    let payload = serde_json::to_vec(message).expect("serializing JSON-RPC payload");
    let mut frame = format!("Content-Length: {}\r\n\r\n", payload.len()).into_bytes();
    frame.extend_from_slice(&payload);
    frame
}

pub async fn write_message<W>(
    writer: &mut W,
    message: &impl Serialize,
) -> Result<(), MessageIoError>
where
    W: AsyncWrite + Unpin,
{
    let frame = encode_message(message);
    writer.write_all(&frame).await?;
    writer.flush().await?;
    Ok(())
}

pub async fn read_message<R>(reader: &mut R) -> Result<Option<IncomingMessage>, MessageIoError>
where
    R: AsyncBufRead + AsyncRead + Unpin,
{
    let mut content_length = None;
    loop {
        let mut line = String::new();
        let read = reader.read_line(&mut line).await?;
        if read == 0 {
            if content_length.is_none() {
                return Ok(None);
            }
            return Err(ProtocolError::UnexpectedEndOfStream.into());
        }
        let trimmed = line.trim_end_matches(['\r', '\n']);
        if trimmed.is_empty() {
            break;
        }
        let (name, value) = trimmed
            .split_once(':')
            .ok_or_else(|| ProtocolError::InvalidHeader(trimmed.to_owned()))?;
        if name.eq_ignore_ascii_case("content-length") {
            let length = value
                .trim()
                .parse::<usize>()
                .map_err(|_| ProtocolError::InvalidContentLength(value.trim().to_owned()))?;
            content_length = Some(length);
        }
    }
    let length = content_length.ok_or(ProtocolError::MissingContentLength)?;
    let mut payload = vec![0_u8; length];
    reader.read_exact(&mut payload).await?;
    parse_message(&payload).map(Some).map_err(Into::into)
}

pub fn parse_message(payload: &[u8]) -> Result<IncomingMessage, ProtocolError> {
    let value: JsonValue = serde_json::from_slice(payload)?;
    parse_message_value(&value)
}

pub fn parse_message_value(value: &JsonValue) -> Result<IncomingMessage, ProtocolError> {
    let object = value
        .as_object()
        .ok_or_else(|| ProtocolError::InvalidHeader("payload was not an object".to_owned()))?;
    let jsonrpc = object
        .get("jsonrpc")
        .and_then(JsonValue::as_str)
        .unwrap_or_default();
    let method = object.get("method").and_then(JsonValue::as_str);
    let id = object.get("id");
    if let Some(method) = method {
        if let Some(id) = id {
            return Ok(IncomingMessage::Request(JsonRpcRequest {
                jsonrpc: jsonrpc.to_owned(),
                id: parse_request_id(id)?,
                method: method.to_owned(),
                params: object.get("params").cloned(),
            }));
        }
        return Ok(IncomingMessage::Notification(JsonRpcNotification {
            jsonrpc: jsonrpc.to_owned(),
            method: method.to_owned(),
            params: object.get("params").cloned(),
        }));
    }
    let id = id.ok_or_else(|| ProtocolError::InvalidHeader("response missing id".to_owned()))?;
    Ok(IncomingMessage::Response(JsonRpcResponse {
        jsonrpc: jsonrpc.to_owned(),
        id: parse_request_id(id)?,
        result: object.get("result").cloned(),
        error: object
            .get("error")
            .cloned()
            .map(serde_json::from_value)
            .transpose()?,
    }))
}

fn parse_request_id(value: &JsonValue) -> Result<RequestId, ProtocolError> {
    if let Some(number) = value.as_i64() {
        return Ok(RequestId::Number(number));
    }
    if let Some(text) = value.as_str() {
        return Ok(RequestId::String(text.to_owned()));
    }
    Err(ProtocolError::UnsupportedResponseId(value.to_string()))
}

pub fn lsp_position_to_editor(
    text: &str,
    position: Position,
    encoding: PositionEncoding,
) -> Result<LogicalPosition, ProtocolError> {
    let mut current_line = 0_u32;
    let mut line_start = 0_usize;
    let mut target_line = None;
    for (index, character) in text.char_indices() {
        if current_line == position.line {
            target_line = Some(line_start);
            break;
        }
        if character == '\n' {
            current_line = current_line.saturating_add(1);
            line_start = index + character.len_utf8();
        }
    }
    if current_line == position.line {
        target_line.get_or_insert(line_start);
    } else if position.line == current_line + 1 && text.ends_with('\n') {
        target_line = Some(text.len());
    }
    let line_start = target_line.ok_or(ProtocolError::InvalidHeader(format!(
        "line {} is outside the document",
        position.line
    )))?;
    let line_end = text[line_start..]
        .find('\n')
        .map_or(text.len(), |offset| line_start + offset);
    let line = &text[line_start..line_end];
    let character = match encoding {
        PositionEncoding::Utf8 => decode_utf8_character(line, position.character)?,
        PositionEncoding::Utf16 => decode_utf16_character(line, position.character)?,
    };
    let character = u32::try_from(character)
        .map_err(|_| ProtocolError::InvalidHeader("character offset overflowed u32".to_owned()))?;
    Ok(LogicalPosition {
        line: position.line,
        character,
    })
}

pub fn editor_position_to_lsp(
    text: &str,
    position: LogicalPosition,
    encoding: PositionEncoding,
) -> Result<Position, ProtocolError> {
    let line_start = line_start_offset(text, position.line).ok_or_else(|| {
        ProtocolError::InvalidHeader(format!("line {} is outside the document", position.line))
    })?;
    let line_end = text[line_start..]
        .find('\n')
        .map_or(text.len(), |offset| line_start + offset);
    let line = &text[line_start..line_end];
    let requested = usize::try_from(position.character).map_err(|_| {
        ProtocolError::InvalidHeader("position character overflowed usize".to_owned())
    })?;
    let character = match encoding {
        PositionEncoding::Utf8 => encode_utf8_character(line, requested)?,
        PositionEncoding::Utf16 => encode_utf16_character(line, requested)?,
    };
    Ok(Position {
        line: position.line,
        character,
    })
}

fn line_start_offset(text: &str, line: u32) -> Option<usize> {
    if line == 0 {
        return Some(0);
    }
    let mut current = 0_u32;
    for (index, character) in text.char_indices() {
        if character == '\n' {
            current = current.saturating_add(1);
            if current == line {
                return Some(index + character.len_utf8());
            }
        }
    }
    None
}

fn encode_utf8_character(line: &str, character: usize) -> Result<u32, ProtocolError> {
    if character > line.chars().count() {
        return Err(ProtocolError::InvalidHeader(
            "utf-8 character offset exceeded line length".to_owned(),
        ));
    }
    let byte_offset = line
        .char_indices()
        .nth(character)
        .map_or(line.len(), |(byte, _)| byte);
    u32::try_from(byte_offset).map_err(|_| {
        ProtocolError::InvalidHeader("utf-8 character offset overflowed u32".to_owned())
    })
}

fn decode_utf8_character(line: &str, character: u32) -> Result<usize, ProtocolError> {
    let character = usize::try_from(character).map_err(|_| {
        ProtocolError::InvalidHeader("utf-8 character offset overflowed usize".to_owned())
    })?;
    if character > line.len() {
        return Err(ProtocolError::InvalidHeader(
            "utf-8 character offset exceeded line length".to_owned(),
        ));
    }
    if !line.is_char_boundary(character) {
        return Err(ProtocolError::InvalidHeader(
            "utf-8 character offset split a scalar value".to_owned(),
        ));
    }
    Ok(line[..character].chars().count())
}

fn encode_utf16_character(line: &str, character: usize) -> Result<u32, ProtocolError> {
    let mut units = 0_usize;
    for (index, scalar) in line.chars().enumerate() {
        if index == character {
            return u32::try_from(units).map_err(|_| {
                ProtocolError::InvalidHeader("utf-16 character offset overflowed u32".to_owned())
            });
        }
        units = units.saturating_add(scalar.len_utf16());
    }
    if character == line.chars().count() {
        return u32::try_from(units).map_err(|_| {
            ProtocolError::InvalidHeader("utf-16 character offset overflowed u32".to_owned())
        });
    }
    Err(ProtocolError::InvalidHeader(
        "utf-16 character offset exceeded line length".to_owned(),
    ))
}

fn decode_utf16_character(line: &str, character: u32) -> Result<usize, ProtocolError> {
    let target = usize::try_from(character).map_err(|_| {
        ProtocolError::InvalidHeader("utf-16 character offset overflowed usize".to_owned())
    })?;
    let mut units = 0_usize;
    for (index, scalar) in line.chars().enumerate() {
        if units == target {
            return Ok(index);
        }
        units = units.saturating_add(scalar.len_utf16());
        if units == target {
            return Ok(index + 1);
        }
        if units > target {
            return Err(ProtocolError::InvalidHeader(
                "utf-16 character offset split a surrogate pair".to_owned(),
            ));
        }
    }
    if units == target {
        return Ok(line.chars().count());
    }
    Err(ProtocolError::InvalidHeader(
        "utf-16 character offset exceeded line length".to_owned(),
    ))
}

pub type WorkspaceEdit = JsonValue;

#[must_use]
pub fn text_range_from_lsp(range: Range) -> TextRange {
    TextRange {
        start: CharacterOffset(range.start.character as usize),
        end: CharacterOffset(range.end.character as usize),
    }
}
