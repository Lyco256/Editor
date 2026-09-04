# Architecture overview

Editor uses a unidirectional event loop with ports/adapters boundaries. Input becomes a normalized
action; the root state transition emits typed effects; background services return events; pure views
render the resulting state to an application-owned framebuffer. Only the root runtime wires services.

Crates form an acyclic dependency graph rooted at `editor-types`. External I/O is isolated in service
adapters, and Workspace Trust is enforced before any process-backed effect is dispatched.

