//! Normalized user intent accepted by the root state machine.

use editor_types::{CommandId, InputEvent};

use super::effect::Effect;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    Input(InputEvent),
    Invoke(CommandId),
    SetWorkspaceTrust(bool),
    RequestEffect(Effect),
    Quit,
}
