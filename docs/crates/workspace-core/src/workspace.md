# Workspace root identity, trust, and recent-workspace persistence

Role: owns the persisted workspace model for trust state, workspace identities, roots, and recent
workspace ordering.

The module provides:

- `TrustState` for trusted versus untrusted workspace policy.
- `WorkspaceIdentity` and `WorkspaceRoot` for canonical, path-based workspace identity.
- `WorkspaceSet` for deduplicated root management keyed by canonical workspace paths.
- `TrustStore` for loading, saving, and querying persistent trust decisions.
- `RecentWorkspaceStore` for tracking recently opened workspaces in most-recent-first order.

Helper functions wrap the store operations so the rest of the crate can work with a compact API
surface.
