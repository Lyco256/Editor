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
    /// Reopens the active file with a user-selected codec after clean-buffer confirmation.
    ReopenWithEncoding(workspace_core::EncodingKind),
    /// Changes the codec used by the next save and marks the document dirty.
    SetEncoding {
        encoding: workspace_core::EncodingKind,
        with_bom: bool,
    },
    SwitchTab(usize),
    CloseTab(usize),
    SaveAs(PathBuf),
    SplitPane {
        axis: app_ui::shell::SplitAxis,
        ratio_percent: u16,
    },
    CloseSplit,
    SetSplitRatio(u16),
    AddWorkspaceRoot(PathBuf),
    ApplyReplacementPlan(workspace_core::ReplacementPlan),
    QuickOpen(app_ui::workspace::QuickOpenAction),
    StartSearch {
        query: String,
        options: app_ui::workspace::SearchOptionsView,
    },
    CancelSearch(u64),
    RequestEffect(Effect),
    Quit,
}
