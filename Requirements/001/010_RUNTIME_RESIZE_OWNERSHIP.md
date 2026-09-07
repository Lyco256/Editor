# 010 — runtime resize ownership

Owner: top Codex

Writable paths:
- `src/app/runtime.rs`
- its mirrored docs
- non-frozen runtime tests

## Objective

Make the runtime own physical terminal dimensions and apply Resize before the next frame, matching Microsoft Edit's resize-state behavior.

## Required implementation

1. Detect `Action::Input(InputEvent::Resize {{ columns, rows }})` in `AppRuntime::run_entered` before passing ordinary input to `AppState`.
2. If either dimension is zero, do not construct/present a zero-sized frame; retain the last non-zero size until the next non-zero Resize.
3. For non-zero Resize, assign `self.size = (columns, rows)` immediately.
4. Mark render required even if the application-state transition would otherwise say no render.
5. Do not forward Resize into `AppState::apply_input` as a successful editor no-op. Remove the current `InputEvent::Resize {{ .. }} => Ok(())` application branch or make it unreachable with a debug/assert test seam.
6. The frame after Resize must use the new dimensions.
7. Do not debounce Resize in this cycle; every normalized non-zero Resize updates state in arrival order.

## Required tests

- frozen `R001_RESIZE_IMMEDIATE`
- frozen `R002_RESIZE_SEQUENCE`
- add unit test for zero-sized Resize followed by valid Resize
- add test proving no extra keypress is needed before the resized frame

## Done only when

runtime frame size follows the latest non-zero Resize immediately and the old silent Resize path is absent.
