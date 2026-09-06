# Workspace model

Role: defines the in-memory workspace tree, Quick Open candidates, search sessions, trust state, and
confirmation prompts used by the workspace UI.

Important types: `WorkspaceEntry`, `ExplorerState`, `QuickOpenState`, `SearchState`, `WorkspacePrompt`,
`WorkspaceModelEvent`, and `WorkspaceUiState`.

Invariants: Explorer entries already contain the fake model tree; draw code never invents filesystem data.
Quick Open and search results are filtered and sorted from already-loaded candidates and streamed events.
Stale search results are ignored by session id.

Data flow: model events update `WorkspaceUiState`; view code reads that state and renders a framebuffer;
actions mutate the same state without directly touching the filesystem.

Concurrency assumptions: this layer is pure state and does not own any background task or channel.

Error behavior: the module returns no external errors and treats invalid selections as no-ops.

Dependencies: `editor-types` for mouse-related helpers and screen coordinates, plus `std::path` for fake
workspace paths.

Tests: unit tests in `mod.rs` exercise the state transitions through the render layer.
# Workspace search and recents

Search options include include/exclude globs in addition to literal/regex, case and whole-word
controls. Recent workspace rows carry stable ids and missing-path state and are capped at twenty.
