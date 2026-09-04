//! Workspace, filesystem, search, and trust domain boundary.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrustState {
    Trusted,
    Untrusted,
}

impl TrustState {
    #[must_use]
    pub const fn allows_external_processes(self) -> bool {
        matches!(self, Self::Trusted)
    }
}
