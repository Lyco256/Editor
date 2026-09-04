# Filesystem services

Role: owns lazy explorer traversal, quick-open indexing, create/rename/move/delete planning and
execution, and the file-change tracker that turns external edits into reload or conflict events.

Important types:

- `ExplorerTree` and `ExplorerEntry` represent workspace tree discovery.
- `QuickOpenIndex` and `QuickOpenCandidate` hold file-name discovery data.
- `FileChangeTracker` watches open documents and classifies on-disk changes.
- `DeletePlan`, `RenamePlan`, `MovePlan`, and `FileOperationPlan` expose confirmation metadata.

Invariants:

- Symlink traversal never follows links recursively.
- Explorer and quick-open visibility respect `.gitignore`-style rules plus configured excludes.
- Case-only rename handling is separated out so Windows can take the safer path.
- File-change scanning compares a disk fingerprint against the tracked snapshot before deciding
  reload versus conflict.

Dependencies:

- `ignore` for discovery and search-side filtering.
- `document.rs` for text decoding and save-on-replace support.
- `path.rs` for identity and case-sensitive/case-insensitive comparisons.

Tests:

- quick-open ranking.
- rename plan case-only behavior.
- symlink-loop protection.
- external reload/conflict classification.
