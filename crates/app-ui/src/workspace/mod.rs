//! Workspace view boundary.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkspaceView {
    Explorer,
    QuickOpen,
    Search,
    Trust,
}
