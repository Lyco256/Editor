//! Authoritative root state and pure transition logic.

use std::path::{Path, PathBuf};

use editor_types::{
    InputEvent, KeyCode, LanguageServerStatus, Modifier, OutputLevel, OutputMessage,
};

use super::{action::Action, effect::Effect, event::Event};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppState {
    pub running: bool,
    pub workspace_trusted: bool,
    pub frame_number: u64,
    pub language_server: LanguageServerStatus,
    pub output: Vec<OutputMessage>,
    pub active_path: Option<PathBuf>,
    pub active_text: String,
    pub active_dirty: bool,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            running: true,
            workspace_trusted: false,
            frame_number: 0,
            language_server: LanguageServerStatus::Stopped,
            output: Vec::new(),
            active_path: None,
            active_text: String::new(),
            active_dirty: false,
        }
    }
}

impl AppState {
    /// Loads a startup path into the view model without launching external processes.
    ///
    /// Directories become workspace roots; regular files are decoded using the workspace document
    /// adapter and retained as editable text. Missing or malformed files are reported as structured
    /// output while the editor remains usable.
    pub fn open_startup_path(&mut self, path: impl AsRef<Path>) {
        let path = path.as_ref().to_path_buf();
        if path.is_dir() {
            self.active_path = Some(path);
            return;
        }
        match workspace_core::load_text_document(
            &path,
            &workspace_core::DocumentLoadOptions::default(),
        ) {
            Ok(document) => {
                self.active_path = Some(document.path);
                self.active_text = document.text;
                self.active_dirty = false;
            }
            Err(error) => self.output.push(OutputMessage {
                subsystem: "workspace".to_owned(),
                operation: "open-file".to_owned(),
                level: OutputLevel::Error,
                message: format!("could not open {}: {error}", path.display()),
            }),
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
    pub fn apply_action(&mut self, action: Action) -> Transition {
        match action {
            Action::Quit => {
                self.running = false;
                Transition {
                    render: true,
                    ..Transition::default()
                }
            }
            Action::SetWorkspaceTrust(trusted) => {
                self.workspace_trusted = trusted;
                Transition {
                    render: true,
                    ..Transition::default()
                }
            }
            Action::RequestEffect(effect)
                if effect.requires_trusted_workspace() && !self.workspace_trusted =>
            {
                let Effect::ExternalProcess { request, kind, .. } = effect else {
                    unreachable!("trust-gated effect is an external process")
                };
                Transition {
                    events: vec![Event::ExternalProcessBlocked { request, kind }],
                    render: true,
                    ..Transition::default()
                }
            }
            Action::RequestEffect(effect) => Transition {
                effects: vec![effect],
                ..Transition::default()
            },
            Action::Input(InputEvent::Key(key))
                if matches!(key.code, KeyCode::Character('c' | 'q'))
                    && key.modifiers.contains(Modifier::Control) =>
            {
                self.running = false;
                Transition {
                    render: true,
                    ..Transition::default()
                }
            }
            Action::Input(_) | Action::Invoke(_) => Transition {
                render: true,
                ..Transition::default()
            },
        }
    }

    pub fn apply_event(&mut self, event: Event) {
        match event {
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
            Event::EffectFailed { message, .. } | Event::Output(message) => {
                self.output.push(message);
            }
            Event::EffectCompleted(_) => {}
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
}
