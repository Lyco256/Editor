# Subagent requirement — LSP client

## Branch

`feat/lsp-client`

## Writable ownership

- `crates/lsp-client/**`
- `docs/crates/lsp-client/**`
- `tests/fixtures/lsp-client/**`

Do not edit root application files, shared types, other crates, or `Cargo.lock`.

## Scope

Implement a robust Language Server Protocol client over stdio and the language-tooling external formatter runner used when LSP formatting is unavailable.

## Process lifecycle

The client:

- resolves a configured or known server executable,
- starts it only after root policy has allowed external execution,
- initializes capabilities,
- owns stdin/stdout framing,
- captures stderr,
- sends shutdown/exit on clean stop,
- detects crash/EOF,
- reports status without crashing the editor,
- supports explicit restart.

No automatic language-server download exists.

## External formatter runner

The crate exposes a formatter runner that accepts an already-authorized formatter specification from root orchestration. It invokes the executable with structured arguments, sends document content according to the formatter specification, captures stdout/stderr, supports cancellation/timeout, and returns one replacement edit or a typed failure.

The runner does not decide Workspace Trust. Root orchestration must authorize the effect before this crate receives it.

## Protocol capabilities

Implement:

- initialize,
- initialized,
- shutdown,
- exit,
- workspace folder notifications,
- didOpen,
- didChange with incremental sync,
- didSave,
- didClose,
- diagnostics,
- completion,
- completion resolve,
- hover,
- signature help,
- definition,
- declaration,
- implementation,
- references,
- document symbols,
- workspace symbols,
- rename,
- prepare rename,
- code action,
- workspace edits,
- document formatting,
- range formatting,
- semantic tokens,
- inlay hints,
- cancellation.

Unsupported server capabilities are represented as unavailable, not as errors.

## Position mapping

Implement explicit conversion between editor-core positions and negotiated LSP position encoding.

Support UTF-16 and UTF-8 protocol positions.

Test multibyte text, emoji, combining sequences, and mixed ASCII/non-ASCII lines.

## Concurrency

Requests have unique IDs and cancellation.

Late responses to superseded requests are ignored.

The client never blocks UI state mutation while waiting for server responses.

## Test server

Create a deterministic fake stdio LSP server fixture/process used by integration tests.

It supports enough behavior to test initialization, incremental sync, diagnostics, completion, rename, formatting, semantic tokens, cancellation, malformed response handling, stderr, and simulated crash.

## Acceptance criteria

- Required protocol features round-trip through the fake server.
- Invalid/malformed server messages become typed protocol errors and do not panic.
- Server crash leaves the editor-side client recoverable.
- External formatter success, timeout, non-zero exit, malformed output, and cancellation are tested without shell command concatenation.
- Position conversions pass Unicode tests.
- `cargo test -p lsp-client` and Clippy pass.
- All owned source files have mirrored documentation.
