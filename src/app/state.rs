//! Authoritative root state and pure transition logic.

use editor_types::{LanguageServerStatus, OutputLevel, OutputMessage};

use super::{action::Action, effect::Effect, event::Event};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppState {
    pub running: bool,
    pub workspace_trusted: bool,
    pub frame_number: u64,
    pub language_server: LanguageServerStatus,
    pub output: Vec<OutputMessage>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            running: true,
            workspace_trusted: false,
            frame_number: 0,
            language_server: LanguageServerStatus::Stopped,
            output: Vec::new(),
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
    use editor_types::{LanguageServerStatus, RequestId};

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
}
