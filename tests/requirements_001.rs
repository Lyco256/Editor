//! Frozen Requirements/001 acceptance oracle entry point.
#![allow(clippy::module_inception)]

#[path = "requirements_001/navigation.rs"]
mod navigation;
#[path = "requirements_001/pointer_caret.rs"]
mod pointer_caret;
#[path = "requirements_001/projection.rs"]
mod projection;
#[path = "requirements_001/resize_geometry.rs"]
mod resize_geometry;
#[path = "requirements_001/robustness.rs"]
mod robustness;
#[path = "requirements_001/support/mod.rs"]
mod support;
#[path = "requirements_001/workbench.rs"]
mod workbench;
