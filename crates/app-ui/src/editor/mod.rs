//! Editor viewport view boundary.

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Viewport {
    pub top_line: u32,
    pub left_column: u32,
}
