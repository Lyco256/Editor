# EditorConfig

## Role and boundary

`editorconfig.rs` discovers `.editorconfig` files from an active document's directory upward,
honors `root = true`, matches sections, and maps the required document formatting properties. It
only reads static files and does not invoke external commands.

## Types and invariants

`DocumentSettings` contains optional values for indentation style/size, tab width, LF/CRLF,
charset, trailing-whitespace trimming, and final-newline insertion. Files apply farthest-to-nearest;
within a file, later matching sections win. `unset` clears an inherited EditorConfig property.
Invalid known values are excluded and retained as source-located `EditorConfigWarning` entries.

## Data flow and errors

Paths are made relative to each configuration file for slash-containing globs; basename patterns
apply to the document name. I/O failures use `EditorConfigError`. Unknown properties are ignored as
required by EditorConfig extensibility and never affect persistent Editor settings.

## Tests

Tests cover directory traversal and root stopping, near-file precedence, all mandatory property
mappings, brace/path glob behavior, and diagnosable invalid values.
