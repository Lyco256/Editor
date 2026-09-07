# 075 — final verifier and forbidden path guard

Owner: top Codex

Writable paths:
- verifier support outside frozen paths only if no semantic weakening occurs
- docs/testing evidence

## Objective

Run and independently inspect the final mechanical gate; ensure old known-bad code paths cannot remain hidden.

## Required implementation

The frozen verifier must enforce at least these source guards in production paths:

- no shell hit-test `Rect::new(0, 0, 120, 40)` assumption;
- no `InputEvent::Resize { .. } => Ok(())` ordinary application handling;
- no Up/Down `move_vertical(..., 4)` hard-coded effective width;
- no forward-delete `CharacterOffset(range.end.0 + 1)` root implementation;
- no fixed split `column >= 60` / `row >= 20`;
- no fixed Explorer screen boundary used by root pointer routing;
- no fixed tab index by dividing raw screen column;
- no `line_text.find(&result.matched_text)` marker reconstruction;
- no ordinary changed-file marker spanning `0..buffer.len_chars()`;
- no source-overwriting `cell("▌", ...)` caret;
- no direct contextual overlay global x/y from `LogicalPosition.character/line`;
- no forbidden decorative chrome glyph constants.

Run platform verifier and save full log under `docs/testing/requirements-001-verify.log`.

Confirm all reviewer reports contain zero blockers.

## Required tests

Verifier itself plus frozen `Q003`–`Q006`.

## Done only when

platform verifier exits 0, all case IDs PASS, frozen diff is empty, and all known-bad source patterns are absent.
