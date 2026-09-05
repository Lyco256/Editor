# VS Code compatibility boundary

Role: converts useful VS Code static data into Editor-owned models without running extension code.
`ParsedSettings`, `ParsedKeybindings`, `StaticTheme`, `StaticSnippetFile`, `StaticLanguage`, and
`LanguageConfiguration` are the public data products. `ContributionKind` and
`CompatibilityWarning` make unsupported contributions diagnosable instead of silently ignored.

JSONC comments/trailing commas are stripped before typed parsing. Directory and VSIX loaders collect
only supported themes, snippets, languages, and language-configuration references. VSIX extraction
validates every archive path before creating output, rejects absolute/escaping entries, and reports
malformed metadata through `CompatibilityError`.

No function invokes a process or evaluates extension JavaScript/TypeScript. Tests cover JSONC,
keybindings, themes, snippets/tab stops, language rules, local extensions, malformed manifests, VSIX
round trips, and traversal rejection; the repository doc-mirror test covers this source path.
