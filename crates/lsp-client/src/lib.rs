//! Language Server Protocol client boundary and external formatter runner.
//!
//! This crate owns the stdio LSP transport, request lifecycle, diagnostics stream,
//! UTF-8/UTF-16 position conversion, and the structured external formatter process runner.
//! It deliberately keeps workspace trust and higher-level policy out of the crate boundary.

pub mod client;
pub mod formatter;
pub mod position;
pub mod protocol;

pub use client::{
    ClientError, ClientEvent, ClientStatus, LspClient, NegotiatedCapabilities, PendingRequest,
    RequestTicket, Subscription, TypedRequestId,
};
pub use formatter::{
    FormatterError, FormatterOutcome, FormatterRunner, FormatterSpec, ProcessCancelToken,
};
pub use position::{PositionError, PositionMapper, TextSnapshot};
pub use protocol::{
    CommandSpec, Diagnostic, DiagnosticSeverity, DidChangeTextDocumentParams,
    DidChangeWorkspaceFoldersParams, DidCloseTextDocumentParams, DidOpenTextDocumentParams,
    DidSaveTextDocumentParams, DocumentUri, IncomingMessage, InitializeError, InitializeParams,
    InitializeResponse, JsonRpcErrorObject, JsonRpcNotification, JsonRpcRequest, JsonRpcResponse,
    Position, PositionEncoding, PublishDiagnosticsParams, Range, RequestId, ServerInfo,
    TextDocumentContentChangeEvent, TextDocumentIdentifier, TextDocumentItem,
    TextDocumentPositionParams, VersionedTextDocumentIdentifier, WorkspaceEdit, WorkspaceFolder,
    encode_message, parse_message, read_message, write_message,
};
