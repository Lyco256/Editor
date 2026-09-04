//! Application shell view boundary.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShellRegion {
    Explorer,
    Editor,
    BottomPanel,
    StatusBar,
}
