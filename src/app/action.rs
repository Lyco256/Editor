//! Normalized user intent accepted by the root state machine.

use editor_types::{CharacterOffset, CommandId, InputEvent};
use std::path::PathBuf;

use super::effect::Effect;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    Input(InputEvent),
    Invoke(CommandId),
    /// Typed language-panel action routed through root state.
    Language(app_ui::language::LanguageAction),
    /// Typed source-control action routed through root state.
    Git(app_ui::git::GitAction),
    SetWorkspaceTrust(bool),
    OpenPath(PathBuf),
    NewFile,
    OpenFolder(PathBuf),
    Save,
    CloseActive,
    CloseOtherTabs,
    NextTab,
    PreviousTab,
    QuickOpenRecent,
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
    /// Editor viewport/fold and pane routing actions remain root-owned so keyboard and mouse
    /// dispatch share one transition path.
    ToggleFold {
        line: u32,
    },
    UnfoldContaining {
        line: u32,
    },
    UnfoldAll,
    FocusPane(u32),
    SetViewport {
        top_line: u32,
        left_column: u16,
    },
    AddCursorAbove,
    AddCursorBelow,
    AddCursorAt(CharacterOffset),
    RemoveLastCursor,
    CollapseCursors,
    SelectNextOccurrence {
        query: String,
        skip: bool,
    },
    SelectAllOccurrences {
        query: String,
    },
    AddWorkspaceRoot(PathBuf),
    ApplyReplacementPlan(workspace_core::ReplacementPlan),
    /// Starts a create/rename/move/delete confirmation flow for a planned filesystem operation.
    RequestFileOperation(workspace_core::FileOperationPlan),
    /// Confirms the currently displayed filesystem operation prompt.
    ConfirmFileOperation,
    /// Cancels the currently displayed filesystem operation prompt.
    CancelFileOperation,
    QuickOpen(app_ui::workspace::QuickOpenAction),
    StartSearch {
        query: String,
        options: app_ui::workspace::SearchOptionsView,
    },
    /// Finds all matches in the active document using editor-core semantics.
    FindInDocument {
        query: String,
        options: editor_core::FindOptions,
    },
    /// Replaces all matches in the active document as one undoable transaction.
    ReplaceInDocument {
        query: String,
        replacement: String,
        options: editor_core::FindOptions,
    },
    CancelSearch(u64),
    RequestEffect(Effect),
    Quit,
}
