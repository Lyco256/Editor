# Application state

Role: authoritative mutable application state and pure transitions. Workers never receive mutable
access. The trust invariant blocks external effects while untrusted and records a visible warning.
Unit tests cover both blocked and authorized paths.

