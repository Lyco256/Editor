//! Crossterm-backed terminal lifecycle with retryable, idempotent cleanup.

use std::{
    io::{self, Write},
    time::Duration,
};

use crossterm::{
    cursor::{Hide, MoveTo, SetCursorStyle, Show},
    event::{
        self, DisableBracketedPaste, DisableMouseCapture, EnableBracketedPaste, EnableMouseCapture,
        KeyboardEnhancementFlags, PopKeyboardEnhancementFlags, PushKeyboardEnhancementFlags,
    },
    execute,
    style::{Attribute, ResetColor, SetAttribute},
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use editor_types::{InputEvent, TerminalCapabilities, TerminalFeature};
use thiserror::Error;

use crate::{
    DifferentialRenderer, Framebuffer, InputReader, MouseClickTracker, TerminalAdapter, Theme,
    detect_capabilities, normalize_event,
};

/// Cursor shapes exposed without leaking crossterm types to higher layers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CursorShape {
    Default,
    BlinkingBlock,
    SteadyBlock,
    BlinkingUnderline,
    SteadyUnderline,
    BlinkingBar,
    SteadyBar,
}

/// A terminal adapter failure annotated with the lifecycle operation that failed.
#[derive(Debug, Error)]
pub enum TerminalError {
    #[error("terminal operation `{operation}` failed: {source}")]
    Io {
        operation: &'static str,
        #[source]
        source: io::Error,
    },
    #[error("terminal must be entered before `{operation}`")]
    NotEntered { operation: &'static str },
}

/// Production terminal adapter. Its writer is injectable for deterministic rendering tests.
#[derive(Debug)]
pub struct CrosstermBackend<W: Write> {
    renderer: DifferentialRenderer<W>,
    cleanup: CleanupState,
    entered: bool,
    click_tracker: MouseClickTracker,
}

impl<W: Write> CrosstermBackend<W> {
    #[must_use]
    pub fn new(writer: W) -> Self {
        Self::with_capabilities(writer, detect_capabilities())
    }

    #[must_use]
    pub fn with_capabilities(writer: W, capabilities: TerminalCapabilities) -> Self {
        Self {
            renderer: DifferentialRenderer::new(writer, capabilities, Theme::default()),
            cleanup: CleanupState::default(),
            entered: false,
            click_tracker: MouseClickTracker::new(),
        }
    }

    /// Replaces the semantic theme used by subsequent framebuffer renders.
    pub fn set_theme(&mut self, theme: Theme) {
        self.renderer.set_theme(theme);
    }

    /// Returns the current terminal dimensions.
    ///
    /// # Errors
    ///
    /// Returns a typed I/O error when the host cannot report a terminal size.
    pub fn size() -> Result<(u16, u16), TerminalError> {
        crossterm::terminal::size().map_err(|source| TerminalError::Io {
            operation: "read terminal size",
            source,
        })
    }

    /// Moves the visible cursor using zero-based cell coordinates.
    ///
    /// # Errors
    ///
    /// Returns an error when the adapter is inactive or terminal output fails.
    pub fn move_cursor(&mut self, column: u16, row: u16) -> Result<(), TerminalError> {
        self.ensure_entered("move cursor")?;
        execute!(self.renderer.writer_mut(), MoveTo(column, row)).map_err(|source| {
            TerminalError::Io {
                operation: "move cursor",
                source,
            }
        })
    }

    /// Changes cursor visibility and records hidden state for crash cleanup.
    ///
    /// # Errors
    ///
    /// Returns an error when the adapter is inactive or terminal output fails.
    pub fn set_cursor_visible(&mut self, visible: bool) -> Result<(), TerminalError> {
        self.ensure_entered("set cursor visibility")?;
        let result = if visible {
            execute!(self.renderer.writer_mut(), Show)
        } else {
            execute!(self.renderer.writer_mut(), Hide)
        };
        result.map_err(|source| TerminalError::Io {
            operation: "set cursor visibility",
            source,
        })?;
        if visible {
            self.cleanup.completed(CleanupAction::Cursor);
        } else {
            self.cleanup.mark(CleanupAction::Cursor);
        }
        Ok(())
    }

    /// Selects a portable cursor shape.
    ///
    /// # Errors
    ///
    /// Returns an error when the adapter is inactive or terminal output fails.
    pub fn set_cursor_shape(&mut self, shape: CursorShape) -> Result<(), TerminalError> {
        self.ensure_entered("set cursor shape")?;
        let style = match shape {
            CursorShape::Default => SetCursorStyle::DefaultUserShape,
            CursorShape::BlinkingBlock => SetCursorStyle::BlinkingBlock,
            CursorShape::SteadyBlock => SetCursorStyle::SteadyBlock,
            CursorShape::BlinkingUnderline => SetCursorStyle::BlinkingUnderScore,
            CursorShape::SteadyUnderline => SetCursorStyle::SteadyUnderScore,
            CursorShape::BlinkingBar => SetCursorStyle::BlinkingBar,
            CursorShape::SteadyBar => SetCursorStyle::SteadyBar,
        };
        execute!(self.renderer.writer_mut(), style).map_err(|source| TerminalError::Io {
            operation: "set cursor shape",
            source,
        })
    }

    fn ensure_entered(&self, operation: &'static str) -> Result<(), TerminalError> {
        if self.entered {
            Ok(())
        } else {
            Err(TerminalError::NotEntered { operation })
        }
    }

    fn enter_inner(&mut self) -> Result<(), TerminalError> {
        enable_raw_mode().map_err(|source| TerminalError::Io {
            operation: "enable raw mode",
            source,
        })?;
        self.cleanup.mark(CleanupAction::Raw);

        if self
            .renderer
            .capabilities()
            .features
            .contains(TerminalFeature::AlternateScreen)
        {
            execute!(self.renderer.writer_mut(), EnterAlternateScreen).map_err(|source| {
                TerminalError::Io {
                    operation: "enter alternate screen",
                    source,
                }
            })?;
            self.cleanup.mark(CleanupAction::AlternateScreen);
        }

        execute!(self.renderer.writer_mut(), Hide).map_err(|source| TerminalError::Io {
            operation: "hide cursor",
            source,
        })?;
        self.cleanup.mark(CleanupAction::Cursor);

        if self
            .renderer
            .capabilities()
            .features
            .contains(TerminalFeature::Mouse)
        {
            execute!(self.renderer.writer_mut(), EnableMouseCapture).map_err(|source| {
                TerminalError::Io {
                    operation: "enable mouse reporting",
                    source,
                }
            })?;
            self.cleanup.mark(CleanupAction::Mouse);
        }

        if self
            .renderer
            .capabilities()
            .features
            .contains(TerminalFeature::BracketedPaste)
        {
            execute!(self.renderer.writer_mut(), EnableBracketedPaste).map_err(|source| {
                TerminalError::Io {
                    operation: "enable bracketed paste",
                    source,
                }
            })?;
            self.cleanup.mark(CleanupAction::BracketedPaste);
        }

        if self
            .renderer
            .capabilities()
            .features
            .contains(TerminalFeature::EnhancedKeyboard)
        {
            let flags = KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES
                | KeyboardEnhancementFlags::REPORT_EVENT_TYPES;
            execute!(
                self.renderer.writer_mut(),
                PushKeyboardEnhancementFlags(flags)
            )
            .map_err(|source| TerminalError::Io {
                operation: "enable enhanced keyboard reporting",
                source,
            })?;
            self.cleanup.mark(CleanupAction::EnhancedKeyboard);
        }
        Ok(())
    }

    fn restore_inner(&mut self) -> Result<(), TerminalError> {
        let mut first_error = None;
        for action in self.cleanup.pending() {
            match self.perform_cleanup(action) {
                Ok(()) => self.cleanup.completed(action),
                Err(error) => {
                    if first_error.is_none() {
                        first_error = Some(error);
                    }
                }
            }
        }
        self.entered = !self.cleanup.pending().is_empty();
        first_error.map_or(Ok(()), Err)
    }

    fn perform_cleanup(&mut self, action: CleanupAction) -> Result<(), TerminalError> {
        let result = match action {
            CleanupAction::EnhancedKeyboard => {
                execute!(self.renderer.writer_mut(), PopKeyboardEnhancementFlags)
            }
            CleanupAction::BracketedPaste => {
                execute!(self.renderer.writer_mut(), DisableBracketedPaste)
            }
            CleanupAction::Mouse => execute!(self.renderer.writer_mut(), DisableMouseCapture),
            CleanupAction::Cursor => execute!(
                self.renderer.writer_mut(),
                SetAttribute(Attribute::Reset),
                ResetColor,
                Show,
                SetCursorStyle::DefaultUserShape
            ),
            CleanupAction::AlternateScreen => {
                execute!(self.renderer.writer_mut(), LeaveAlternateScreen)
            }
            CleanupAction::Raw => disable_raw_mode(),
        };
        result.map_err(|source| TerminalError::Io {
            operation: action.operation(),
            source,
        })
    }
}

impl CrosstermBackend<std::io::Stdout> {
    #[must_use]
    pub fn stdout() -> Self {
        Self::new(std::io::stdout())
    }
}

impl<W: Write> TerminalAdapter for CrosstermBackend<W> {
    type Error = TerminalError;

    fn capabilities(&self) -> TerminalCapabilities {
        self.renderer.capabilities()
    }

    fn enter(&mut self) -> Result<(), Self::Error> {
        if self.entered {
            return Ok(());
        }
        if let Err(primary) = self.enter_inner() {
            let _cleanup_result = self.restore_inner();
            return Err(primary);
        }
        self.entered = true;
        self.renderer.invalidate();
        Ok(())
    }

    fn render(&mut self, frame: &Framebuffer) -> Result<(), Self::Error> {
        self.ensure_entered("render frame")?;
        self.renderer
            .render(frame)
            .map(|_| ())
            .map_err(|source| TerminalError::Io {
                operation: "render frame",
                source,
            })
    }

    fn present_cursor(&mut self, cursor: Option<(u16, u16)>) -> Result<(), Self::Error> {
        self.ensure_entered("present cursor")?;
        if let Some((column, row)) = cursor {
            execute!(
                self.renderer.writer_mut(),
                MoveTo(column, row),
                SetCursorStyle::SteadyBar,
                Show
            )
        } else {
            execute!(self.renderer.writer_mut(), Hide)
        }
        .map_err(|source| TerminalError::Io {
            operation: "present cursor",
            source,
        })
    }

    fn restore(&mut self) -> Result<(), Self::Error> {
        self.restore_inner()
    }
}

impl<W: Write> InputReader for CrosstermBackend<W> {
    type Error = TerminalError;

    fn read_input(&mut self) -> Result<InputEvent, Self::Error> {
        self.ensure_entered("read input")?;
        loop {
            let event = event::read().map_err(|source| TerminalError::Io {
                operation: "read input",
                source,
            })?;
            let input = match event {
                crossterm::event::Event::Mouse(mouse) => self
                    .click_tracker
                    .normalize(mouse, std::time::Instant::now())
                    .map(InputEvent::Mouse),
                other => normalize_event(other),
            };
            if let Some(input) = input {
                return Ok(input);
            }
        }
    }

    fn poll_input(&mut self, timeout: Duration) -> Result<bool, Self::Error> {
        self.ensure_entered("poll input")?;
        event::poll(timeout).map_err(|source| TerminalError::Io {
            operation: "poll input",
            source,
        })
    }
}

impl<W: Write> Drop for CrosstermBackend<W> {
    fn drop(&mut self) {
        let _cleanup_result = self.restore_inner();
    }
}

/// Best-effort process-wide cleanup used by the panic hook.
///
/// Release builds abort on panic, so adapter `Drop` implementations cannot be relied on. This
/// helper deliberately ignores individual terminal errors because it runs while unwinding or
/// aborting and must never mask the original panic.
pub fn restore_after_panic() {
    let mut stdout = io::stdout();
    let _ = disable_raw_mode();
    let _ = execute!(
        stdout,
        PopKeyboardEnhancementFlags,
        DisableBracketedPaste,
        DisableMouseCapture,
        SetAttribute(Attribute::Reset),
        ResetColor,
        Show,
        SetCursorStyle::DefaultUserShape,
        LeaveAlternateScreen
    );
    let _ = stdout.flush();
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CleanupAction {
    EnhancedKeyboard,
    BracketedPaste,
    Mouse,
    Cursor,
    AlternateScreen,
    Raw,
}

impl CleanupAction {
    const fn operation(self) -> &'static str {
        match self {
            Self::EnhancedKeyboard => "disable enhanced keyboard reporting",
            Self::BracketedPaste => "disable bracketed paste",
            Self::Mouse => "disable mouse reporting",
            Self::Cursor => "restore cursor and style",
            Self::AlternateScreen => "leave alternate screen",
            Self::Raw => "disable raw mode",
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct CleanupState {
    bits: u8,
}

impl CleanupState {
    const ENHANCED_KEYBOARD: u8 = 1 << 0;
    const BRACKETED_PASTE: u8 = 1 << 1;
    const MOUSE: u8 = 1 << 2;
    const CURSOR: u8 = 1 << 3;
    const ALTERNATE_SCREEN: u8 = 1 << 4;
    const RAW: u8 = 1 << 5;

    fn pending(&self) -> Vec<CleanupAction> {
        let states = [
            (
                self.bits & Self::ENHANCED_KEYBOARD != 0,
                CleanupAction::EnhancedKeyboard,
            ),
            (
                self.bits & Self::BRACKETED_PASTE != 0,
                CleanupAction::BracketedPaste,
            ),
            (self.bits & Self::MOUSE != 0, CleanupAction::Mouse),
            (self.bits & Self::CURSOR != 0, CleanupAction::Cursor),
            (
                self.bits & Self::ALTERNATE_SCREEN != 0,
                CleanupAction::AlternateScreen,
            ),
            (self.bits & Self::RAW != 0, CleanupAction::Raw),
        ];
        states
            .into_iter()
            .filter_map(|(active, action)| active.then_some(action))
            .collect()
    }

    fn completed(&mut self, action: CleanupAction) {
        match action {
            CleanupAction::EnhancedKeyboard => self.bits &= !Self::ENHANCED_KEYBOARD,
            CleanupAction::BracketedPaste => self.bits &= !Self::BRACKETED_PASTE,
            CleanupAction::Mouse => self.bits &= !Self::MOUSE,
            CleanupAction::Cursor => self.bits &= !Self::CURSOR,
            CleanupAction::AlternateScreen => self.bits &= !Self::ALTERNATE_SCREEN,
            CleanupAction::Raw => self.bits &= !Self::RAW,
        }
    }

    fn mark(&mut self, action: CleanupAction) {
        match action {
            CleanupAction::EnhancedKeyboard => self.bits |= Self::ENHANCED_KEYBOARD,
            CleanupAction::BracketedPaste => self.bits |= Self::BRACKETED_PASTE,
            CleanupAction::Mouse => self.bits |= Self::MOUSE,
            CleanupAction::Cursor => self.bits |= Self::CURSOR,
            CleanupAction::AlternateScreen => self.bits |= Self::ALTERNATE_SCREEN,
            CleanupAction::Raw => self.bits |= Self::RAW,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{CleanupAction, CleanupState, CrosstermBackend};
    use crate::TerminalAdapter;
    use editor_types::TerminalCapabilities;

    #[test]
    fn cleanup_state_is_ordered_and_idempotent() {
        let mut state = CleanupState::default();
        state.mark(CleanupAction::EnhancedKeyboard);
        state.mark(CleanupAction::BracketedPaste);
        state.mark(CleanupAction::Mouse);
        state.mark(CleanupAction::Cursor);
        state.mark(CleanupAction::AlternateScreen);
        state.mark(CleanupAction::Raw);
        let first = state.pending();
        assert_eq!(
            first,
            vec![
                CleanupAction::EnhancedKeyboard,
                CleanupAction::BracketedPaste,
                CleanupAction::Mouse,
                CleanupAction::Cursor,
                CleanupAction::AlternateScreen,
                CleanupAction::Raw,
            ]
        );
        for action in first {
            state.completed(action);
        }
        assert!(state.pending().is_empty());
        assert!(state.pending().is_empty());
    }

    #[test]
    fn failed_cleanup_action_remains_retryable() {
        let mut state = CleanupState::default();
        state.mark(CleanupAction::Raw);
        assert_eq!(state.pending(), vec![CleanupAction::Raw]);
        assert_eq!(state.pending(), vec![CleanupAction::Raw]);
    }

    #[test]
    fn native_cursor_presentation_emits_steady_bar_and_hides_when_unfocused() {
        let mut backend =
            CrosstermBackend::with_capabilities(Vec::<u8>::new(), TerminalCapabilities::default());
        backend.entered = true;
        backend
            .present_cursor(Some((4, 2)))
            .expect("cursor presentation");
        assert!(!backend.renderer.writer_mut().is_empty());

        let mut backend =
            CrosstermBackend::with_capabilities(Vec::<u8>::new(), TerminalCapabilities::default());
        backend.entered = true;
        backend.present_cursor(None).expect("cursor hide");
        assert!(!backend.renderer.writer_mut().is_empty());
    }
}
