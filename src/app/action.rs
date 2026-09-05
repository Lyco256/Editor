//! Normalized user intent accepted by the root state machine.

use editor_types::{CommandId, InputEvent};
use std::path::PathBuf;

use super::effect::Effect;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    Input(InputEvent),
    Invoke(CommandId),
    SetWorkspaceTrust(bool),
    OpenPath(PathBuf),
    SwitchTab(usize),
    SplitPane {
        axis: app_ui::shell::SplitAxis,
        ratio_percent: u16,
    },
    CloseSplit,
    SetSplitRatio(u16),
    AddWorkspaceRoot(PathBuf),
    ApplyReplacementPlan(workspace_core::ReplacementPlan),
    RequestEffect(Effect),
    Quit,
}
