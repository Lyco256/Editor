# Workspace views

Role: owns the pure workspace UI surface for Explorer, Quick Open, search, recent workspaces, and trust prompts.
The module keeps all interaction in-memory and renders from fake model state into the terminal framebuffer.

Important types: `WorkspaceView`, `WorkspaceUi`, and the re-exported state/action types from `model.rs`.

Invariants: drawing never performs filesystem, process, or network I/O; destructive operations are represented
as confirmation prompts only; Explorer state is driven by model events rather than live traversal.

Data flow: model events update `WorkspaceUiState`, actions mutate the in-memory state, and render helpers
emit framebuffer cells for the active workspace view.

Concurrency assumptions: the view layer is single-threaded and reacts to already-collected model data.

Error behavior: the module is intentionally total for valid in-memory state and does not surface typed I/O
errors because it never opens files itself.

Dependencies: `editor-types` for input and style types, `terminal-backend` for the framebuffer, and the
workspace-local `model` and `render` helpers.

Tests: framebuffer/state tests cover multi-root Explorer rendering, compact collapse, file prompts, Quick Open,
streamed search, superseded search, replace preview, and trust states.
