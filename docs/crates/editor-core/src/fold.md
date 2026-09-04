# Folding boundary

Role: models foldable line regions and their collapsed state.

Important types:

- `FoldRegion` stores an inclusive start and end line plus whether the region is collapsed.
- `FoldSet` stores ordered regions and exposes toggle and visibility helpers.

Invariants:

- A fold region must span at least two logical lines.
- Collapsed regions hide the lines strictly inside the region while leaving the start line visible.
- Regions are sorted and deduplicated by their line span.

Data flow:

- Higher layers provide parser or language-server fold ranges.
- The fold set normalizes those ranges and then answers visibility queries for rendering and
  navigation.

Concurrency and error behavior:

- The module is pure state management with typed validation errors.

Tests:

- Module tests cover region validation, ordering, toggling, hidden-line checks, and unfolding.
