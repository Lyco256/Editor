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
    AddWorkspaceRoot(PathBuf),
    RequestEffect(Effect),
    Quit,
}
