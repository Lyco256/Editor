//! Authoritative root state and pure transition logic.
#![allow(clippy::struct_excessive_bools)]

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use app_ui::widgets::{CommandEntry, CommandPaletteState};
use editor_core::{PairConfig, TextBuffer};
use editor_types::{
    CharacterOffset, GitStatusSummary, InputEvent, KeyCode, LanguageServerStatus, LogicalPosition,
    Modifier, MouseAction, MouseButton, OutputLevel, OutputMessage, RequestId,
};

use super::{action::Action, effect::Effect, event::Event};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppState {
    pub running: bool,
    pub workspace_trusted: bool,
    pub frame_number: u64,
    pub language_server: LanguageServerStatus,
    pub git_status: Option<GitStatusSummary>,
    pub output: Vec<OutputMessage>,
    pub active_path: Option<PathBuf>,
    pub workspace_roots: Vec<PathBuf>,
    pub explorer_entries: Vec<ExplorerProjection>,
    pub active_text: String,
    pub active_dirty: bool,
    pub explorer_visible: bool,
    pub bottom_panel_visible: bool,
    pub palette_visible: bool,
    pub(crate) palette: CommandPaletteState,
    pub(crate) tabs: Vec<TabState>,
    pub(crate) active_tab: usize,
    pending_processes: HashMap<RequestId, super::effect::ExternalProcessKind>,
    latest_explorer_request: Option<RequestId>,
    mouse_anchor: Option<CharacterOffset>,
    /// Persistent editor buffer backing the active document. The public text fields remain a
    /// lightweight projection for views and recovery serialization.
    pub(crate) buffer: TextBuffer,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExplorerProjection {
    pub depth: u8,
    pub label: String,
    pub active: bool,
    pub expanded: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TabState {
    pub path: Option<PathBuf>,
    pub buffer: TextBuffer,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            running: true,
            workspace_trusted: false,
            frame_number: 0,
            language_server: LanguageServerStatus::Stopped,
            git_status: None,
            output: Vec::new(),
            active_path: None,
            workspace_roots: Vec::new(),
            explorer_entries: Vec::new(),
            active_text: String::new(),
            active_dirty: false,
            explorer_visible: true,
            bottom_panel_visible: false,
            palette_visible: false,
            palette: CommandPaletteState::new(vec![
                CommandEntry::available("editor.save", "Save"),
                CommandEntry::available("editor.undo", "Undo"),
                CommandEntry::available("editor.redo", "Redo"),
                CommandEntry::available("editor.quit", "Quit"),
                CommandEntry::available("git.refresh", "Refresh Git Status"),
            ]),
            tabs: vec![TabState {
                path: None,
                buffer: TextBuffer::default(),
            }],
            active_tab: 0,
            pending_processes: HashMap::new(),
            latest_explorer_request: None,
            mouse_anchor: None,
            buffer: TextBuffer::default(),
        }
    }
}

impl AppState {
    /// Applies an editor-view action through the same root transition path used by terminal input.
    pub fn apply_editor_action(&mut self, action: app_ui::editor::EditorAction) -> Transition {
        use app_ui::editor::EditorAction;
        match action {
            EditorAction::MoveCursor {
                line_delta,
                column_delta,
                extend_selection,
            } => {
                let (code, count) = if line_delta != 0 {
                    (
                        if line_delta.is_negative() {
                            KeyCode::Up
                        } else {
                            KeyCode::Down
                        },
                        line_delta.unsigned_abs(),
                    )
                } else {
                    (
                        if column_delta.is_negative() {
                            KeyCode::Left
                        } else {
                            KeyCode::Right
                        },
                        column_delta.unsigned_abs(),
                    )
                };
                let modifiers = if extend_selection {
                    editor_types::Modifiers::from_modifiers([Modifier::Shift])
                } else {
                    editor_types::Modifiers::default()
                };
                for _ in 0..count {
                    self.apply_input(InputEvent::Key(editor_types::KeyEvent {
                        code: code.clone(),
                        modifiers,
                        repeat: false,
                    }));
                }
                Transition {
                    render: true,
                    ..Transition::default()
                }
            }
            EditorAction::SelectRange(range) => {
                let _ = self
                    .buffer
                    .set_selections(editor_core::SelectionSet::single(
                        editor_core::Selection::new(range.start, range.end),
                    ));
                self.sync_buffer_projection();
                Transition {
                    render: true,
                    ..Transition::default()
                }
            }
            EditorAction::ToggleFold { .. } | EditorAction::Scroll { .. } => Transition {
                render: true,
                ..Transition::default()
            },
        }
    }

    /// Routes language-panel actions to root effects. Language servers are discovered only after
    /// an explicit trust decision; unavailable servers become a visible non-blocking status.
    #[allow(clippy::needless_pass_by_value)]
    pub fn apply_language_action(
        &mut self,
        action: app_ui::language::LanguageAction,
    ) -> Transition {
        if matches!(
            action,
            app_ui::language::LanguageAction::RestartLanguageServer
        ) {
            if let Some(path) = self.active_path.as_ref().filter(|path| path.is_file()) {
                if let Some(language) = syntax_engine::SyntaxLanguage::from_path(path) {
                    if let Some(command) = lsp_client::CommandSpec::discover_known(language.name())
                    {
                        return self.apply_action(Action::RequestEffect(Effect::ExternalProcess {
                            request: RequestId(self.frame_number.saturating_add(1)),
                            kind: super::effect::ExternalProcessKind::LanguageServer,
                            spec: super::effect::ProcessSpec {
                                executable: command.executable.to_string_lossy().into_owned(),
                                arguments: command.args,
                            },
                        }));
                    }
                }
            }
            self.language_server = LanguageServerStatus::Unavailable;
        }
        Transition {
            render: true,
            ..Transition::default()
        }
    }

    /// Converts Git UI intents into structured, trust-gated process effects. Destructive actions
    /// that require a confirmation remain visible but are not executed by this adapter.
    pub fn apply_git_action(&mut self, action: app_ui::git::GitAction) -> Transition {
        use app_ui::git::GitAction;
        let root = self.workspace_roots.first().cloned();
        let args = match action {
            GitAction::StageFile(path) => Some(vec![
                "add".to_owned(),
                "--".to_owned(),
                path.display().to_string(),
            ]),
            GitAction::UnstageFile(path) => Some(vec![
                "restore".to_owned(),
                "--staged".to_owned(),
                "--".to_owned(),
                path.display().to_string(),
            ]),
            GitAction::Fetch => Some(vec!["fetch".to_owned()]),
            GitAction::Pull => Some(vec!["pull".to_owned()]),
            GitAction::Push => Some(vec!["push".to_owned()]),
            GitAction::CreateBranch(name) => Some(vec!["switch".to_owned(), "-c".to_owned(), name]),
            GitAction::SwitchBranch(name) => Some(vec!["switch".to_owned(), name]),
            GitAction::DeleteBranch { name, force } => Some(vec![
                "branch".to_owned(),
                if force { "-D" } else { "-d" }.to_owned(),
                name,
            ]),
            GitAction::CreateStash => Some(vec!["stash".to_owned(), "push".to_owned()]),
            GitAction::ApplyStash(reference) => {
                Some(vec!["stash".to_owned(), "apply".to_owned(), reference])
            }
            GitAction::PopStash(reference) => {
                Some(vec!["stash".to_owned(), "pop".to_owned(), reference])
            }
            GitAction::Commit | GitAction::ConfirmDiscard => {
                self.output.push(OutputMessage {
                    subsystem: "git".to_owned(),
                    operation: "confirmation".to_owned(),
                    level: OutputLevel::Warning,
                    message: "this Git action requires a complete confirmation form".to_owned(),
                });
                None
            }
            GitAction::RequestDiscard(_)
            | GitAction::CancelDiscard
            | GitAction::UnstageHunk { .. }
            | GitAction::StageHunk { .. }
            | GitAction::SwitchView(_)
            | GitAction::ToggleChangeMode
            | GitAction::SelectChange(_)
            | GitAction::SelectHunk(_)
            | GitAction::EditCommitMessage(_)
            | GitAction::ToggleAmend
            | GitAction::ToggleAllowEmpty
            | GitAction::OpenDiffFile(_)
            | GitAction::OpenConflict(_) => None,
        };
        let Some((root, args)) = root.zip(args) else {
            return Transition {
                render: true,
                ..Transition::default()
            };
        };
        self.apply_action(Action::RequestEffect(Effect::ExternalProcess {
            request: RequestId(self.frame_number.saturating_add(1)),
            kind: super::effect::ExternalProcessKind::Git,
            spec: super::effect::ProcessSpec {
                executable: "git".to_owned(),
                arguments: {
                    let mut all = vec!["-C".to_owned(), root.display().to_string()];
                    all.extend(args);
                    all
                },
            },
        }))
    }

    /// Serializes all open documents into the crash-safe session schema. The caller owns the
    /// `RecoveryStore` and decides when to persist this snapshot (normally after each committed
    /// action and once more during shutdown).
    #[must_use]
    pub fn session_state(&self) -> config_core::SessionState {
        let editors = self
            .tabs
            .iter()
            .enumerate()
            .map(|(index, tab)| {
                let selection = tab
                    .buffer
                    .selections()
                    .selections()
                    .iter()
                    .map(|item| {
                        let anchor = tab
                            .buffer
                            .offset_to_position(item.anchor)
                            .unwrap_or_default();
                        let active = tab
                            .buffer
                            .offset_to_position(item.active)
                            .unwrap_or_default();
                        config_core::SelectionState {
                            anchor: config_core::CursorState {
                                line: anchor.line,
                                character: anchor.character,
                            },
                            active: config_core::CursorState {
                                line: active.line,
                                character: active.character,
                            },
                        }
                    })
                    .collect::<Vec<_>>();
                let primary = tab.buffer.selections().primary();
                let cursor = tab
                    .buffer
                    .offset_to_position(primary.active)
                    .unwrap_or_default();
                config_core::EditorSession {
                    editor_id: format!("tab-{index}"),
                    original_path: tab.path.clone(),
                    original_encoding: None,
                    original_line_ending: None,
                    cursor: config_core::CursorState {
                        line: cursor.line,
                        character: cursor.character,
                    },
                    selections: selection,
                    unsaved_text: tab.buffer.is_dirty().then(|| tab.buffer.to_string()),
                    dirty: tab.buffer.is_dirty(),
                    unknown: std::collections::BTreeMap::new(),
                }
            })
            .collect::<Vec<_>>();
        let tab_order = editors
            .iter()
            .map(|editor| editor.editor_id.clone())
            .collect::<Vec<_>>();
        config_core::SessionState {
            workspace_roots: self.workspace_roots.clone(),
            editors,
            tab_order,
            split_layout: config_core::SplitLayout::Editor {
                editor_id: format!("tab-{}", self.active_tab),
            },
            active_editor: Some(format!("tab-{}", self.active_tab)),
            ..config_core::SessionState::default()
        }
    }

    /// Restores every editor tab from a previously persisted session. Missing files are skipped
    /// safely while unsaved text remains available from the recovery record.
    pub fn restore_session(&mut self, session: &config_core::SessionState) {
        self.workspace_roots.clone_from(&session.workspace_roots);
        if session.editors.is_empty() {
            return;
        }
        self.tabs.clear();
        for editor in &session.editors {
            let (path, disk_text) = if let Some(path) = &editor.original_path {
                let Ok(document) = workspace_core::load_text_document(
                    path,
                    &workspace_core::DocumentLoadOptions::default(),
                ) else {
                    continue;
                };
                (Some(path.clone()), document.text)
            } else {
                (None, String::new())
            };
            let text = editor.unsaved_text.as_deref().unwrap_or(&disk_text);
            let mut buffer = TextBuffer::new(text);
            if editor.dirty {
                buffer.mark_recovered_dirty();
            }
            if let Ok(position) = buffer.position_to_offset(LogicalPosition {
                line: editor.cursor.line,
                character: editor.cursor.character,
            }) {
                let _ = buffer.set_selections(editor_core::SelectionSet::single(
                    editor_core::Selection::cursor(position),
                ));
            }
            self.tabs.push(TabState { path, buffer });
        }
        if self.tabs.is_empty() {
            return;
        }
        self.active_tab = session
            .active_editor
            .as_ref()
            .and_then(|active| {
                session
                    .editors
                    .iter()
                    .position(|editor| &editor.editor_id == active)
            })
            .unwrap_or(0)
            .min(self.tabs.len().saturating_sub(1));
        let active = self.tabs[self.active_tab].clone();
        self.active_path = active.path;
        self.buffer = active.buffer;
        self.sync_buffer_projection();
        self.refresh_explorer_entries();
    }

    /// Loads a startup path into the view model without launching external processes.
    ///
    /// Directories become workspace roots; regular files are decoded using the workspace document
    /// adapter and retained as editable text. Missing or malformed files are reported as structured
    /// output while the editor remains usable.
    pub fn open_startup_path(&mut self, path: impl AsRef<Path>) {
        let path = path.as_ref().to_path_buf();
        if path.is_dir() {
            self.active_path = Some(path);
            self.tabs = vec![TabState {
                path: None,
                buffer: TextBuffer::default(),
            }];
            self.active_tab = 0;
            self.workspace_roots = self.active_path.iter().cloned().collect();
            self.refresh_explorer_entries();
            self.active_text.clear();
            self.active_dirty = false;
            self.buffer = TextBuffer::default();
            return;
        }
        match workspace_core::load_text_document(
            &path,
            &workspace_core::DocumentLoadOptions::default(),
        ) {
            Ok(document) => {
                self.active_path = Some(document.path);
                self.tabs = vec![TabState {
                    path: self.active_path.clone(),
                    buffer: TextBuffer::new(&document.text),
                }];
                self.active_tab = 0;
                self.workspace_roots = self
                    .active_path
                    .as_ref()
                    .and_then(|path| path.parent())
                    .map(|path| vec![path.to_path_buf()])
                    .unwrap_or_default();
                self.refresh_explorer_entries();
                self.active_text = document.text;
                self.active_dirty = false;
                self.buffer = TextBuffer::new(&self.active_text);
            }
            Err(error) => self.output.push(OutputMessage {
                subsystem: "workspace".to_owned(),
                operation: "open-file".to_owned(),
                level: OutputLevel::Error,
                message: format!("could not open {}: {error}", path.display()),
            }),
        }
    }

    /// Opens a regular file as a new tab while preserving the current tab snapshot.
    ///
    /// # Errors
    ///
    /// Returns the typed workspace document error when the file cannot be decoded.
    pub fn open_tab(
        &mut self,
        path: impl AsRef<Path>,
    ) -> Result<(), workspace_core::DocumentOpenError> {
        let path = path.as_ref().to_path_buf();
        let document = workspace_core::load_text_document(
            &path,
            &workspace_core::DocumentLoadOptions::default(),
        )?;
        self.sync_active_tab();
        self.tabs.push(TabState {
            path: Some(document.path.clone()),
            buffer: TextBuffer::new(&document.text),
        });
        self.active_tab = self.tabs.len().saturating_sub(1);
        self.active_path = Some(document.path);
        self.active_text = document.text;
        self.active_dirty = false;
        self.buffer = self.tabs[self.active_tab].buffer.clone();
        self.refresh_explorer_entries();
        Ok(())
    }

    #[must_use]
    pub fn tab_count(&self) -> usize {
        self.tabs.len()
    }

    #[must_use]
    pub fn active_tab_index(&self) -> usize {
        self.active_tab
    }

    fn switch_tab(&mut self, index: usize) {
        if index >= self.tabs.len() || index == self.active_tab {
            return;
        }
        self.sync_active_tab();
        self.active_tab = index;
        let tab = self.tabs[index].clone();
        self.active_path = tab.path;
        self.buffer = tab.buffer;
        self.sync_buffer_projection();
    }

    fn sync_active_tab(&mut self) {
        if let Some(tab) = self.tabs.get_mut(self.active_tab) {
            tab.path.clone_from(&self.active_path);
            tab.buffer = self.buffer.clone();
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Transition {
    pub effects: Vec<Effect>,
    pub events: Vec<Event>,
    pub render: bool,
}

impl AppState {
    #[must_use]
    #[allow(clippy::too_many_lines)]
    pub fn apply_action(&mut self, action: Action) -> Transition {
        match action {
            Action::Quit => {
                if self.active_dirty {
                    self.output.push(OutputMessage {
                        subsystem: "editor".to_owned(),
                        operation: "quit".to_owned(),
                        level: OutputLevel::Warning,
                        message: "unsaved changes prevent quitting; save or discard them first"
                            .to_owned(),
                    });
                } else {
                    self.running = false;
                }
                Transition {
                    render: true,
                    ..Transition::default()
                }
            }
            Action::SetWorkspaceTrust(trusted) => {
                self.workspace_trusted = trusted;
                if trusted {
                    if let Some(path) = self.active_path.as_ref().filter(|path| path.is_file()) {
                        if let Some(language) = syntax_engine::SyntaxLanguage::from_path(path) {
                            if let Some(command) =
                                lsp_client::CommandSpec::discover_known(language.name())
                            {
                                let request = RequestId(self.frame_number.saturating_add(1));
                                self.pending_processes.insert(
                                    request,
                                    super::effect::ExternalProcessKind::LanguageServer,
                                );
                                self.language_server = LanguageServerStatus::Starting;
                                return Transition {
                                    effects: vec![Effect::ExternalProcess {
                                        request,
                                        kind: super::effect::ExternalProcessKind::LanguageServer,
                                        spec: super::effect::ProcessSpec {
                                            executable: command
                                                .executable
                                                .to_string_lossy()
                                                .into_owned(),
                                            arguments: command.args,
                                        },
                                    }],
                                    render: true,
                                    ..Transition::default()
                                };
                            }
                        }
                        self.language_server = LanguageServerStatus::Unavailable;
                    }
                }
                Transition {
                    render: true,
                    ..Transition::default()
                }
            }
            Action::OpenPath(path) => {
                if self.active_path.is_some() && path.is_file() {
                    if let Err(error) = self.open_tab(&path) {
                        self.output.push(OutputMessage {
                            subsystem: "workspace".to_owned(),
                            operation: "open-tab".to_owned(),
                            level: OutputLevel::Error,
                            message: error.to_string(),
                        });
                    }
                } else {
                    self.open_startup_path(path);
                }
                self.explorer_refresh_transition()
            }
            Action::SwitchTab(index) => {
                self.switch_tab(index);
                Transition {
                    render: true,
                    ..Transition::default()
                }
            }
            Action::AddWorkspaceRoot(path) => {
                if path.is_dir() {
                    if !self.workspace_roots.iter().any(|root| root == &path) {
                        self.workspace_roots.push(path);
                        return self.explorer_refresh_transition();
                    }
                } else {
                    self.output.push(OutputMessage {
                        subsystem: "workspace".to_owned(),
                        operation: "add-root".to_owned(),
                        level: OutputLevel::Error,
                        message: format!("workspace root is not a directory: {}", path.display()),
                    });
                }
                Transition {
                    render: true,
                    ..Transition::default()
                }
            }
            Action::RequestEffect(effect)
                if effect.requires_trusted_workspace() && !self.workspace_trusted =>
            {
                let (request, kind) = match &effect {
                    Effect::ExternalProcess { request, kind, .. } => (*request, *kind),
                    Effect::RefreshGitStatus { request, .. } => {
                        (*request, super::effect::ExternalProcessKind::Git)
                    }
                    Effect::SaveDocument { .. }
                    | Effect::RefreshExplorer { .. }
                    | Effect::Render => {
                        unreachable!("unguarded effect")
                    }
                };
                Transition {
                    events: vec![Event::ExternalProcessBlocked { request, kind }],
                    render: true,
                    ..Transition::default()
                }
            }
            Action::RequestEffect(effect) => {
                match &effect {
                    Effect::ExternalProcess { request, kind, .. } => {
                        self.pending_processes.insert(*request, *kind);
                    }
                    Effect::RefreshGitStatus { request, .. } => {
                        self.pending_processes
                            .insert(*request, super::effect::ExternalProcessKind::Git);
                    }
                    Effect::SaveDocument { .. }
                    | Effect::RefreshExplorer { .. }
                    | Effect::Render => {}
                }
                Transition {
                    effects: vec![effect],
                    ..Transition::default()
                }
            }
            Action::Input(InputEvent::Key(key))
                if matches!(key.code, KeyCode::Character('c' | 'q'))
                    && key.modifiers.contains(Modifier::Control) =>
            {
                if self.active_dirty {
                    self.output.push(OutputMessage {
                        subsystem: "editor".to_owned(),
                        operation: "quit".to_owned(),
                        level: OutputLevel::Warning,
                        message: "unsaved changes prevent quitting; save or discard them first"
                            .to_owned(),
                    });
                } else {
                    self.running = false;
                }
                Transition {
                    render: true,
                    ..Transition::default()
                }
            }
            Action::Input(input) => {
                self.apply_input(input);
                Transition {
                    render: true,
                    ..Transition::default()
                }
            }
            Action::Invoke(command) => {
                let effect = self.apply_command(command.as_str());
                Transition {
                    effects: effect.into_iter().collect(),
                    render: true,
                    ..Transition::default()
                }
            }
        }
    }

    #[allow(clippy::too_many_lines)]
    fn apply_input(&mut self, input: InputEvent) {
        if let InputEvent::Key(ref key) = input {
            if key.code == KeyCode::Character('p') && key.modifiers.contains(Modifier::Control) {
                self.palette_visible = !self.palette_visible;
                if !self.palette_visible {
                    self.palette.clear_query();
                }
                return;
            }
            if key.code == KeyCode::Character('b') && key.modifiers.contains(Modifier::Control) {
                self.explorer_visible = !self.explorer_visible;
                return;
            }
            if key.code == KeyCode::Character('j') && key.modifiers.contains(Modifier::Control) {
                self.bottom_panel_visible = !self.bottom_panel_visible;
                return;
            }
            if self.palette_visible {
                match key.code {
                    KeyCode::Character(ch) if !key.modifiers.contains(Modifier::Control) => {
                        self.palette.push_query(ch);
                    }
                    KeyCode::Backspace => {
                        self.palette.pop_query();
                    }
                    KeyCode::Up => self.palette.move_selection(-1),
                    KeyCode::Down => self.palette.move_selection(1),
                    KeyCode::Enter => {
                        if let Some(command) = self.palette.activate_selected() {
                            let _ = self.apply_command(command.as_str());
                            self.palette_visible = false;
                            self.palette.clear_query();
                        }
                    }
                    KeyCode::Escape => {
                        self.palette_visible = false;
                        self.palette.clear_query();
                    }
                    _ => {}
                }
                return;
            }
        }
        let result = match input {
            InputEvent::Paste(text) => self
                .buffer
                .smart_insert(&text, &PairConfig::common_defaults())
                .map(|_| ()),
            InputEvent::Key(key) => match key.code {
                KeyCode::Character(character)
                    if !key.modifiers.contains(Modifier::Control)
                        && !key.modifiers.contains(Modifier::Alt)
                        && !key.modifiers.contains(Modifier::Meta) =>
                {
                    self.buffer
                        .smart_insert(&character.to_string(), &PairConfig::common_defaults())
                        .map(|_| ())
                }
                KeyCode::Enter => self
                    .buffer
                    .smart_enter(
                        &PairConfig::common_defaults(),
                        editor_core::IndentStyle::Spaces(4),
                    )
                    .map(|_| ()),
                KeyCode::Backspace => self.buffer.smart_backspace().map(|_| ()),
                KeyCode::Delete => self.delete_forward(),
                KeyCode::Left => self
                    .buffer
                    .move_left(key.modifiers.contains(Modifier::Shift)),
                KeyCode::Right => self
                    .buffer
                    .move_right(key.modifiers.contains(Modifier::Shift)),
                KeyCode::Up => {
                    self.buffer
                        .move_vertical(-1, key.modifiers.contains(Modifier::Shift), 4)
                }
                KeyCode::Down => {
                    self.buffer
                        .move_vertical(1, key.modifiers.contains(Modifier::Shift), 4)
                }
                KeyCode::Character('z') if key.modifiers.contains(Modifier::Control) => {
                    self.buffer.undo().map(|_| ())
                }
                KeyCode::Character('y') if key.modifiers.contains(Modifier::Control) => {
                    self.buffer.redo().map(|_| ())
                }
                _ => Ok(()),
            },
            InputEvent::Mouse(mouse) => {
                if let Some(offset) = self.mouse_offset(mouse.position.row, mouse.position.column) {
                    match mouse.action {
                        MouseAction::Down(MouseButton::Left) => {
                            self.mouse_anchor = Some(offset);
                            let _ = self
                                .buffer
                                .set_selections(editor_core::SelectionSet::single(
                                    editor_core::Selection::cursor(offset),
                                ));
                        }
                        MouseAction::Drag(MouseButton::Left) => {
                            let anchor = self.mouse_anchor.unwrap_or(offset);
                            let _ = self
                                .buffer
                                .set_selections(editor_core::SelectionSet::single(
                                    editor_core::Selection::new(anchor, offset),
                                ));
                        }
                        MouseAction::Up(MouseButton::Left) => self.mouse_anchor = None,
                        _ => {}
                    }
                }
                Ok(())
            }
            InputEvent::Resize { .. } => Ok(()),
        };
        if let Err(error) = result {
            self.output.push(OutputMessage {
                subsystem: "editor".to_owned(),
                operation: "input".to_owned(),
                level: OutputLevel::Error,
                message: error.to_string(),
            });
        }
        self.sync_buffer_projection();
    }

    fn delete_forward(&mut self) -> editor_core::Result<()> {
        let selections = self.buffer.selections().clone();
        let mut edits = Vec::new();
        for selection in selections.selections() {
            let range = selection.range();
            if !selection.is_cursor() {
                edits.push(editor_core::Edit::delete(range));
            } else if range.end.0 < self.buffer.len_chars() {
                edits.push(editor_core::Edit::delete(editor_types::TextRange {
                    start: range.end,
                    end: editor_types::CharacterOffset(range.end.0 + 1),
                }));
            }
        }
        if edits.is_empty() {
            return Ok(());
        }
        let transaction = editor_core::Transaction::new(edits)?;
        self.buffer.apply_transaction(transaction).map(|_| ())
    }

    fn apply_command(&mut self, command: &str) -> Option<Effect> {
        match command {
            "editor.undo" => {
                let _ = self.buffer.undo();
            }
            "editor.redo" => {
                let _ = self.buffer.redo();
            }
            "editor.quit" if !self.active_dirty => self.running = false,
            "editor.save" if self.active_dirty => {
                if let Some(path) = self.active_path.clone() {
                    return Some(Effect::SaveDocument {
                        path,
                        text: self.active_text.clone(),
                    });
                }
                self.output.push(OutputMessage {
                    subsystem: "workspace".to_owned(),
                    operation: "save-file".to_owned(),
                    level: OutputLevel::Warning,
                    message: "untitled buffer requires Save As before it can be written".to_owned(),
                });
            }
            "editor.save" => {}
            "git.refresh" if self.workspace_trusted => {
                if let Some(root) = self.workspace_roots.first().cloned() {
                    let request = RequestId(self.frame_number.saturating_add(1));
                    return Some(Effect::RefreshGitStatus { request, root });
                }
            }
            "git.refresh" => {
                self.output.push(OutputMessage {
                    subsystem: "workspace-trust".to_owned(),
                    operation: "git-status".to_owned(),
                    level: OutputLevel::Warning,
                    message: "trust the workspace before running Git operations".to_owned(),
                });
            }
            unknown => {
                self.output.push(OutputMessage {
                    subsystem: "editor".to_owned(),
                    operation: "command".to_owned(),
                    level: OutputLevel::Warning,
                    message: format!("command `{unknown}` is not available in this build"),
                });
            }
        }
        self.sync_buffer_projection();
        None
    }

    fn sync_buffer_projection(&mut self) {
        self.active_text = self.buffer.to_string();
        self.active_dirty = self.buffer.is_dirty();
        self.sync_active_tab();
    }

    fn refresh_explorer_entries(&mut self) {
        let mut entries = Vec::new();
        for root in &self.workspace_roots {
            let children = match workspace_core::canonicalize_path(root) {
                Ok(canonical) => {
                    workspace_core::ExplorerTree::new(vec![canonical.clone()], Vec::new())
                        .children(canonical.as_path())
                }
                Err(error) => {
                    self.output.push(OutputMessage {
                        subsystem: "workspace".to_owned(),
                        operation: "explorer".to_owned(),
                        level: OutputLevel::Warning,
                        message: error.to_string(),
                    });
                    continue;
                }
            };
            match children {
                Ok(children) => {
                    entries.extend(children.into_iter().map(|entry| ExplorerProjection {
                        depth: u8::try_from(entry.depth).unwrap_or(u8::MAX),
                        label: entry.path.file_name().map_or_else(
                            || entry.path.display().to_string(),
                            |name| name.to_string_lossy().into_owned(),
                        ),
                        active: self.active_path.as_ref() == Some(&entry.path),
                        expanded: false,
                    }));
                }
                Err(error) => self.output.push(OutputMessage {
                    subsystem: "workspace".to_owned(),
                    operation: "explorer".to_owned(),
                    level: OutputLevel::Warning,
                    message: error.to_string(),
                }),
            }
        }
        self.explorer_entries = entries;
    }

    fn explorer_refresh_transition(&mut self) -> Transition {
        let request = RequestId(self.frame_number.saturating_add(1));
        self.latest_explorer_request = Some(request);
        Transition {
            effects: vec![Effect::RefreshExplorer {
                request,
                roots: self.workspace_roots.clone(),
            }],
            render: true,
            ..Transition::default()
        }
    }

    fn mouse_offset(&self, screen_row: u16, screen_column: u16) -> Option<CharacterOffset> {
        // The shell reserves one row for tabs and a narrow Explorer gutter. On compact layouts
        // the subtraction saturates, keeping clicks within the first document line.
        let line = u32::from(screen_row.saturating_sub(2));
        let column = u32::from(if self.explorer_visible {
            screen_column.saturating_sub(25)
        } else {
            screen_column.saturating_sub(1)
        });
        self.buffer
            .position_to_offset(LogicalPosition {
                line,
                character: column,
            })
            .ok()
    }

    pub fn apply_event(&mut self, event: Event) {
        match event {
            Event::DocumentSaved { path } => {
                if self.active_path.as_ref() == Some(&path) {
                    self.buffer.mark_saved();
                    self.sync_buffer_projection();
                }
                self.output.push(OutputMessage {
                    subsystem: "workspace".to_owned(),
                    operation: "save-file".to_owned(),
                    level: OutputLevel::Information,
                    message: format!("saved {}", path.display()),
                });
            }
            Event::DocumentSaveFailed { message, .. } | Event::Output(message) => {
                self.output.push(message);
            }
            Event::GitStatusUpdated { request, summary } => {
                self.pending_processes.remove(&request);
                self.git_status = Some(summary);
            }
            Event::ExplorerUpdated { request, entries }
                if self.latest_explorer_request == Some(request) =>
            {
                self.explorer_entries = entries
                    .into_iter()
                    .map(|entry| ExplorerProjection {
                        depth: entry.depth,
                        label: entry.path.file_name().map_or_else(
                            || entry.path.display().to_string(),
                            |name| name.to_string_lossy().into_owned(),
                        ),
                        active: self.active_path.as_ref() == Some(&entry.path),
                        expanded: false,
                    })
                    .collect();
            }
            Event::ExplorerUpdated { .. } => {}
            Event::ExternalProcessBlocked { kind, .. } => {
                if matches!(kind, super::effect::ExternalProcessKind::LanguageServer) {
                    self.language_server = LanguageServerStatus::DisabledByPolicy;
                }
                self.output.push(OutputMessage {
                    subsystem: "workspace-trust".to_owned(),
                    operation: "authorize-external-process".to_owned(),
                    level: OutputLevel::Warning,
                    message: format!("blocked {kind:?} in an untrusted workspace"),
                });
            }
            Event::EffectFailed { request, message } => {
                if self.pending_processes.remove(&request)
                    == Some(super::effect::ExternalProcessKind::LanguageServer)
                {
                    self.language_server = LanguageServerStatus::Crashed {
                        message: message.message.clone(),
                    };
                }
                self.output.push(message);
            }
            Event::EffectCompleted(request) => {
                if self.pending_processes.remove(&request)
                    == Some(super::effect::ExternalProcessKind::LanguageServer)
                {
                    self.language_server = LanguageServerStatus::Running {
                        name: "language-server".to_owned(),
                    };
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use editor_types::{InputEvent, LanguageServerStatus, RequestId};

    use super::AppState;
    use crate::app::{
        action::Action,
        effect::{Effect, ExternalProcessKind, ProcessSpec},
    };
    use editor_core::TextBuffer;

    fn lsp_effect() -> Effect {
        Effect::ExternalProcess {
            request: RequestId(1),
            kind: ExternalProcessKind::LanguageServer,
            spec: ProcessSpec {
                executable: "server".to_owned(),
                arguments: Vec::new(),
            },
        }
    }

    #[test]
    fn external_process_is_blocked_before_dispatch_when_untrusted() {
        let mut state = AppState::default();
        let transition = state.apply_action(Action::RequestEffect(lsp_effect()));
        assert!(transition.effects.is_empty());
        assert_eq!(transition.events.len(), 1);
        state.apply_event(transition.events[0].clone());
        assert_eq!(
            state.language_server,
            LanguageServerStatus::DisabledByPolicy
        );
    }

    #[test]
    fn trusted_workspace_allows_external_process_dispatch() {
        let mut state = AppState::default();
        let _ = state.apply_action(Action::SetWorkspaceTrust(true));
        let transition = state.apply_action(Action::RequestEffect(lsp_effect()));
        assert_eq!(transition.effects.len(), 1);
        assert!(transition.events.is_empty());
    }

    #[test]
    fn control_q_and_control_c_exit_the_event_loop() {
        use editor_types::{KeyCode, KeyEvent, Modifiers};

        for key in ['q', 'c'] {
            let mut state = AppState::default();
            let action = Action::Input(InputEvent::Key(KeyEvent {
                code: KeyCode::Character(key),
                modifiers: Modifiers::from_modifiers([editor_types::Modifier::Control]),
                repeat: false,
            }));
            let transition = state.apply_action(action);
            assert!(!state.running);
            assert!(transition.render);
        }
    }

    #[test]
    fn text_input_updates_persistent_buffer_and_undoes() {
        use editor_types::{InputEvent, KeyCode, KeyEvent, Modifiers};

        let mut state = AppState::default();
        let key = |character| {
            Action::Input(InputEvent::Key(KeyEvent {
                code: KeyCode::Character(character),
                modifiers: Modifiers::default(),
                repeat: false,
            }))
        };
        let _ = state.apply_action(key('h'));
        let _ = state.apply_action(key('i'));
        assert_eq!(state.active_text, "hi");
        assert!(state.active_dirty);

        let _ = state.apply_action(Action::Invoke(editor_types::CommandId::new("editor.undo")));
        assert_eq!(state.active_text, "h");
        assert!(state.active_dirty);
    }

    #[test]
    fn dirty_quit_is_blocked_and_surfaces_a_warning() {
        use editor_types::{InputEvent, KeyCode, KeyEvent, Modifiers};

        let mut state = AppState::default();
        let _ = state.apply_action(Action::Input(InputEvent::Key(KeyEvent {
            code: KeyCode::Character('x'),
            modifiers: Modifiers::default(),
            repeat: false,
        })));
        let transition = state.apply_action(Action::Quit);
        assert!(state.running);
        assert!(transition.render);
        assert!(
            state
                .output
                .iter()
                .any(|message| message.operation == "quit")
        );
    }

    #[test]
    fn command_palette_routes_selected_command_to_root_state() {
        use editor_types::{InputEvent, KeyCode, KeyEvent, Modifier, Modifiers};
        let mut state = AppState::default();
        let key = |code| {
            Action::Input(InputEvent::Key(KeyEvent {
                code,
                modifiers: Modifiers::default(),
                repeat: false,
            }))
        };
        let _ = state.apply_action(Action::Input(InputEvent::Key(KeyEvent {
            code: KeyCode::Character('p'),
            modifiers: Modifiers::from_modifiers([Modifier::Control]),
            repeat: false,
        })));
        assert!(state.palette_visible);
        for character in "quit".chars() {
            let _ = state.apply_action(key(KeyCode::Character(character)));
        }
        let _ = state.apply_action(key(KeyCode::Enter));
        assert!(!state.running);
        assert!(!state.palette_visible);
    }

    #[test]
    fn trusting_workspace_marks_lsp_unavailable_when_no_known_server_exists() {
        let directory = tempfile::tempdir().expect("temporary directory is available");
        let path = directory.path().join("sample.rs");
        std::fs::write(&path, "fn main() {}\n").expect("fixture is writable");
        let mut state = AppState::default();
        state.open_startup_path(&path);
        let transition = state.apply_action(Action::SetWorkspaceTrust(true));
        assert!(transition.render);
        assert!(matches!(
            state.language_server,
            LanguageServerStatus::Unavailable
                | LanguageServerStatus::Starting
                | LanguageServerStatus::Running { .. }
        ));
    }

    #[test]
    fn mouse_click_and_drag_create_a_logical_selection() {
        use editor_types::{
            InputEvent, Modifiers, MouseAction, MouseButton, MouseEvent, ScreenCell,
        };
        let mut state = AppState {
            buffer: TextBuffer::new("hello world"),
            ..AppState::default()
        };
        state.sync_buffer_projection();
        let click = MouseEvent {
            position: ScreenCell { row: 2, column: 25 },
            action: MouseAction::Down(MouseButton::Left),
            modifiers: Modifiers::default(),
        };
        let drag = MouseEvent {
            position: ScreenCell { row: 2, column: 30 },
            action: MouseAction::Drag(MouseButton::Left),
            modifiers: Modifiers::default(),
        };
        let _ = state.apply_action(Action::Input(InputEvent::Mouse(click)));
        let _ = state.apply_action(Action::Input(InputEvent::Mouse(drag)));
        assert_eq!(state.buffer.selections().primary().range().start.0, 0);
        assert_eq!(state.buffer.selections().primary().range().end.0, 5);
    }

    #[test]
    fn trusted_git_refresh_is_emitted_as_a_background_effect() {
        let directory = tempfile::tempdir().expect("temporary directory is available");
        let mut state = AppState::default();
        let _ = state.apply_action(Action::AddWorkspaceRoot(directory.path().to_path_buf()));
        let _ = state.apply_action(Action::SetWorkspaceTrust(true));
        let transition =
            state.apply_action(Action::Invoke(editor_types::CommandId::new("git.refresh")));
        assert!(matches!(
            transition.effects.first(),
            Some(Effect::RefreshGitStatus { .. })
        ));
    }
}
