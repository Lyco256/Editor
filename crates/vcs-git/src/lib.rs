//! Structured Git process adapter boundary.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiffTarget {
    WorkingTree,
    Index,
}
