# Requirements/001 pre-goal red-state proof

This evidence was collected on the unmodified production implementation before the Goal run.

## Acceptance command

```text
cargo test --test requirements_001 --no-fail-fast
```

Exit status: **101**.

The frozen oracle executed 78 cases: 70 passed and the following eight intentionally remained red:

- `R001_RESIZE_IMMEDIATE`: root input handling still has a silent resize branch (`InputEvent::Resize { .. } => Ok(())`).
- `R004_RENDER_HIT_SAME_LAYOUT`: the shell source still contains a fixed `120x40` geometry fixture/pattern.
- `K004_PREFERRED_COLUMN_EMPTY`: vertical navigation loses the preferred display column across an empty line.
- `K005_PREFERRED_COLUMN_SHORT`: vertical navigation does not restore the preferred column after a short line.
- `K006_PREFERRED_COLUMN_TABS`: tab-expanded display columns are not preserved across vertical movement.
- `M011_PRIMARY_CARET_PRESERVES_GLYPH`: the editor source still contains the forbidden source-glyph caret marker pattern.
- `U010_GROUP_LOCAL_TABS`: pane-local tab ownership is not represented by the root state.
- `P003_SECONDARY_PANE_ISOLATION`: the root state has no per-pane buffer accessor/isolation path.

These failures are the expected red-state signal for the known interaction defects; no production
files were edited while installing this oracle.

## Existing-test protection

Command:

```text
cargo test --workspace --all-features --lib
```

Exit status: **0**. All existing library/unit tests passed before the acceptance baseline was
frozen.
