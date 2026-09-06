//! Terminal lifecycle, normalized input, clipboard, and virtual-framebuffer boundary.

mod capability;
mod clipboard;
mod framebuffer;
mod input;
mod renderer;
mod terminal;

pub use capability::{
    ResolvedColor, ResolvedUnderline, RgbColor, Theme, detect_capabilities, quantize_ansi16,
    quantize_ansi256, resolve_color, resolve_underline,
};
pub use clipboard::{Clipboard, ClipboardError, MemoryClipboard, SystemClipboard};
pub use framebuffer::{Cell, Framebuffer, FramebufferError};
pub use input::{InputReader, normalize_event, normalize_key, normalize_mouse};
pub use renderer::DifferentialRenderer;
pub use terminal::{CrosstermBackend, CursorShape, TerminalError, restore_after_panic};

use editor_types::TerminalCapabilities;
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

/// Display width of one grapheme cluster in terminal cells.
#[must_use]
pub fn grapheme_width(grapheme: &str) -> usize {
    UnicodeWidthStr::width(grapheme)
}

/// Display width of a string using terminal grapheme metrics.
#[must_use]
pub fn display_width(text: &str) -> usize {
    text.graphemes(true).map(grapheme_width).sum()
}

/// Truncates text to at most `width` terminal cells, appending ASCII `...` when needed.
#[must_use]
pub fn truncate_display(text: &str, width: usize) -> String {
    if display_width(text) <= width {
        return text.to_owned();
    }
    if width <= 3 {
        return text
            .graphemes(true)
            .scan(0_usize, |used, grapheme| {
                let next = used.saturating_add(grapheme_width(grapheme));
                (next <= width).then(|| {
                    *used = next;
                    grapheme
                })
            })
            .collect();
    }
    let target = width.saturating_sub(3);
    let mut output = String::new();
    let mut used: usize = 0;
    for grapheme in text.graphemes(true) {
        let next = used.saturating_add(grapheme_width(grapheme));
        if next > target {
            break;
        }
        output.push_str(grapheme);
        used = next;
    }
    output.push_str("...");
    output
}

pub trait TerminalAdapter {
    type Error: std::error::Error + Send + Sync + 'static;

    fn capabilities(&self) -> TerminalCapabilities;
    /// Activates terminal modes owned by the adapter.
    ///
    /// # Errors
    ///
    /// Returns the adapter error when terminal modes cannot be activated safely.
    fn enter(&mut self) -> Result<(), Self::Error>;
    /// Presents the next framebuffer.
    ///
    /// # Errors
    ///
    /// Returns the adapter error when frame output fails.
    fn render(&mut self, frame: &Framebuffer) -> Result<(), Self::Error>;
    /// Presents the native terminal caret. The default keeps lightweight/fake adapters source
    /// compatible while concrete terminals may emit a steady-bar cursor command.
    ///
    /// # Errors
    ///
    /// Returns the adapter error when cursor output cannot be written.
    fn present_cursor(&mut self, _cursor: Option<(u16, u16)>) -> Result<(), Self::Error> {
        Ok(())
    }
    /// Restores all terminal modes changed by [`Self::enter`].
    ///
    /// # Errors
    ///
    /// Returns the adapter error when restoration is incomplete.
    fn restore(&mut self) -> Result<(), Self::Error>;
}
