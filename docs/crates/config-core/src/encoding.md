# Encoding and line endings

## Role and boundary

`encoding.rs` detects and converts document bytes without changing the source file. It supports
all labels resolved by `encoding_rs` plus explicit UTF-16 little- and big-endian conversion.

## Types and invariants

`EncodingKind` stores canonical labels, and `DecodedText` retains encoding, BOM, replacement, and
line-ending metadata needed for a preserving save. Detection order is BOM, valid UTF-8, confident
`chardetng` result, then configured fallback. `DecodePolicy::Strict` prevents silent lossy reads or
writes; `Replace` makes replacement explicit in the result.

`inspect_line_endings` distinguishes LF, CRLF, mixed, and no-line-ending content. Encoding unchanged
text preserves its exact terminators; normalization occurs only through an explicit request.

## Errors and dependencies

Unsupported labels, malformed input, and unmappable output return `EncodingError`. Conversion is
deterministic and has no filesystem or concurrency behavior. `chardetng` supplies legacy detection,
and `encoding_rs` supplies canonical web-compatible encodings and aliases.

## Tests

Tests verify detection priority, UTF-8/UTF-16 BOMs, Japanese Shift_JIS and Windows-1252 round trips,
aliases, strict/replacement handling, and mixed-EOL detection and normalization.
