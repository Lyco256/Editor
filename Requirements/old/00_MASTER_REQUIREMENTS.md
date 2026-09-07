# Editor — master product requirements

## 1. Product definition

Editor is a Windows-first, terminal-native, non-modal code editor written in Rust.

The product carries forward the core concept of Microsoft Edit: immediate startup, low resource use, small distribution size, keyboard-first operation, mouse support, portability within a normal installer-based desktop installation, and a UI designed for terminal constraints.

The product also provides the major editing and workspace experience expected by a VS Code user. VS Code compatibility is a convenience and ecosystem strategy, not a requirement to reproduce VS Code internals or GUI-only behavior.

Editor is a new application. It is not a fork of Microsoft Edit and does not target source compatibility or detailed UI compatibility with Edit.

## 2. Product priorities

The priorities are fixed in this order:

1. Correctness and data safety.
2. Low perceived latency and an event-driven idle state.
3. Full-featured editing, workspace, language intelligence, diagnostics, search, and Git UX.
4. Small base executable and low memory use compared with VS Code.
5. Compatibility with useful VS Code formats and workflows where terminal constraints do not make that compatibility harmful.
6. Cross-platform support after the Windows experience is stable.

## 3. Platform support

### Tier 1

- Windows 10 x64.
- Windows 11 x64.
- Windows Terminal is the reference terminal.
- Mouse and keyboard are both supported.
- System clipboard integration is required.

### Tier 2

- Modern Linux distributions on x86-64.
- Modern ANSI/VT-capable terminals.
- Keyboard and mouse behavior follows the same logical command model as Windows.

### Tier 3

- Legacy terminals and legacy Windows console behavior are best-effort.
- Unsupported visual attributes fall back without breaking editing.

Windows ARM64 must remain buildable by architecture-neutral code, but it is not a release-blocking target for the initial release.

## 4. Interaction model

Editor is non-modal. Typing text inserts text without entering an Insert mode. Navigation, selection, editing, command invocation, and mouse actions follow conventional desktop-editor behavior.

The default keyboard model is inspired by VS Code and Microsoft Edit without requiring exact shortcut identity where terminal input protocols cannot represent the same chord reliably.

Every core editing and workspace action is reachable from the keyboard. Mouse interaction is additionally supported for clicking, selecting, resizing panes, choosing tabs, using menus/palettes, and interacting with list views.

A Vim compatibility mode is outside the initial release.

## 5. Main UI

The application contains:

- Persistent file Explorer/sidebar by default.
- Tabbed editors.
- Horizontal and vertical split editors.
- Line numbers enabled by default.
- Gutter indicators.
- A one-cell or narrow overview ruler / scrollbar area showing diagnostics, search matches, Git changes, and the viewport location.
- Command Palette.
- Quick Open.
- Bottom panel with Problems and Output views. Integrated Terminal is not part of the initial release.
- Status bar.

The status bar exposes, when applicable:

- active file name and dirty state,
- Git branch,
- language mode,
- encoding,
- line ending,
- indentation mode and size,
- cursor line and column,
- selection information,
- diagnostic counts,
- language server status,
- workspace trust status.

When the terminal becomes too small, lower-priority regions collapse before the active editor. The editor remains interactive even when Explorer or the bottom panel has no usable space.

## 6. Editing capabilities

The initial release includes:

- opening, creating, editing, saving, Save As, closing, and reopening files,
- tabs and split views,
- multiple cursors and multiple selections,
- selection expansion where syntax information exists,
- undo and redo,
- find and replace in file,
- project-wide search and replace,
- bracket matching,
- code folding,
- smart pair insertion for braces, brackets, parentheses, single quotes, double quotes, and language-defined pairs,
- overtype behavior for automatically inserted closing pairs,
- paired deletion for empty auto-inserted pairs,
- smart Enter behavior that expands paired braces and applies indentation,
- automatic indentation,
- format on paste,
- format on save,
- manual document formatting,
- range formatting when the language server supports it,
- configurable tabs/spaces and indentation width,
- `.editorconfig` support,
- system clipboard copy/cut/paste,
- mouse selection,
- dirty-buffer protection.

VS Code-compatible language configuration data is used when available for bracket pairs, comments, word patterns, auto-closing pairs, surrounding pairs, and indentation rules.

## 7. Text model

Normal editable documents use Ropey behind an application-owned `TextBuffer` abstraction.

No other crate outside the editor-core boundary accesses Ropey directly.

The text model separates:

- logical character offsets,
- line/character positions,
- screen/display columns,
- LSP protocol positions.

UTF-8 byte indices, Unicode scalar/character indices, grapheme boundaries, display-cell widths, and UTF-16 protocol offsets are never treated as interchangeable values.

Transactions represent edits. Multiple edits are applied as one transaction. Undo and redo operate on transactions rather than individual low-level writes.

Large-file mode activates by default at **32 MiB** and is configurable. In the initial release, large-file mode unconditionally disables syntax parsing, semantic tokens, LSP, inline Git diff, inlay hints, document symbols, and other whole-document semantic services for that document. File display, editing, saving, and text search remain available. Files up to 1 GiB are not rejected solely because of file size; successful loading still depends on available memory and address space.

## 8. Syntax and language support

Tree-sitter is the syntax engine.

The initial language set is:

- Rust
- C
- C++
- C#
- Java
- Go
- Python
- JavaScript
- TypeScript
- HTML
- CSS
- JSON
- TOML
- YAML
- Markdown
- shell script

For these languages, syntax highlighting and structural parsing are available without a language server.

When an LSP server is available, semantic tokens layer on top of syntax highlighting.

The editor supports syntax-driven folding, matching pairs, structural selection inputs, and document-symbol data where supported by the parser or LSP.

## 9. LSP behavior

Language intelligence uses Language Server Protocol over stdio.

The application does not bundle language servers in the initial release.

A known language server found on `PATH` is detected and started automatically only in a trusted workspace. If no server is found, the editor remains fully usable and shows a non-blocking language-server-unavailable state.

The initial LSP client supports:

- initialize / initialized / shutdown / exit,
- workspace folders,
- didOpen / didChange / didSave / didClose,
- incremental text synchronization,
- diagnostics,
- completion,
- completion resolve,
- hover,
- signature help,
- go to definition,
- go to declaration,
- go to implementation,
- references,
- document symbols,
- workspace symbols,
- rename,
- prepare rename,
- code actions and quick fixes,
- formatting,
- range formatting,
- semantic tokens,
- inlay hints,
- server cancellation where supported,
- server capability negotiation,
- server-requested workspace edits.

Position encoding negotiation is implemented correctly. UTF-16 remains supported for servers that use it. Servers advertising UTF-8 position encoding are handled without lossy conversion.

A crashed language server does not crash the editor. Its stderr is captured for Output, its state becomes visible to the user, and the user can restart it.

## 10. Diagnostics and visual feedback

Diagnostics support Error, Warning, Information, and Hint severities.

Diagnostics are visible through:

- gutter markers,
- text decoration,
- Problems panel,
- overview ruler,
- status summary.

On a terminal supporting colored curly underlines, diagnostics use them. Fallback order is:

1. colored curly underline,
2. colored straight underline,
3. diagnostic foreground emphasis,
4. gutter/overview marker and background emphasis.

The content remains readable at every fallback level.

Diagnostics update while editing when the server supports live diagnostics.

Compiler/build diagnostics generated by a future Build/Task subsystem are outside the initial release. The Problems model is designed so those diagnostics can be added later without changing the UI data model.

## 11. Formatting

Formatting precedence is:

1. active language server formatting capability,
2. configured external formatter command,
3. no formatting action with a clear status message.

External formatter execution is blocked in untrusted workspaces.

Formatting edits are applied as one transaction and are fully undoable.

## 12. Workspace and files

`editor .` opens the current directory as a workspace.

`editor <file>` opens a file.

The workspace model supports:

- one or more workspace roots,
- Explorer tree,
- file creation, rename, move, and delete,
- Quick Open,
- project-wide search,
- project-wide replace,
- `.gitignore`-aware discovery and search,
- excluded paths from settings,
- recent workspaces,
- session restore,
- workspace-specific settings.

Search uses installed `rg` when present and uses an internal Rust fallback when it is not present. Search semantics visible to the user remain consistent between backends for basic literal, regex, case-sensitive, and case-insensitive search.

## 13. Encoding and line endings

The editor preserves source encoding and line endings unless the user explicitly changes them.

Detection order is:

1. BOM,
2. valid UTF-8,
3. heuristic legacy-encoding detection using `chardetng`,
4. configured fallback.

Encoding support is based on the complete set of canonical encodings and aliases exposed by `encoding_rs`, plus explicit UTF-16 LE and UTF-16 BE handling.

The UI permits reopening a file using another supported encoding and saving with another supported encoding.

Line endings support LF and CRLF and preserve the detected form by default.

Mixed line endings are detected and surfaced.

## 14. Recovery and data safety

Unsaved buffers are recovered after normal restart and unexpected termination.

Recovery data is written atomically to the application data directory. It is not stored in the workspace unless the user explicitly saves the file.

The session store preserves:

- workspace roots,
- open files,
- tab order,
- split layout,
- active editor,
- cursor positions,
- selections,
- unsaved buffer contents,
- dirty state.

Saving a file uses an atomic replacement strategy where the platform and filesystem permit it. A failed save never discards the last recoverable unsaved contents.

On panic or terminal failure, terminal modes, cursor visibility, and screen state are restored before process exit whenever process execution still permits cleanup.

## 15. Workspace Trust

Workspace Trust is required.

Trust is stored per canonical workspace path.

An untrusted workspace permits safe viewing and editing but blocks automatic or user-configured external process execution originating from workspace configuration, including:

- language servers,
- external formatters,
- Git operations,
- future tasks/build/run/test,
- future extension hosts.

Static parsing of JSON/JSONC configuration remains allowed.

The UI clearly shows trust state and provides an explicit trust action.

## 16. Git

Git integration uses the installed `git` executable. The editor does not embed libgit2 in the initial release.

Initial Git capabilities include:

- repository detection,
- status,
- branch display,
- diff for working tree and index,
- file and hunk staging,
- unstaging,
- discard with confirmation,
- commit,
- amend,
- branch create,
- branch switch,
- branch delete with confirmation,
- fetch,
- pull,
- push,
- stash create/apply/pop/list,
- basic log/history,
- conflict-file detection.

Advanced merge-editor UI, interactive rebase UI, and visual commit graph are outside the initial release.

Git operations never block the UI thread.

## 17. VS Code compatibility

Compatibility is implemented only where it provides useful interoperability without embedding VS Code.

Initial compatibility includes:

- `.vscode/settings.json` subset,
- JSONC parsing,
- VS Code-style keybinding configuration concepts,
- VS Code color-theme JSON static data,
- VS Code snippet JSON,
- VS Code language configuration JSON,
- `.vscode` workspace settings where mapped commands/settings exist,
- local static VSIX contribution extraction for supported static contribution points.

No Microsoft Marketplace integration is implemented.

No JavaScript/TypeScript VS Code Extension Host is implemented in the initial release.

No WebView extension is implemented.

No Electron-dependent extension behavior is implemented.

Unsupported settings or extension contribution points are ignored with a diagnosable compatibility warning rather than crashing or silently corrupting configuration.

## 18. Theme behavior

The initial release ships with one complete dark theme.

Theme data uses logical semantic roles internally rather than terminal palette indices.

Rendering supports:

- true color where supported,
- 256-color quantization,
- 16-color quantization.

The same theme semantics are preserved across these modes even when exact RGB output differs.

Additional themes and VS Code theme imports are supported by the compatibility layer but are not required to match GUI rendering pixel-for-pixel.

## 19. Performance and resource rules

The UI event loop is event-driven. No subsystem uses high-frequency idle polling.

Typing must not trigger a whole-workspace scan, full Git status refresh, or synchronous language-server request.

Parsing, LSP I/O, search, filesystem traversal, and Git execution run outside the UI update path and return typed events/messages.

The root Cargo release profile uses `opt-level = "s"`, fat LTO, `codegen-units = 1`, symbol stripping, and `panic = "abort"`. A panic hook performs best-effort terminal restoration and safe crash logging before abort.

The base editor starts without launching a language server, Git process, search process, extension host, or other subprocess until the current workspace needs that subsystem.

The initial quality budgets are:

- empty-workspace idle CPU target below 1% on the Windows reference environment,
- empty-workspace resident memory target below 100 MiB,
- release executable plus mandatory bundled assets target below 50 MiB before installer compression,
- no visible multi-frame input stall during normal editing of a 10 MiB source file,
- no unbounded memory growth during repeated open/close cycles.

These budgets are release gates unless a measured platform limitation is documented by the top Codex and approved by the user.

## 20. Initial-release exclusions

The following are explicitly outside the initial release:

- integrated terminal,
- build/run/test task execution UI,
- DAP debugger,
- AI features,
- Remote SSH,
- collaborative editing,
- JavaScript/TypeScript VS Code Extension Host,
- WebView extensions,
- complex GUI extensions,
- Vim mode,
- advanced visual merge editor,
- interactive rebase UI,
- auto-download of language servers or formatters,
- extension marketplace service.

Architecture must not prevent these from being added later.

## 21. Distribution and licensing

The application is installed by an installer. Single-file portability is not a release requirement.

The final product name is not fixed. `Editor` is the working title and `editor` is the working command name inside requirements and tests. The final executable name remains replaceable without changing domain APIs.

Licensing is undecided. The repository must not claim an open-source license or publish package metadata that implies one until the user makes a licensing decision.

The initial implementation is not prepared for public release unless explicitly requested later.
