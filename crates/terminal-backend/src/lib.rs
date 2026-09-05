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
    /// Restores all terminal modes changed by [`Self::enter`].
    ///
    /// # Errors
    ///
    /// Returns the adapter error when restoration is incomplete.
    fn restore(&mut self) -> Result<(), Self::Error>;
}
