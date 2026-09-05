use std::convert::Infallible;

use editor::app::{
    action::Action,
    runtime::{AppRuntime, QueueActionSource, RecordingDispatcher},
};
use editor_types::TerminalCapabilities;
use terminal_backend::{Framebuffer, TerminalAdapter};

#[derive(Debug, Default)]
struct FakeTerminal {
    entered: bool,
    restored: bool,
    frames: usize,
}

impl TerminalAdapter for FakeTerminal {
    type Error = Infallible;

    fn capabilities(&self) -> TerminalCapabilities {
        TerminalCapabilities::default()
    }

    fn enter(&mut self) -> Result<(), Self::Error> {
        self.entered = true;
        Ok(())
    }

    fn render(&mut self, _frame: &Framebuffer) -> Result<(), Self::Error> {
        assert!(self.entered);
        assert!(!self.restored);
        self.frames += 1;
        Ok(())
    }

    fn restore(&mut self) -> Result<(), Self::Error> {
        assert!(self.entered);
        self.restored = true;
        Ok(())
    }
}

#[test]
fn starts_renders_and_quits_cleanly_with_fake_adapters() {
    let runtime = AppRuntime::new(
        FakeTerminal::default(),
        QueueActionSource::new([Action::Quit]),
        RecordingDispatcher::default(),
        (80, 24),
    );
    let final_state = runtime.run().expect("headless runtime should be clean");
    assert!(!final_state.running);
    assert!(final_state.frame_number >= 1);
}

#[test]
fn startup_file_is_loaded_without_launching_external_effects() {
    let directory = tempfile::tempdir().expect("temporary directory is available");
    let path = directory.path().join("hello.rs");
    std::fs::write(&path, "fn main() {}\n").expect("fixture is writable");

    let mut state = editor::app::state::AppState::default();
    state.open_startup_path(&path);
    assert_eq!(state.active_path.as_deref(), Some(path.as_path()));
    assert_eq!(state.active_text, "fn main() {}\n");
    assert!(state.output.is_empty());
}
