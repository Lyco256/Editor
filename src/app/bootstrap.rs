//! Process bootstrap and safe fallback startup path.

use std::{convert::Infallible, process::ExitCode};

use editor_types::TerminalCapabilities;
use terminal_backend::{Framebuffer, TerminalAdapter};

use super::{
    action::Action,
    runtime::{AppRuntime, QueueActionSource, RecordingDispatcher},
};

#[derive(Debug, Default)]
struct BootstrapTerminal;

impl TerminalAdapter for BootstrapTerminal {
    type Error = Infallible;

    fn capabilities(&self) -> TerminalCapabilities {
        TerminalCapabilities::default()
    }

    fn enter(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }

    fn render(&mut self, _frame: &Framebuffer) -> Result<(), Self::Error> {
        Ok(())
    }

    fn restore(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
}

#[must_use]
pub fn run() -> ExitCode {
    let input = QueueActionSource::new([Action::Quit]);
    let runtime = AppRuntime::new(
        BootstrapTerminal,
        input,
        RecordingDispatcher::default(),
        (80, 24),
    );
    match runtime.run() {
        Ok(_) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("Editor startup failed: {error}");
            ExitCode::FAILURE
        }
    }
}
