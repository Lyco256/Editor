# Language intelligence views

Role: owns the pure UI model for language intelligence. The module accepts versioned language-server
results, rejects stale data, groups diagnostics into Problems, and exposes panel-oriented state for
completion, hover, signature help, go-to and references choosers, rename previews, code actions,
inlay hints, symbols, formatting feedback, and language-server status. Accepted contextual results
also expose a cursor-anchored overlay model for popup rendering.

Important types:

- `Glyph`, `PanelRow`, and `PanelState` describe the reusable framebuffer-friendly view primitives.
- `Versioned<T>` and `VersionedState<T>` implement stale-result suppression by document version.
- `SpanSet` and `StyledSpan` carry syntax and semantic token overlays.
- `DiagnosticRecord`, `ProblemsView`, and `DiagnosticDashboard` normalize diagnostics into grouped
  Problems data with severity summaries and decorative markers.
- `CompletionView`, `HoverView`, `SignatureView`, `LocationChooserView`, `RenamePreviewView`,
  `CodeActionView`, `InlayHintsView`, `SymbolsView`, `FormattingFeedbackView`, and
  `LanguageServerView` hold the individual language overlays.
- `LanguageAction`, `LanguageEffectRequest`, `LanguageResult`, and `LanguageModel` provide the
  normalized action/effect/result boundary used by the UI layer.

Requests include completion resolve, prepare rename, declaration/implementation navigation, and
range formatting so those protocol capabilities remain reachable without direct process access.

Invariants and flow:

- All versioned results are compared against the active document version before being accepted.
- Problems groups by workspace and path, preserving stable ordering for deterministic tests.
- Rendering remains terminal-safe by using the shared virtual framebuffer and pre-tokenized glyph
  cells instead of direct process or terminal access.

Tests cover diagnostics, grouping, Unicode grapheme handling, stale suppression, and the major
overlay render paths used by the language UI.
# Contextual overlays

Language surfaces expose payload-preserving completion items plus anchored contextual overlay
placement for completion, hover, signature, Quick Fix, rename, navigation, references and symbols.
