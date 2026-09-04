//! ANSI/VT differential rendering confined to the terminal adapter boundary.

use std::io::{self, Write};

use editor_types::{StyleRole, TerminalCapabilities};

use crate::{
    Cell, Framebuffer, ResolvedColor, ResolvedUnderline, RgbColor, Theme, resolve_color,
    resolve_underline,
};

/// Renders only cells changed since the last successfully written frame.
#[derive(Debug)]
pub struct DifferentialRenderer<W: Write> {
    writer: W,
    capabilities: TerminalCapabilities,
    theme: Theme,
    previous: Option<Framebuffer>,
}

impl<W: Write> DifferentialRenderer<W> {
    #[must_use]
    pub fn new(writer: W, capabilities: TerminalCapabilities, theme: Theme) -> Self {
        Self {
            writer,
            capabilities,
            theme,
            previous: None,
        }
    }

    #[must_use]
    pub const fn capabilities(&self) -> TerminalCapabilities {
        self.capabilities
    }

    #[must_use]
    pub const fn writer(&self) -> &W {
        &self.writer
    }

    pub fn writer_mut(&mut self) -> &mut W {
        &mut self.writer
    }

    #[must_use]
    pub fn into_inner(self) -> W {
        self.writer
    }

    /// Forces the next presentation to repaint the complete frame.
    pub fn invalidate(&mut self) {
        self.previous = None;
    }

    /// Writes a frame and returns the exact payload size. Identical frames return zero and do no I/O.
    ///
    /// # Errors
    ///
    /// Returns the writer error. The previous-frame snapshot advances only after a complete write.
    pub fn render(&mut self, frame: &Framebuffer) -> io::Result<usize> {
        if self.previous.as_ref() == Some(frame) {
            return Ok(0);
        }

        let full_redraw = self
            .previous
            .as_ref()
            .is_none_or(|previous| previous.size() != frame.size());
        let mut payload = Vec::new();
        if full_redraw {
            payload.extend_from_slice(b"\x1b[2J");
        }
        let (columns, rows) = frame.size();
        for row in 0..rows {
            for column in 0..columns {
                let index = usize::from(row) * usize::from(columns) + usize::from(column);
                let cell = &frame.cells()[index];
                if cell.continuation {
                    continue;
                }
                let changed = full_redraw
                    || self
                        .previous
                        .as_ref()
                        .is_none_or(|previous| previous.cells()[index] != *cell);
                if changed {
                    write_cell(
                        &mut payload,
                        column,
                        row,
                        cell,
                        self.capabilities,
                        &self.theme,
                    )?;
                }
            }
        }
        payload.extend_from_slice(b"\x1b[0m");
        self.writer.write_all(&payload)?;
        self.writer.flush()?;
        self.previous = Some(frame.clone());
        Ok(payload.len())
    }
}

fn write_cell(
    payload: &mut Vec<u8>,
    column: u16,
    row: u16,
    cell: &Cell,
    capabilities: TerminalCapabilities,
    theme: &Theme,
) -> io::Result<()> {
    write!(
        payload,
        "\x1b[{};{}H\x1b[0m",
        u32::from(row) + 1,
        u32::from(column) + 1
    )?;
    write_color(
        payload,
        resolve_color(theme.color(cell.foreground), capabilities.color_depth),
        true,
    )?;
    write_color(
        payload,
        resolve_color(theme.color(cell.background), capabilities.color_depth),
        false,
    )?;
    if cell.bold {
        payload.extend_from_slice(b"\x1b[1m");
    }
    if is_diagnostic(cell.foreground) {
        write_underline(
            payload,
            resolve_underline(capabilities.underline),
            resolve_color(theme.color(cell.foreground), capabilities.color_depth),
        )?;
    }
    payload.extend_from_slice(cell.symbol.as_bytes());
    Ok(())
}

fn write_color(payload: &mut Vec<u8>, color: ResolvedColor, foreground: bool) -> io::Result<()> {
    let base = if foreground { 38 } else { 48 };
    match color {
        ResolvedColor::TrueColor(RgbColor { red, green, blue }) => {
            write!(payload, "\x1b[{base};2;{red};{green};{blue}m")?;
        }
        ResolvedColor::Ansi256(index) => write!(payload, "\x1b[{base};5;{index}m")?,
        ResolvedColor::Ansi16(index) => {
            let code = ansi16_code(index, foreground);
            write!(payload, "\x1b[{code}m")?;
        }
    }
    Ok(())
}

fn write_underline(
    payload: &mut Vec<u8>,
    underline: ResolvedUnderline,
    color: ResolvedColor,
) -> io::Result<()> {
    match underline {
        ResolvedUnderline::Curly => {
            payload.extend_from_slice(b"\x1b[4:3m");
            write_underline_color(payload, color)?;
        }
        ResolvedUnderline::Straight => payload.extend_from_slice(b"\x1b[4m"),
        ResolvedUnderline::Emphasis => payload.extend_from_slice(b"\x1b[1m"),
    }
    Ok(())
}

fn write_underline_color(payload: &mut Vec<u8>, color: ResolvedColor) -> io::Result<()> {
    match color {
        ResolvedColor::TrueColor(RgbColor { red, green, blue }) => {
            write!(payload, "\x1b[58;2;{red};{green};{blue}m")?;
        }
        ResolvedColor::Ansi256(index) => write!(payload, "\x1b[58;5;{index}m")?,
        ResolvedColor::Ansi16(_) => {}
    }
    Ok(())
}

const fn ansi16_code(index: u8, foreground: bool) -> u8 {
    match (foreground, index < 8) {
        (true, true) => 30 + index,
        (true, false) => 90 + (index - 8),
        (false, true) => 40 + index,
        (false, false) => 100 + (index - 8),
    }
}

const fn is_diagnostic(role: StyleRole) -> bool {
    matches!(
        role,
        StyleRole::Error | StyleRole::Warning | StyleRole::Information | StyleRole::Hint
    )
}

#[cfg(test)]
mod tests {
    use editor_types::{
        ColorDepth, StyleRole, TerminalCapabilities, TerminalFeatures, UnderlineStyle,
    };

    use super::DifferentialRenderer;
    use crate::{Cell, Framebuffer, Theme};

    fn capabilities() -> TerminalCapabilities {
        TerminalCapabilities {
            color_depth: ColorDepth::TrueColor,
            underline: UnderlineStyle::Curly,
            features: TerminalFeatures::default(),
        }
    }

    #[test]
    fn identical_frame_emits_no_output() {
        let mut renderer = DifferentialRenderer::new(Vec::new(), capabilities(), Theme::default());
        let frame = Framebuffer::new(2, 1);
        assert!(renderer.render(&frame).expect("first frame writes") > 0);
        let first_len = renderer.writer().len();
        assert_eq!(renderer.render(&frame).expect("same frame succeeds"), 0);
        assert_eq!(renderer.writer().len(), first_len);
    }

    #[test]
    fn small_edit_is_bounded_and_not_a_full_redraw() {
        let mut renderer = DifferentialRenderer::new(Vec::new(), capabilities(), Theme::default());
        let mut frame = Framebuffer::new(80, 24);
        renderer.render(&frame).expect("first frame writes");
        let start = renderer.writer().len();
        frame
            .set(
                40,
                12,
                Cell {
                    symbol: "x".to_owned(),
                    ..Cell::default()
                },
            )
            .expect("edit fits");
        let emitted = renderer.render(&frame).expect("edit writes");
        let delta = &renderer.writer()[start..];
        assert!(emitted < 128, "one-cell edit emitted {emitted} bytes");
        assert!(!delta.windows(4).any(|window| window == b"\x1b[2J"));
    }

    #[test]
    fn diagnostic_uses_curly_underline_when_supported() {
        let mut renderer = DifferentialRenderer::new(Vec::new(), capabilities(), Theme::default());
        let mut frame = Framebuffer::new(1, 1);
        frame
            .set(
                0,
                0,
                Cell {
                    symbol: "!".to_owned(),
                    foreground: StyleRole::Error,
                    ..Cell::default()
                },
            )
            .expect("cell fits");
        renderer.render(&frame).expect("frame writes");
        assert!(
            renderer
                .writer()
                .windows(5)
                .any(|window| window == b"\x1b[4:3")
        );
    }
}
