# Syntax engine boundary

Role: identifies versioned parse work so stale syntax results cannot overwrite newer document state.
The engine consumes read-only editor snapshots and shared semantic roles. Wave 1 adds built-in grammar
parsers and remains disabled in large-file mode.

