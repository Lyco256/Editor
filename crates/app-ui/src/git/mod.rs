//! Source-control view boundary.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GitView {
    Changes,
    Diff,
    Branches,
    Stashes,
    History,
}
