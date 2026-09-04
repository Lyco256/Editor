//! Event-driven runtime with one authoritative state mutation path.

use std::collections::VecDeque;

use app_ui::frame::empty_frame;
use terminal_backend::{Framebuffer, TerminalAdapter};
use thiserror::Error;

use super::{action::Action, effect::Effect, state::AppState};

pub trait ActionSource {
    fn next_action(&mut self) -> Option<Action>;
}

pub trait EffectDispatcher {
    fn dispatch(&mut self, effect: Effect);
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum RuntimeError {
    #[error("could not enter terminal: {0}")]
    Enter(String),
    #[error("could not render terminal frame: {0}")]
    Render(String),
    #[error("could not restore terminal: {0}")]
    Restore(String),
    #[error("runtime failed ({runtime}); terminal restore also failed ({restore})")]
    RestoreAfterFailure { runtime: String, restore: String },
}

pub struct AppRuntime<T, I, D> {
    terminal: T,
    input: I,
    dispatcher: D,
    state: AppState,
    size: (u16, u16),
}

impl<T, I, D> AppRuntime<T, I, D>
where
    T: TerminalAdapter,
    I: ActionSource,
    D: EffectDispatcher,
{
    #[must_use]
    pub fn new(terminal: T, input: I, dispatcher: D, size: (u16, u16)) -> Self {
        Self {
            terminal,
            input,
            dispatcher,
            state: AppState::default(),
            size,
        }
    }

    /// Runs until input ends or the state accepts a quit action.
    ///
    /// # Errors
    ///
    /// Returns a typed lifecycle error when terminal entry, rendering, or restoration fails.
    pub fn run(mut self) -> Result<AppState, RuntimeError> {
        self.terminal
            .enter()
            .map_err(|error| RuntimeError::Enter(error.to_string()))?;

        let runtime_result = self.run_entered();
        let restore_result = self.terminal.restore().map_err(|error| error.to_string());

        match (runtime_result, restore_result) {
            (Ok(()), Ok(())) => Ok(self.state),
            (Ok(()), Err(restore)) => Err(RuntimeError::Restore(restore)),
            (Err(runtime), Ok(())) => Err(runtime),
            (Err(runtime), Err(restore)) => Err(RuntimeError::RestoreAfterFailure {
                runtime: runtime.to_string(),
                restore,
            }),
        }
    }

    fn run_entered(&mut self) -> Result<(), RuntimeError> {
        self.render()?;
        while self.state.running {
            let Some(action) = self.input.next_action() else {
                break;
            };
            let transition = self.state.apply_action(action);
            for event in transition.events {
                self.state.apply_event(event);
            }
            for effect in transition.effects {
                self.dispatcher.dispatch(effect);
            }
            if transition.render {
                self.render()?;
            }
        }
        Ok(())
    }

    fn render(&mut self) -> Result<(), RuntimeError> {
        let frame: Framebuffer = empty_frame(self.size.0, self.size.1);
        self.terminal
            .render(&frame)
            .map_err(|error| RuntimeError::Render(error.to_string()))?;
        self.state.frame_number += 1;
        Ok(())
    }
}

#[derive(Debug, Default)]
pub struct QueueActionSource {
    actions: VecDeque<Action>,
}

impl QueueActionSource {
    #[must_use]
    pub fn new(actions: impl IntoIterator<Item = Action>) -> Self {
        Self {
            actions: actions.into_iter().collect(),
        }
    }
}

impl ActionSource for QueueActionSource {
    fn next_action(&mut self) -> Option<Action> {
        self.actions.pop_front()
    }
}

#[derive(Debug, Default)]
pub struct RecordingDispatcher {
    pub effects: Vec<Effect>,
}

impl EffectDispatcher for RecordingDispatcher {
    fn dispatch(&mut self, effect: Effect) {
        self.effects.push(effect);
    }
}
