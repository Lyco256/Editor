# Document load/save adapter

Role: handles byte loading, encoding detection/conversion, line-ending inspection, and atomic save
for text documents. It is the document-facing boundary of the workspace crate.

Important types:

- `EncodingKind`, `DecodePolicy`, and `LineEndings` describe the text codec and end-of-line policy.
- `DecodedText` carries decoded text plus the metadata needed for faithful save.
- `DocumentLoadOptions` and `DocumentSaveOptions` keep the open/save behavior explicit.
- `TextDocument` is the loaded document snapshot the rest of the crate uses.
- `decode_text_document_with_encoding` and `load_text_document_with_encoding` provide an explicit,
  BOM-aware codec path for user-requested reopen/conversion operations.

Invariants:

- BOM detection happens before UTF-8 validation and heuristic legacy detection.
- Save normalizes line endings only when the caller asks for a target ending.
- Atomic save writes to a temporary file first and then persists it to the final path.

Dependencies:

- `config-core::LargeFileSettings` for the large-file boundary.
- `encoding_rs`, `chardetng`, and `tempfile` for codec and save support.

Tests:

- UTF-8, UTF-16LE, and UTF-16BE round trips.
- legacy encoding rejection on unmappable text.
- atomic save failure safety.
