//! Shared workbench geometry facade.
//!
//! Geometry is currently implemented by the shell facade so rendering and hit testing remain
//! transactionally coupled. This module exposes the stable types for feature-owned consumers.
pub use super::{
    EditorGroupLayout, EditorPointerRegion, ExplorerRowLayout, PointerEvent, PointerTarget,
    WorkbenchLayoutSnapshot,
};
