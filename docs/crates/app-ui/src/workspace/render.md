# Workspace rendering

Role: turns workspace UI state into framebuffer cells for Explorer, Quick Open, search, and trust views.

Important types: `framebuffer_lines`, `draw_workspace`, `draw_quick_open`, and `draw_search`.

Invariants: rendering is deterministic, width-bounded, and side-effect free. It only reads in-memory state
and writes to the provided framebuffer.

Data flow: the view layer clears the target frame, paints a header, then renders the active workspace panel
and any pending confirmation prompt or search preview.

Concurrency assumptions: callers own the framebuffer for the duration of the draw call.

Error behavior: drawing intentionally swallows frame-set errors because the code only writes inside the
frame bounds it checked from the framebuffer size.

Dependencies: `editor-types` for styles and ranges, `terminal-backend` for framebuffer access, and the
workspace model types for tree, quick-open, and search state.

Tests: the workspace UI tests compare rendered framebuffer text against snapshots for all required states.
