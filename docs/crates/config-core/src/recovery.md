# Session and recovery persistence

## Role and boundary

`recovery.rs` serializes workspace roots, editor/tab/split state, cursors, selections, original file
metadata, dirty flags, and unsaved text into the operating system application-data tree. A store's
write directory is derived only from its application-data base and a validated single-component
application identifier—never from a workspace or document path.

## Types and invariants

`SessionState` is versioned and preserves additive unknown fields within the current format.
`EditorSession` retains enough metadata to restore both saved and untitled dirty buffers.
`RecoveryStore::save` writes JSON to a same-directory temporary file, flushes and synchronizes it,
then publishes a unique monotonically numbered generation with a no-clobber atomic rename.

## Data flow, concurrency, and errors

Recovery is synchronous at this domain boundary and is intended to run off the UI update path.
Unique generation publication prevents a partial write from replacing a valid record and tolerates
concurrent writers with bounded retries. Loading scans newest-to-oldest, returns the first valid
supported generation, and reports corrupt/truncated/incompatible files as `RecoveryWarning`; bad
recovery data therefore cannot block startup. Directory and write failures use `RecoveryError`.

## Tests

Tests simulate restart and repeated atomic publication, assert that unsaved text never appears at
the original workspace path, verify corrupt-newest fallback and all-corrupt safe failure, exercise
format-version compatibility, and reject path-traversing application identifiers.
