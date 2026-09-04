//! Configuration, encoding, and recovery domain boundary.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LargeFileSettings {
    pub threshold_bytes: u64,
}

impl Default for LargeFileSettings {
    fn default() -> Self {
        Self {
            threshold_bytes: 32 * 1024 * 1024,
        }
    }
}
