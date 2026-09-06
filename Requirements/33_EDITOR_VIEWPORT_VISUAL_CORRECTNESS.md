# Subagent requirement — editor viewport and visual correctness

## Branch

`feat/editor-viewport-visuals`

## Writable ownership

- `crates/app-ui/src/editor/**`
- `docs/crates/app-ui/src/editor/**`
- `tests/fixtures/app-ui/editor/**`
- editor-specific snapshots under `tests/snapshots/**`

Do not edit root `src/app/**`, `crates/editor-types/**`, other feature UI directories, shared module-registration files, or `Cargo.lock`.

## Goal

Make the editor viewport satisfy the original MVP visual requirements using correct coordinate systems and whole-document projection.

## 1. Marker coordinate correctness

`TextRange` contains character offsets. It must never be compared directly with a logical line number.

Remove any implementation equivalent to converting `range.start/end` character offsets to integers and treating those integers as line numbers.

Before gutter/overview rendering, marker ranges are mapped to logical line spans using the current `TextSnapshot`.

All marker tests use realistic multi-line character offsets so an accidental offset-as-line implementation fails.

## 2. Whole-document overview ruler

The right-side overview ruler represents the entire document, not only visible rows.

For a ruler height `H` and document line count `N`, a document line is mapped proportionally into `[0, H-1]`.

The ruler displays:

- viewport location,
- Error,
- Warning,
- Information,
- Hint,
- Git added,
- Git modified,
- Git deleted,
- search matches.

When multiple marker types map to one cell, priority is:

1. Error
2. Warning
3. Git deleted
4. Git modified
5. Git added
6. search match
7. Information
8. Hint

The viewport indicator remains distinguishable without hiding a higher-priority diagnostic marker.

## 3. Gutter markers

The gutter shows visible-line indicators for:

- diagnostic severities,
- Git added/modified/deleted lines,
- fold state,
- current line.

The gutter may reserve multiple narrow cells if required, but line numbers remain readable.

The same marker priority is deterministic across true-color, 256-color, and 16-color modes.

## 4. Inline diagnostic decoration

Diagnostics decorate the affected source text.

Required style request:

- Error: error semantic foreground/underline role.
- Warning: warning semantic foreground/underline role.
- Information: information underline/emphasis role.
- Hint: hint underline/emphasis role.

The editor emits semantic decoration attributes. Terminal capability fallback remains the terminal backend's responsibility.

Selection background has precedence over diagnostic background, but diagnostic underline/emphasis is retained where the cell model allows both.

Syntax foreground remains visible unless the fallback capability requires diagnostic foreground emphasis.

## 5. Search and bracket marker correctness

In-file and project-search ranges projected into the active document use real document offsets and line mapping.

Bracket matches do not participate in the whole-document diagnostic/Git priority unless explicitly rendered as a lower-priority local marker.

## 6. Inline hint primitive

Add a view-only inline hint primitive accepted by `EditorViewportState`.

An inline hint has:

- document position,
- text/glyph sequence,
- hint semantic style.

Hints are drawn as virtual text and do not modify the buffer snapshot, character offsets, selections, undo history, or LSP text.

The top integration stage maps LSP inlay hints into this primitive.

## 7. Cursor and selection rendering

Rendering supports multiple selections and cursors from the supplied authoritative `SelectionSet`.

Primary and secondary cursors remain distinguishable.

Wide characters and combining sequences must not cause a cursor, selection, gutter marker, or inline hint to split a terminal cell incorrectly.

## Tests

Deterministic framebuffer tests cover:

- character-offset to line-span conversion,
- long document with markers near start/middle/end mapped across the ruler,
- viewport indicator in the whole-document ruler,
- diagnostic/Git/search collision priority,
- gutter diagnostic and Git markers,
- diagnostic decoration combined with syntax highlighting,
- diagnostic decoration combined with selection,
- multiple cursors,
- inline hints without changing source offsets,
- wide Unicode and combining text,
- 16-color semantic fallback inputs.

## Acceptance criteria

- No editor rendering helper treats character offsets as logical line numbers.
- The overview ruler maps the whole document.
- Diagnostics are visible in source text, gutter, and overview ruler.
- Git marker inputs can appear in gutter and overview ruler.
- The editor can render supplied inline hints without mutating document text.
- All owned tests, formatting, Clippy, and docs mirror checks pass.
