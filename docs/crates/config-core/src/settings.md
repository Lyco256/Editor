# Settings

## Role and boundary

`settings.rs` defines the complete editor settings model, partial persistent/session layers,
keybindings, structured language-server and formatter commands, and logical theme data. It parses
JSONC without process execution and explicitly preserves unknown keys.

## Types and invariants

`SettingsStack` always applies defaults, user, workspace, active-folder, then session layers.
`EditorSettings` is fully resolved, while `SettingsLayer` uses optional fields so absence cannot
accidentally override a lower layer. Commands remain executable-plus-argument vectors rather than
shell strings. Unknown JSON members remain in flattened maps through explicit rewrites.

## Data flow and errors

JSONC comments and trailing commas are normalized before Serde parsing. `load_settings_or_default`
turns malformed or unreadable user files into an empty layer plus a surfaced `SettingsIssue`, so a
bad settings file cannot prevent startup. Serialization occurs only through an explicit caller
request and returns `SettingsError` on failure.

## Tests

Unit tests cover all five precedence levels, JSONC syntax, unknown-key round trips, malformed-file
fallback, and structured command arguments.
