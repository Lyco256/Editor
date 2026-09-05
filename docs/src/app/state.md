# Application state

Role: authoritative mutable application state and pure transitions. Workers never receive mutable
access. The trust invariant blocks external effects while untrusted and records a visible warning.
Control-Q and Control-C provide keyboard exits for the interactive loop. Unit tests cover blocked and
authorized paths plus clean input shutdown.
