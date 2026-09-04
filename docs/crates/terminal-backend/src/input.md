# Input normalization

Role: converts crossterm input into protocol-neutral editor input events.

Important types:

- `InputReader` defines the blocking and polling input boundary.
- `normalize_event` maps crossterm events into `InputEvent`.
- `normalize_key` and `normalize_mouse` normalize the concrete key and mouse shapes.

Invariants:

- Key release reports are filtered out.
- Unsupported legacy keys and mouse wheel directions that the editor does not consume are ignored.
- Scroll wheel motion is normalized to line-based vertical scroll events.
- Resize and paste events pass through unchanged.

Data flow:

- The terminal backend reads crossterm events from the host terminal.
- Normalized events move upward to the application event system without exposing crossterm types.
- Modifier flags are normalized to the shared `editor-types` bitset.

Error behavior:

- The pure normalization functions do not return runtime errors; they return `None` for unsupported
  events.
- The `InputReader` trait uses typed adapter errors for host I/O failures.

Dependencies:

- `crossterm` for terminal event types.
- `editor-types` for shared input types.

Tests:

- Key normalization preserves modifiers and repeat state.
- Key release events are suppressed.
- Mouse normalization preserves position and vertical scroll direction.
- Resize events map to the shared resize input event.
