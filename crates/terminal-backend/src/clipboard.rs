//! System clipboard port and deterministic in-memory adapter.

use thiserror::Error;

/// Clipboard text operations used by copy, cut, and paste commands.
pub trait Clipboard {
    type Error: std::error::Error + Send + Sync + 'static;

    /// Reads the current text content.
    ///
    /// # Errors
    ///
    /// Returns a typed adapter error when the clipboard is unavailable or has no readable text.
    fn read_text(&mut self) -> Result<String, Self::Error>;

    /// Replaces clipboard text. A cut command must delete its selection only after this succeeds.
    ///
    /// # Errors
    ///
    /// Returns a typed adapter error when the host clipboard cannot be written.
    fn write_text(&mut self, text: &str) -> Result<(), Self::Error>;
}

/// A system clipboard failure that never includes clipboard content.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ClipboardError {
    #[error("system clipboard is unavailable: {message}")]
    Unavailable { message: String },
    #[error("could not read system clipboard text: {message}")]
    Read { message: String },
    #[error("could not write system clipboard text: {message}")]
    Write { message: String },
}

/// The Tier 1 Windows and graphical Tier 2 Unix system clipboard adapter.
pub struct SystemClipboard {
    inner: arboard::Clipboard,
}

impl SystemClipboard {
    /// Connects to the current desktop clipboard service.
    ///
    /// # Errors
    ///
    /// Returns [`ClipboardError::Unavailable`] in headless sessions or when no provider exists.
    pub fn new() -> Result<Self, ClipboardError> {
        arboard::Clipboard::new()
            .map(|inner| Self { inner })
            .map_err(|error| ClipboardError::Unavailable {
                message: error.to_string(),
            })
    }
}

impl Clipboard for SystemClipboard {
    type Error = ClipboardError;

    fn read_text(&mut self) -> Result<String, Self::Error> {
        self.inner.get_text().map_err(|error| ClipboardError::Read {
            message: error.to_string(),
        })
    }

    fn write_text(&mut self, text: &str) -> Result<(), Self::Error> {
        self.inner
            .set_text(text)
            .map_err(|error| ClipboardError::Write {
                message: error.to_string(),
            })
    }
}

/// A fake clipboard suitable for headless UI and command tests.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MemoryClipboard {
    text: String,
    available: bool,
}

impl MemoryClipboard {
    #[must_use]
    pub fn available() -> Self {
        Self {
            text: String::new(),
            available: true,
        }
    }

    #[must_use]
    pub fn unavailable() -> Self {
        Self::default()
    }
}

impl Clipboard for MemoryClipboard {
    type Error = ClipboardError;

    fn read_text(&mut self) -> Result<String, Self::Error> {
        self.available
            .then(|| self.text.clone())
            .ok_or_else(|| ClipboardError::Unavailable {
                message: "fake provider disabled".to_owned(),
            })
    }

    fn write_text(&mut self, text: &str) -> Result<(), Self::Error> {
        if !self.available {
            return Err(ClipboardError::Unavailable {
                message: "fake provider disabled".to_owned(),
            });
        }
        self.text.clear();
        self.text.push_str(text);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{Clipboard, ClipboardError, MemoryClipboard};

    #[test]
    fn fake_clipboard_round_trips_text() {
        let mut clipboard = MemoryClipboard::available();
        clipboard
            .write_text("copied text")
            .expect("fake clipboard is writable");
        assert_eq!(
            clipboard.read_text().expect("fake clipboard is readable"),
            "copied text"
        );
    }

    #[test]
    fn unavailable_clipboard_is_explicit() {
        let mut clipboard = MemoryClipboard::unavailable();
        assert!(matches!(
            clipboard.read_text(),
            Err(ClipboardError::Unavailable { .. })
        ));
    }
}
