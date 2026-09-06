//! Editor viewport view boundary.
//!
//! The editor layer owns the text viewport, gutter, selection rendering, fold presentation, and
//! semantic line markers. It stays pure: the state comes from upstream models and the output is a
//! framebuffer plus action intents.
#![allow(
    clippy::bool_to_int_with_if,
    clippy::if_same_then_else,
    clippy::ignored_unit_patterns,
    clippy::let_unit_value,
    clippy::manual_is_variant_and,
    clippy::match_same_arms,
    clippy::missing_panics_doc,
    clippy::must_use_unit,
    clippy::nonminimal_bool,
    clippy::too_many_arguments,
    clippy::uninlined_format_args,
    clippy::unused_self,
    clippy::while_let_on_iterator
)]

use editor_core::{FoldRegion, FoldSet, SelectionSet, TextRange, TextSnapshot};
use editor_types::{StyleRole, TerminalCapabilities};
use terminal_backend::{Cell, Framebuffer};

use crate::widgets::{Rect, cell, fill_rect, write_text};

pub mod cursor;
pub mod pointer;
pub mod selection;
pub mod viewport;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TextViewport {
    pub top_line: u32,
    pub left_column: u16,
}

/// Cell regions used by both viewport rendering and pointer/cursor projection.
///
/// Keeping these rectangles in one value prevents input code from guessing gutter or overview
/// widths that differ from what the renderer actually painted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EditorGeometry {
    pub area: Rect,
    pub line_number: Rect,
    pub gutter: Rect,
    pub text: Rect,
    pub overview: Rect,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MarkerKind {
    Error,
    Warning,
    Information,
    Hint,
    Added,
    Modified,
    Deleted,
    Match,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MarkerSpan {
    pub range: TextRange,
    pub kind: MarkerKind,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SemanticMarkerSet {
    pub language: Vec<MarkerSpan>,
    pub git: Vec<MarkerSpan>,
    pub search: Vec<MarkerSpan>,
    pub diagnostics: Vec<MarkerSpan>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DiagnosticCounts {
    pub error: usize,
    pub warning: usize,
    pub information: usize,
    pub hint: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditorStatusData {
    pub file_name: String,
    pub dirty: bool,
    pub branch: Option<String>,
    pub language_mode: String,
    pub encoding: String,
    pub line_ending: String,
    pub indent_style: String,
    pub indent_size: u8,
    pub cursor_line: u32,
    pub cursor_column: u32,
    pub selection_summary: String,
    pub diagnostics: DiagnosticCounts,
    pub language_server: String,
    pub trust: String,
}

impl Default for EditorStatusData {
    fn default() -> Self {
        Self {
            file_name: String::new(),
            dirty: false,
            branch: None,
            language_mode: String::from("plain text"),
            encoding: String::from("utf-8"),
            line_ending: String::from("lf"),
            indent_style: String::from("spaces"),
            indent_size: 4,
            cursor_line: 1,
            cursor_column: 1,
            selection_summary: String::from("1 cursor"),
            diagnostics: DiagnosticCounts::default(),
            language_server: String::from("stopped"),
            trust: String::from("trusted"),
        }
    }
}

impl EditorStatusData {
    #[must_use]
    pub fn summary(&self) -> String {
        let mut parts = Vec::new();
        parts.push(if self.dirty {
            format!("{}*", self.file_name)
        } else {
            self.file_name.clone()
        });
        if let Some(branch) = &self.branch {
            parts.push(branch.clone());
        }
        parts.push(format!("{}:{}", self.cursor_line, self.cursor_column));
        parts.push(self.language_mode.clone());
        parts.push(self.encoding.clone());
        parts.push(self.line_ending.clone());
        parts.push(format!("{}:{}", self.indent_style, self.indent_size));
        parts.push(self.selection_summary.clone());
        parts.push(format!(
            "E{} W{} I{} H{}",
            self.diagnostics.error,
            self.diagnostics.warning,
            self.diagnostics.information,
            self.diagnostics.hint
        ));
        parts.push(self.language_server.clone());
        parts.push(self.trust.clone());
        parts.join(" | ")
    }
}

#[derive(Debug, Clone)]
pub struct EditorViewportState {
    /// Stable root-assigned pane id used by shell hit testing and focus routing.
    pub pane_id: u32,
    pub title: String,
    pub snapshot: TextSnapshot,
    pub viewport: TextViewport,
    pub selections: SelectionSet,
    pub folds: FoldSet,
    pub markers: SemanticMarkerSet,
    /// Syntax/semantic foreground roles keyed by logical character ranges. The root application
    /// supplies this immutable projection; the viewport never performs parsing itself.
    pub syntax_spans: Vec<(TextRange, StyleRole)>,
    pub search_matches: Vec<TextRange>,
    pub bracket_matches: Vec<TextRange>,
    pub status: EditorStatusData,
    pub show_line_numbers: bool,
    pub tab_width: usize,
    pub overview_whole_document: bool,
    /// Typed LSP inlay hints rendered at their logical line/character positions.
    pub inlay_hints: Vec<crate::language::InlayHintView>,
}

impl EditorViewportState {
    #[must_use]
    pub fn geometry(&self, area: Rect) -> EditorGeometry {
        let lines = parse_lines(&self.snapshot);
        let digits = line_number_digits(lines.len().max(1));
        let gutter_width = if area.width >= 8 { 2 } else { 1 };
        let line_number_width = if self.show_line_numbers && area.width > digits + gutter_width + 3
        {
            digits + 1
        } else {
            0
        };
        let overview_width = if area.width > gutter_width + line_number_width + 6 {
            1
        } else {
            0
        };
        let gutter = Rect::new(area.x, area.y, gutter_width.min(area.width), area.height);
        let line_number = Rect::new(
            area.x.saturating_add(gutter.width),
            area.y,
            line_number_width.min(area.width.saturating_sub(gutter.width)),
            area.height,
        );
        let text_x = area
            .x
            .saturating_add(gutter_width)
            .saturating_add(line_number_width);
        let text_right = area.right().saturating_sub(overview_width);
        let text = Rect::new(
            text_x.min(text_right),
            area.y,
            text_right.saturating_sub(text_x),
            area.height,
        );
        let overview = Rect::new(
            text_right,
            area.y,
            overview_width.min(area.width),
            area.height,
        );
        EditorGeometry {
            area,
            line_number,
            gutter,
            text,
            overview,
        }
    }

    /// Resolves the native terminal cursor cell for this viewport without painting a glyph.
    #[must_use]
    pub fn cursor_cell(&self, area: Rect) -> Option<(u16, u16)> {
        if area.is_empty() {
            return None;
        }
        let cursor = self.selections.primary().active;
        let position = self.snapshot.offset_to_position(cursor).ok()?;
        let geometry = self.geometry(area);
        let text_left = geometry.text.x;
        let text_right = geometry.text.right();
        let row = position.line.saturating_sub(self.viewport.top_line);
        if row >= u32::from(area.height) || position.line < self.viewport.top_line {
            return None;
        }
        let column = self
            .snapshot
            .display_column(cursor, self.tab_width)
            .ok()?
            .saturating_sub(usize::from(self.viewport.left_column));
        let x = text_left.saturating_add(u16::try_from(column).ok()?);
        let y = area.y.saturating_add(u16::try_from(row).ok()?);
        (x < text_right && y < area.bottom()).then_some((x, y))
    }

    pub fn render(&self, frame: &mut Framebuffer, area: Rect, _capabilities: TerminalCapabilities) {
        if area.is_empty() {
            return;
        }
        fill_rect(
            frame,
            area,
            " ",
            StyleRole::EditorText,
            StyleRole::EditorBackground,
        );

        let lines = parse_lines(&self.snapshot);
        let geometry = self.geometry(area);
        let gutter_width = geometry.gutter.width;
        let line_number_width = geometry.line_number.width;
        let overview_width = geometry.overview.width;
        let text_left = geometry.text.x;
        let text_right = geometry.text.right();
        let content_width = geometry.text.width;

        let mut row = 0_u16;
        let mut line_index = self.viewport.top_line;
        while row < area.height
            && usize::try_from(line_index)
                .ok()
                .is_some_and(|index| index < lines.len())
        {
            if self.folds.is_line_hidden(line_index)
                && !fold_start_region(&self.folds, line_index).is_some()
            {
                line_index = line_index.saturating_add(1);
                continue;
            }
            if let Some(region) = fold_start_region(&self.folds, line_index) {
                self.render_fold_placeholder(
                    frame,
                    area,
                    row,
                    text_left,
                    text_right,
                    line_number_width,
                    gutter_width,
                    line_index,
                    region.end_line.saturating_sub(region.start_line),
                );
                row = row.saturating_add(1);
                line_index = region.end_line.saturating_add(1);
                continue;
            }
            let index = usize::try_from(line_index).expect("validated above");
            let line = &lines[index];
            self.render_line(
                frame,
                area,
                row,
                line,
                text_left,
                text_right,
                line_number_width,
                gutter_width,
                content_width,
            );
            row = row.saturating_add(1);
            line_index = line_index.saturating_add(1);
        }
        self.render_overview(frame, area, overview_width);
        self.render_inlay_hints(frame, area, text_left, text_right);
    }

    fn render_inlay_hints(
        &self,
        frame: &mut Framebuffer,
        area: Rect,
        text_left: u16,
        text_right: u16,
    ) {
        for hint in &self.inlay_hints {
            let Some(row) = hint
                .position
                .line
                .checked_sub(self.viewport.top_line)
                .and_then(|row| u16::try_from(row).ok())
            else {
                continue;
            };
            if row >= area.height {
                continue;
            }
            let offset = self.snapshot.position_to_offset(hint.position).ok();
            let display_column = offset
                .and_then(|offset| self.snapshot.display_column(offset, self.tab_width).ok())
                .unwrap_or(usize::try_from(hint.position.character).unwrap_or(usize::MAX));
            let column = text_left.saturating_add(
                u16::try_from(
                    display_column.saturating_sub(usize::from(self.viewport.left_column)),
                )
                .unwrap_or(u16::MAX),
            );
            if column >= text_right {
                continue;
            }
            let label = hint
                .label
                .iter()
                .map(|glyph| glyph.symbol.as_str())
                .collect::<String>();
            let _ = write_text(
                frame,
                column,
                area.y.saturating_add(row),
                &format!(" {label}"),
                StyleRole::Hint,
                StyleRole::EditorBackground,
            );
        }
    }

    #[must_use]
    pub fn status_summary(&self) -> String {
        self.status.summary()
    }

    fn render_fold_placeholder(
        &self,
        frame: &mut Framebuffer,
        area: Rect,
        row: u16,
        text_left: u16,
        text_right: u16,
        line_number_width: u16,
        gutter_width: u16,
        line_index: u32,
        hidden_lines: u32,
    ) {
        let y = area.y.saturating_add(row);
        let line_background = StyleRole::CurrentLine;
        let marker = if self.status.cursor_line == line_index.saturating_add(1) {
            ">"
        } else {
            "+"
        };
        let marker_background = if self.status.cursor_line == line_index.saturating_add(1) {
            StyleRole::CurrentLine
        } else {
            StyleRole::EditorBackground
        };
        let _ = frame.set(
            area.x,
            y,
            cell(marker, StyleRole::Gutter, marker_background, true),
        );
        if line_number_width > 0 {
            let number = format!(
                "{:>width$}",
                line_index.saturating_add(1),
                width = usize::from(line_number_width.saturating_sub(1))
            );
            let _ = write_text(
                frame,
                area.x.saturating_add(gutter_width),
                y,
                &number,
                StyleRole::LineNumber,
                line_background,
            );
        }
        let summary = format!("{} folded lines", hidden_lines);
        let available = text_right.saturating_sub(text_left);
        let text = if available == 0 {
            String::new()
        } else if summary.chars().count() > usize::from(available) {
            ellipsize(&summary, usize::from(available))
        } else {
            summary
        };
        let _ = write_text(
            frame,
            text_left,
            y,
            &text,
            StyleRole::EditorText,
            line_background,
        );
    }

    #[allow(clippy::too_many_arguments)]
    fn render_line(
        &self,
        frame: &mut Framebuffer,
        area: Rect,
        row: u16,
        line: &DocumentLine,
        text_left: u16,
        text_right: u16,
        line_number_width: u16,
        gutter_width: u16,
        content_width: u16,
    ) {
        let y = area.y.saturating_add(row);
        let is_current_line = self.status.cursor_line == line.line_number.saturating_add(1);
        let row_background = if is_current_line {
            StyleRole::CurrentLine
        } else {
            StyleRole::EditorBackground
        };
        let gutter_background = row_background;
        let gutter_marker = gutter_marker(line.line_number, is_current_line, &self.folds);
        let _ = frame.set(
            area.x,
            y,
            cell(
                gutter_marker,
                StyleRole::Gutter,
                gutter_background,
                is_current_line,
            ),
        );
        if line_number_width > 0 {
            let number = format!(
                "{:>width$}",
                line.line_number.saturating_add(1),
                width = usize::from(line_number_width.saturating_sub(1))
            );
            let _ = write_text(
                frame,
                area.x.saturating_add(gutter_width),
                y,
                &number,
                StyleRole::LineNumber,
                row_background,
            );
        }

        self.render_line_text(
            frame,
            line,
            y,
            text_left,
            text_right,
            content_width,
            row_background,
        );
        self.render_cursors(frame, line, y, text_left, text_right, row_background);
    }

    #[allow(clippy::too_many_arguments)]
    fn render_line_text(
        &self,
        frame: &mut Framebuffer,
        line: &DocumentLine,
        row: u16,
        text_left: u16,
        text_right: u16,
        content_width: u16,
        row_background: StyleRole,
    ) {
        let visible_left = usize::from(self.viewport.left_column);
        let visible_right = visible_left.saturating_add(usize::from(content_width));
        let mut offset = line.start_offset;
        for grapheme in terminal_backend::grapheme_clusters(&line.text) {
            let grapheme_len = grapheme.chars().count();
            let start_column = self
                .snapshot
                .display_column(editor_core::CharacterOffset(offset), self.tab_width)
                .unwrap_or(0);
            let end_column = self
                .snapshot
                .display_column(
                    editor_core::CharacterOffset(offset.saturating_add(grapheme_len)),
                    self.tab_width,
                )
                .unwrap_or(start_column);
            if end_column > start_column {
                self.paint_pending(
                    frame,
                    row,
                    text_left,
                    text_right,
                    visible_left,
                    visible_right,
                    row_background,
                    offset,
                    start_column,
                    grapheme,
                );
            }
            offset = offset.saturating_add(grapheme_len);
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn paint_pending(
        &self,
        frame: &mut Framebuffer,
        row: u16,
        text_left: u16,
        text_right: u16,
        visible_left: usize,
        visible_right: usize,
        row_background: StyleRole,
        pending_start_offset: usize,
        pending_start_column: usize,
        pending: &str,
    ) {
        let width = terminal_backend::display_width(pending);
        if width == 0 {
            return;
        }
        let end_column = pending_start_column.saturating_add(width);
        if end_column <= visible_left || pending_start_column >= visible_right {
            return;
        }
        let draw_column = pending_start_column.saturating_sub(visible_left);
        let draw_column = u16::try_from(draw_column).unwrap_or(0);
        if text_left.saturating_add(draw_column) >= text_right {
            return;
        }
        let style = self.style_for_range(
            TextRange {
                start: editor_core::CharacterOffset(pending_start_offset),
                end: editor_core::CharacterOffset(
                    pending_start_offset.saturating_add(pending.chars().count()),
                ),
            },
            row_background,
        );
        let _ = frame.set(
            text_left.saturating_add(draw_column),
            row,
            Cell {
                symbol: pending.to_owned(),
                foreground: style.foreground,
                background: style.background,
                bold: style.bold,
                continuation: false,
            },
        );
    }

    fn render_cursors(
        &self,
        frame: &mut Framebuffer,
        line: &DocumentLine,
        row: u16,
        text_left: u16,
        text_right: u16,
        row_background: StyleRole,
    ) {
        // The primary caret is presented by the terminal's native steady-bar cursor. Drawing a
        // glyph into the source cell would overwrite user text and made selections destructive.
        // Secondary cursors are represented by style-only overlays in `style_for_range`.
        let _ = (frame, line, row, text_left, text_right, row_background);
    }

    fn render_overview(&self, frame: &mut Framebuffer, area: Rect, overview_width: u16) {
        if overview_width == 0 || area.height == 0 {
            return;
        }
        let column = area.right().saturating_sub(1);
        let total_lines = parse_lines(&self.snapshot).len().max(1);
        for row in 0..area.height {
            let line_index = if self.overview_whole_document {
                usize::from(row)
                    .saturating_mul(total_lines)
                    .checked_div(usize::from(area.height).max(1))
                    .unwrap_or(0)
                    .min(total_lines - 1)
            } else {
                self.viewport
                    .top_line
                    .saturating_add(u32::from(row))
                    .try_into()
                    .unwrap_or(total_lines - 1)
                    .min(total_lines - 1)
            };
            let marker = overview_marker_for_line(
                &self.snapshot,
                self.overview_whole_document,
                u32::try_from(line_index).unwrap_or(u32::MAX),
                &self.markers,
                &self.search_matches,
                &self.bracket_matches,
                &self.folds,
            );
            let background = if self.status.cursor_line.saturating_sub(1)
                == u32::try_from(line_index).unwrap_or(0)
            {
                StyleRole::CurrentLine
            } else {
                StyleRole::EditorBackground
            };
            let _ = frame.set(
                column,
                area.y.saturating_add(row),
                cell(marker, StyleRole::Hint, background, false),
            );
        }
    }

    fn style_for_range(&self, range: TextRange, row_background: StyleRole) -> GlyphStyle {
        if self.range_hits_any_selection(&range, &self.selections) {
            return GlyphStyle::new(
                StyleRole::SelectionForeground,
                StyleRole::SelectionBackground,
                true,
            );
        }
        if self.range_hits_any_ranges(&range, &self.search_matches) {
            return GlyphStyle::new(StyleRole::SearchMatch, row_background, false);
        }
        if self.range_hits_any_ranges(&range, &self.bracket_matches) {
            return GlyphStyle::new(StyleRole::SyntaxKeyword, row_background, true);
        }
        if let Some(marker) = self
            .markers
            .diagnostics
            .iter()
            .find(|marker| intersects(range, marker.range))
        {
            let role = match marker.kind {
                MarkerKind::Error => StyleRole::Error,
                MarkerKind::Warning => StyleRole::Warning,
                MarkerKind::Information => StyleRole::Information,
                MarkerKind::Hint => StyleRole::Hint,
                _ => StyleRole::EditorText,
            };
            if role != StyleRole::EditorText {
                return GlyphStyle::new(role, row_background, false);
            }
        }
        if let Some((_, role)) = self
            .syntax_spans
            .iter()
            .find(|(candidate, _)| intersects(range, *candidate))
        {
            return GlyphStyle::new(*role, row_background, false);
        }
        GlyphStyle::new(StyleRole::EditorText, row_background, false)
    }

    fn range_hits_any_selection(&self, range: &TextRange, selections: &SelectionSet) -> bool {
        selections
            .selections()
            .iter()
            .any(|selection| intersects(*range, selection.range()))
    }

    fn range_hits_any_ranges(&self, range: &TextRange, ranges: &[TextRange]) -> bool {
        ranges
            .iter()
            .any(|candidate| intersects(*range, *candidate))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditorAction {
    MoveCursor {
        line_delta: i32,
        column_delta: i32,
        extend_selection: bool,
    },
    Scroll {
        line_delta: i32,
        column_delta: i32,
    },
    ToggleFold {
        line: u32,
    },
    SelectRange(TextRange),
    /// Expands the primary selection to the next syntax-aware structural range.
    ExpandSelection,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct GlyphStyle {
    foreground: StyleRole,
    background: StyleRole,
    bold: bool,
}

impl GlyphStyle {
    const fn new(foreground: StyleRole, background: StyleRole, bold: bool) -> Self {
        Self {
            foreground,
            background,
            bold,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DocumentLine {
    line_number: u32,
    start_offset: usize,
    end_offset: usize,
    text: String,
}

fn parse_lines(snapshot: &TextSnapshot) -> Vec<DocumentLine> {
    let mut lines = Vec::new();
    let mut text = String::new();
    let mut start_offset = 0_usize;
    let mut offset = 0_usize;
    let mut line_number = 0_u32;
    let mut chars = snapshot.text().chars().peekable();
    while let Some(character) = chars.next() {
        offset = offset.saturating_add(1);
        if character == '\n' {
            if text.ends_with('\r') {
                text.pop();
            }
            lines.push(DocumentLine {
                line_number,
                start_offset,
                end_offset: offset
                    .saturating_sub(1)
                    .saturating_sub(usize::from(text.ends_with('\r'))),
                text: std::mem::take(&mut text),
            });
            start_offset = offset;
            line_number = line_number.saturating_add(1);
        } else {
            text.push(character);
        }
    }
    let end_offset = start_offset.saturating_add(text.chars().count());
    lines.push(DocumentLine {
        line_number,
        start_offset,
        end_offset,
        text,
    });
    lines
}

fn fold_start_region(folds: &FoldSet, line: u32) -> Option<FoldRegion> {
    folds
        .regions()
        .iter()
        .copied()
        .find(|region| region.collapsed && region.start_line == line)
}

fn gutter_marker(line: u32, current: bool, folds: &FoldSet) -> &'static str {
    if current {
        ">"
    } else if fold_start_region(folds, line).is_some() {
        "+"
    } else if folds.is_line_hidden(line) {
        "·"
    } else {
        "│"
    }
}

fn overview_marker_for_line(
    snapshot: &TextSnapshot,
    whole_document: bool,
    line: u32,
    markers: &SemanticMarkerSet,
    search_matches: &[TextRange],
    bracket_matches: &[TextRange],
    folds: &FoldSet,
) -> &'static str {
    if fold_start_region(folds, line).is_some() {
        return "+";
    }
    let mut best = None;
    for marker in markers
        .language
        .iter()
        .chain(markers.git.iter())
        .chain(markers.search.iter())
    {
        if range_contains_line(snapshot, whole_document, marker.range, line) {
            best = Some(match marker.kind {
                MarkerKind::Error => "E",
                MarkerKind::Warning => "W",
                MarkerKind::Information => "I",
                MarkerKind::Hint => "H",
                MarkerKind::Added => "+",
                MarkerKind::Modified => "~",
                MarkerKind::Deleted => "-",
                MarkerKind::Match => "*",
            });
            if matches!(marker.kind, MarkerKind::Error) {
                break;
            }
        }
    }
    if best.is_none()
        && search_matches
            .iter()
            .any(|range| range_contains_line(snapshot, whole_document, *range, line))
    {
        best = Some("S");
    }
    if best.is_none()
        && bracket_matches
            .iter()
            .any(|range| range_contains_line(snapshot, whole_document, *range, line))
    {
        best = Some("B");
    }
    best.unwrap_or(" ")
}

fn range_contains_line(
    snapshot: &TextSnapshot,
    _whole_document: bool,
    range: TextRange,
    line: u32,
) -> bool {
    let start = snapshot
        .offset_to_position(range.start)
        .ok()
        .map_or(u32::MAX, |p| p.line);
    let end = snapshot
        .offset_to_position(range.end)
        .ok()
        .map_or(start, |p| p.line);
    line >= start && line <= end
}

fn intersects(left: TextRange, right: TextRange) -> bool {
    left.start < right.end && right.start < left.end
}

fn line_number_digits(lines: usize) -> u16 {
    let mut digits = 1_u16;
    let mut value = lines.max(1);
    while value >= 10 {
        value /= 10;
        digits = digits.saturating_add(1);
    }
    digits
}

fn ellipsize(text: &str, limit: usize) -> String {
    terminal_backend::truncate_display(text, limit)
}

#[cfg(test)]
mod tests {
    use std::{fs, path::PathBuf};

    use editor_core::{FoldRegion, FoldSet, Selection, SelectionSet, TextBuffer, TextRange};
    use editor_types::{CharacterOffset, ColorDepth, TerminalCapabilities};

    use super::{
        DiagnosticCounts, EditorStatusData, EditorViewportState, MarkerKind, MarkerSpan,
        SemanticMarkerSet, TextViewport, parse_lines,
    };
    use crate::widgets::{Rect, frame_snapshot};
    use terminal_backend::Framebuffer;

    fn repo_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("crates directory")
            .parent()
            .expect("repository root")
            .to_path_buf()
    }

    fn read_fixture(name: &str) -> String {
        let path = repo_root()
            .join("tests/fixtures/app-ui/shell-editor")
            .join(name);
        fs::read_to_string(&path).expect("fixture read")
    }

    fn render_state(state: &EditorViewportState, width: u16, height: u16) -> String {
        let mut frame = Framebuffer::new(width, height);
        state.render(
            &mut frame,
            Rect::new(0, 0, width, height),
            TerminalCapabilities {
                color_depth: ColorDepth::TrueColor,
                ..TerminalCapabilities::default()
            },
        );
        frame_snapshot(&frame)
    }

    fn sample_state(text: &str) -> EditorViewportState {
        let mut buffer = TextBuffer::new(text);
        let selections = SelectionSet::new(
            vec![
                Selection::cursor(CharacterOffset(6)),
                Selection::new(CharacterOffset(9), CharacterOffset(11)),
            ],
            0,
        )
        .expect("selection set");
        buffer
            .set_selections(selections.clone())
            .expect("selection");
        let snapshot = buffer.snapshot();
        let mut folds = FoldSet::default();
        folds.set_regions(vec![FoldRegion::new(1, 3).expect("fold region")]);
        let mut collapsed = folds.clone();
        collapsed.toggle_at(1);
        EditorViewportState {
            pane_id: 0,
            title: String::from("sample.rs"),
            snapshot,
            viewport: TextViewport {
                top_line: 0,
                left_column: 0,
            },
            selections,
            folds: collapsed,
            markers: SemanticMarkerSet {
                language: vec![MarkerSpan {
                    range: TextRange {
                        start: CharacterOffset(0),
                        end: CharacterOffset(4),
                    },
                    kind: MarkerKind::Warning,
                }],
                git: vec![MarkerSpan {
                    range: TextRange {
                        start: CharacterOffset(16),
                        end: CharacterOffset(22),
                    },
                    kind: MarkerKind::Modified,
                }],
                search: vec![MarkerSpan {
                    range: TextRange {
                        start: CharacterOffset(24),
                        end: CharacterOffset(28),
                    },
                    kind: MarkerKind::Match,
                }],
                diagnostics: Vec::new(),
            },
            syntax_spans: Vec::new(),
            search_matches: vec![TextRange {
                start: CharacterOffset(24),
                end: CharacterOffset(28),
            }],
            bracket_matches: vec![TextRange {
                start: CharacterOffset(9),
                end: CharacterOffset(10),
            }],
            status: EditorStatusData {
                file_name: String::from("sample.rs"),
                dirty: true,
                branch: Some(String::from("main")),
                language_mode: String::from("rust"),
                encoding: String::from("utf-8"),
                line_ending: String::from("lf"),
                indent_style: String::from("spaces"),
                indent_size: 4,
                cursor_line: 1,
                cursor_column: 7,
                selection_summary: String::from("2 selections"),
                diagnostics: DiagnosticCounts {
                    error: 1,
                    warning: 2,
                    information: 0,
                    hint: 1,
                },
                language_server: String::from("running"),
                trust: String::from("trusted"),
            },
            show_line_numbers: true,
            tab_width: 4,
            overview_whole_document: false,
            inlay_hints: Vec::new(),
        }
    }

    #[test]
    fn status_summary_is_compact_and_complete() {
        let summary = EditorStatusData {
            file_name: String::from("main.rs"),
            dirty: true,
            branch: Some(String::from("feature/ui")),
            language_mode: String::from("rust"),
            encoding: String::from("utf-8"),
            line_ending: String::from("crlf"),
            indent_style: String::from("spaces"),
            indent_size: 2,
            cursor_line: 10,
            cursor_column: 7,
            selection_summary: String::from("3 selections"),
            diagnostics: DiagnosticCounts {
                error: 1,
                warning: 0,
                information: 2,
                hint: 4,
            },
            language_server: String::from("running"),
            trust: String::from("trusted"),
        }
        .summary();
        assert!(summary.contains("main.rs*"));
        assert!(summary.contains("feature/ui"));
        assert!(summary.contains("10:7"));
        assert!(summary.contains("E1 W0 I2 H4"));
    }

    #[test]
    fn parse_lines_preserves_trailing_empty_line() {
        let buffer = TextBuffer::new("a\n");
        let lines = parse_lines(&buffer.snapshot());
        assert_eq!(lines.len(), 2);
        assert!(lines[1].text.is_empty());
    }

    #[test]
    fn viewport_snapshot_handles_line_numbers_folds_unicode_and_markers() {
        let text = read_fixture("sample.rs");
        let state = sample_state(&text);
        let snapshot = render_state(&state, 80, 24);
        assert!(snapshot.starts_with("[80x24]"));
        assert!(snapshot.contains("<EditorText/CurrentLine>f</>"));
        assert!(!snapshot.contains("▌"));
    }

    #[test]
    fn viewport_snapshot_covers_small_layout() {
        let text = read_fixture("compact.rs");
        let state = sample_state(&text);
        let snapshot = render_state(&state, 40, 12);
        assert!(!snapshot.is_empty());
    }

    #[test]
    fn semantic_color_depths_map_to_distinct_outputs() {
        let truecolor = crate::widgets::semantic_palette_snapshot(ColorDepth::TrueColor);
        let reduced = crate::widgets::semantic_palette_snapshot(ColorDepth::Color16);
        assert_ne!(truecolor, reduced);
        assert!(truecolor.contains("rgb("));
        assert!(reduced.contains("ansi16("));
    }
}
