# Workspace Trust boundary

Trust is keyed by canonical workspace identity. Untrusted workspaces may be viewed and edited, but the
root state machine blocks LSP, external formatter, and Git effects before a process adapter receives
them. Static JSON/JSONC parsing does not cross this boundary.

