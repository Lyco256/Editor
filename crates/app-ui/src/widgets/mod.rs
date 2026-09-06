//! Reusable terminal UI widget boundary.
//!
//! This module keeps the shared drawing and command-palette helpers that the shell and editor
//! views need. The helpers are pure, deterministic, and framebuffer-oriented so the higher layers
//! stay free of I/O and terminal escape-sequence concerns.
#![allow(
    clippy::format_push_string,
    clippy::match_same_arms,
    clippy::missing_panics_doc
)]

use editor_types::{ColorDepth, CommandId, StyleRole};
use std::fmt::Write as _;
use terminal_backend::{Cell, Framebuffer, RgbColor, Theme, resolve_color};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rect {
    pub x: u16,
    pub y: u16,
    pub width: u16,
    pub height: u16,
}

impl Rect {
    #[must_use]
    pub const fn new(x: u16, y: u16, width: u16, height: u16) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    #[must_use]
    pub const fn right(self) -> u16 {
        self.x.saturating_add(self.width)
    }

    #[must_use]
    pub const fn bottom(self) -> u16 {
        self.y.saturating_add(self.height)
    }

    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.width == 0 || self.height == 0
    }

    #[must_use]
    pub fn contains(self, column: u16, row: u16) -> bool {
        column >= self.x && column < self.right() && row >= self.y && row < self.bottom()
    }

    #[must_use]
    pub fn inset(self, horizontal: u16, vertical: u16) -> Option<Self> {
        let width = self.width.checked_sub(horizontal.saturating_mul(2))?;
        let height = self.height.checked_sub(vertical.saturating_mul(2))?;
        Some(Self {
            x: self.x.saturating_add(horizontal),
            y: self.y.saturating_add(vertical),
            width,
            height,
        })
    }

    #[must_use]
    pub fn split_vertical(self, left_width: u16) -> (Self, Self) {
        let left_width = left_width.min(self.width);
        let right_width = self.width.saturating_sub(left_width);
        (
            Self::new(self.x, self.y, left_width, self.height),
            Self::new(
                self.x.saturating_add(left_width),
                self.y,
                right_width,
                self.height,
            ),
        )
    }

    #[must_use]
    pub fn split_horizontal(self, top_height: u16) -> (Self, Self) {
        let top_height = top_height.min(self.height);
        let bottom_height = self.height.saturating_sub(top_height);
        (
            Self::new(self.x, self.y, self.width, top_height),
            Self::new(
                self.x,
                self.y.saturating_add(top_height),
                self.width,
                bottom_height,
            ),
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandEntry {
    pub id: CommandId,
    pub label: String,
    pub detail: Option<String>,
    pub shortcut: Option<String>,
    pub available: bool,
}

impl CommandEntry {
    #[must_use]
    pub fn available(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: CommandId::new(id),
            label: label.into(),
            detail: None,
            shortcut: None,
            available: true,
        }
    }

    #[must_use]
    pub fn disabled(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: CommandId::new(id),
            label: label.into(),
            detail: None,
            shortcut: None,
            available: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandMatch {
    pub command: CommandEntry,
    pub score: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CommandPaletteState {
    commands: Vec<CommandEntry>,
    query: String,
    selection: usize,
}

/// Feature-neutral picker row. Stable ids let callers route acceptance without parsing labels.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PickerRow {
    pub id: String,
    pub label: String,
    pub detail: Option<String>,
    pub disabled: bool,
}

impl PickerRow {
    #[must_use]
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            detail: None,
            disabled: false,
        }
    }
}

/// Reusable modal/picker interaction contract shared by palette, quick-open and contextual UI.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct GenericPicker {
    pub title: String,
    pub query: String,
    rows: Vec<PickerRow>,
    selected: usize,
    pub scroll: usize,
}

impl GenericPicker {
    #[must_use]
    pub fn new(title: impl Into<String>, rows: Vec<PickerRow>) -> Self {
        Self {
            title: title.into(),
            query: String::new(),
            rows,
            selected: 0,
            scroll: 0,
        }
    }

    #[must_use]
    pub fn rows(&self) -> &[PickerRow] {
        &self.rows
    }

    #[must_use]
    pub fn visible_rows(&self) -> Vec<&PickerRow> {
        let query = self.query.to_lowercase();
        self.rows
            .iter()
            .filter(|row| {
                query.is_empty()
                    || row.label.to_lowercase().contains(&query)
                    || row.id.to_lowercase().contains(&query)
            })
            .collect()
    }

    #[must_use]
    pub fn selected(&self) -> Option<&PickerRow> {
        self.visible_rows().get(self.selected).copied()
    }

    pub fn set_query(&mut self, query: impl Into<String>) {
        self.query = query.into();
        self.selected = 0;
        self.scroll = 0;
    }
    pub fn move_selection(&mut self, delta: isize) {
        let len = self.visible_rows().len();
        if len == 0 {
            self.selected = 0;
            return;
        }
        let next = (isize::try_from(self.selected).unwrap_or(0) + delta)
            .rem_euclid(isize::try_from(len).unwrap_or(isize::MAX));
        self.selected = usize::try_from(next).unwrap_or(0);
    }
    pub fn select_index(&mut self, index: usize) {
        self.selected = index.min(self.visible_rows().len().saturating_sub(1));
    }
    #[must_use]
    pub fn accept(&self) -> Option<&str> {
        self.selected()
            .filter(|row| !row.disabled)
            .map(|row| row.id.as_str())
    }
    pub fn cancel(&mut self) {
        self.query.clear();
        self.selected = 0;
        self.scroll = 0;
    }
}

/// Single command registry consumed by both keybinding and palette dispatch.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CommandRegistry {
    entries: Vec<CommandEntry>,
}

impl CommandRegistry {
    pub fn register(&mut self, entry: CommandEntry) {
        if let Some(existing) = self.entries.iter_mut().find(|item| item.id == entry.id) {
            *existing = entry;
        } else {
            self.entries.push(entry);
        }
    }
    #[must_use]
    pub fn entries(&self) -> &[CommandEntry] {
        &self.entries
    }
    #[must_use]
    pub fn contains(&self, id: &CommandId) -> bool {
        self.entries.iter().any(|entry| &entry.id == id)
    }
    #[must_use]
    pub fn palette(&self) -> CommandPaletteState {
        CommandPaletteState::new(self.entries.clone())
    }
}

impl CommandPaletteState {
    #[must_use]
    pub fn new(commands: Vec<CommandEntry>) -> Self {
        Self {
            commands,
            query: String::new(),
            selection: 0,
        }
    }

    /// Returns the registered commands without exposing palette selection state.
    #[must_use]
    pub fn commands(&self) -> &[CommandEntry] {
        &self.commands
    }

    /// Materializes the registry view used by non-palette dispatchers.
    #[must_use]
    pub fn registry(&self) -> CommandRegistry {
        let mut registry = CommandRegistry::default();
        for command in &self.commands {
            registry.register(command.clone());
        }
        registry
    }

    #[must_use]
    pub fn query(&self) -> &str {
        &self.query
    }

    pub fn set_query(&mut self, query: impl Into<String>) {
        self.query = query.into();
        self.selection = 0;
    }

    pub fn push_query(&mut self, character: char) {
        self.query.push(character);
        self.selection = 0;
    }

    pub fn pop_query(&mut self) -> Option<char> {
        let popped = self.query.pop();
        self.selection = 0;
        popped
    }

    pub fn clear_query(&mut self) {
        self.query.clear();
        self.selection = 0;
    }

    #[must_use]
    pub fn visible_commands(&self) -> Vec<CommandMatch> {
        let query = self.query.to_lowercase();
        let mut matches = self
            .commands
            .iter()
            .filter_map(|command| {
                let score = command_score(command, &query)?;
                Some(CommandMatch {
                    command: command.clone(),
                    score,
                })
            })
            .collect::<Vec<_>>();
        matches.sort_by(|left, right| {
            left.command
                .available
                .cmp(&right.command.available)
                .reverse()
                .then_with(|| left.score.cmp(&right.score))
                .then_with(|| left.command.label.cmp(&right.command.label))
                .then_with(|| left.command.id.cmp(&right.command.id))
        });
        matches
    }

    #[must_use]
    pub fn selected(&self) -> Option<CommandMatch> {
        self.visible_commands().get(self.selection).cloned()
    }

    pub fn move_selection(&mut self, delta: isize) {
        let len = self.visible_commands().len();
        if len == 0 {
            self.selection = 0;
            return;
        }
        let len = isize::try_from(len).unwrap_or(isize::MAX);
        let selection = isize::try_from(self.selection).unwrap_or(0);
        let next = (selection + delta).rem_euclid(len);
        self.selection = usize::try_from(next).unwrap_or(0);
    }

    pub fn select_index(&mut self, index: usize) {
        let len = self.visible_commands().len();
        if len == 0 {
            self.selection = 0;
        } else {
            self.selection = index.min(len - 1);
        }
    }

    #[must_use]
    pub fn activate_selected(&self) -> Option<CommandId> {
        self.selected()
            .filter(|selection| selection.command.available)
            .map(|selection| selection.command.id)
    }
}

fn command_score(command: &CommandEntry, query: &str) -> Option<u16> {
    if query.is_empty() {
        return Some(0);
    }
    let label = command.label.to_lowercase();
    let detail = command.detail.as_deref().unwrap_or_default().to_lowercase();
    let shortcut = command
        .shortcut
        .as_deref()
        .unwrap_or_default()
        .to_lowercase();
    let id = command.id.as_str().to_lowercase();
    score_field(&label, query, 0)
        .or_else(|| score_field(&detail, query, 100))
        .or_else(|| score_field(&shortcut, query, 200))
        .or_else(|| score_field(&id, query, 300))
}

fn score_field(field: &str, query: &str, base: u16) -> Option<u16> {
    field.find(query).and_then(|index| {
        u16::try_from(index)
            .ok()
            .map(|offset| base.saturating_add(offset))
    })
}

#[must_use]
pub fn cell(
    symbol: impl Into<String>,
    foreground: StyleRole,
    background: StyleRole,
    bold: bool,
) -> Cell {
    Cell {
        symbol: symbol.into(),
        foreground,
        background,
        bold,
        continuation: false,
    }
}

pub fn fill_rect(
    frame: &mut Framebuffer,
    rect: Rect,
    symbol: &str,
    foreground: StyleRole,
    background: StyleRole,
) {
    for row in rect.y..rect.bottom() {
        for column in rect.x..rect.right() {
            let _ = frame.set(
                column,
                row,
                Cell {
                    symbol: symbol.to_owned(),
                    foreground,
                    background,
                    bold: false,
                    continuation: false,
                },
            );
        }
    }
}

#[must_use]
pub fn write_text(
    frame: &mut Framebuffer,
    column: u16,
    row: u16,
    text: &str,
    foreground: StyleRole,
    background: StyleRole,
) -> u16 {
    let mut current = column;
    let (_, frame_rows) = frame.size();
    if row >= frame_rows {
        return current;
    }
    for grapheme in terminal_backend::grapheme_clusters(text) {
        let width = terminal_backend::grapheme_width(grapheme);
        if width == 0 {
            continue;
        }
        let Ok(width_u16) = u16::try_from(width) else {
            break;
        };
        let (frame_columns, _) = frame.size();
        if current >= frame_columns || current.saturating_add(width_u16) > frame_columns {
            break;
        }
        if frame
            .set(
                current,
                row,
                Cell {
                    symbol: grapheme.to_owned(),
                    foreground,
                    background,
                    bold: false,
                    continuation: false,
                },
            )
            .is_err()
        {
            break;
        }
        current = current.saturating_add(width_u16);
    }
    current
}

pub fn draw_border(
    frame: &mut Framebuffer,
    rect: Rect,
    foreground: StyleRole,
    background: StyleRole,
) {
    if rect.width == 0 || rect.height == 0 {
        return;
    }
    let horizontal = "─";
    let vertical = "│";
    let top_left = "┌";
    let top_right = "┐";
    let bottom_left = "└";
    let bottom_right = "┘";
    let last_column = rect.right().saturating_sub(1);
    let last_row = rect.bottom().saturating_sub(1);
    let _ = frame.set(
        rect.x,
        rect.y,
        cell(top_left, foreground, background, false),
    );
    let _ = frame.set(
        last_column,
        rect.y,
        cell(top_right, foreground, background, false),
    );
    let _ = frame.set(
        rect.x,
        last_row,
        cell(bottom_left, foreground, background, false),
    );
    let _ = frame.set(
        last_column,
        last_row,
        cell(bottom_right, foreground, background, false),
    );
    if rect.width > 2 {
        for column in rect.x.saturating_add(1)..last_column {
            let _ = frame.set(
                column,
                rect.y,
                cell(horizontal, foreground, background, false),
            );
            let _ = frame.set(
                column,
                last_row,
                cell(horizontal, foreground, background, false),
            );
        }
    }
    if rect.height > 2 {
        for row in rect.y.saturating_add(1)..last_row {
            let _ = frame.set(rect.x, row, cell(vertical, foreground, background, false));
            let _ = frame.set(
                last_column,
                row,
                cell(vertical, foreground, background, false),
            );
        }
    }
}

#[must_use]
pub fn frame_snapshot(frame: &Framebuffer) -> String {
    let (columns, rows) = frame.size();
    let mut output = String::new();

    let _ = writeln!(output, "[{columns}x{rows}]");
    for row in 0..rows {
        let _ = write!(output, "{row:02}|");
        for column in 0..columns {
            let cell = frame.get(column, row).expect("frame coordinates are valid");
            if cell.continuation {
                output.push('⟫');
                continue;
            }
            let symbol = escape_symbol(&cell.symbol);
            if is_default_style(cell) {
                output.push_str(&symbol);
            } else {
                output.push('<');
                output.push_str(role_name(cell.foreground));
                output.push('/');
                output.push_str(role_name(cell.background));
                if cell.bold {
                    output.push('!');
                }
                output.push('>');
                output.push_str(&symbol);
                output.push_str("</>");
            }
        }
        if row + 1 != rows {
            output.push('\n');
        }
    }
    output
}

#[must_use]
pub fn semantic_palette_snapshot(depth: ColorDepth) -> String {
    let theme = Theme::default();
    let roles = [
        StyleRole::EditorText,
        StyleRole::Selection,
        StyleRole::CurrentLine,
        StyleRole::LineNumber,
        StyleRole::Gutter,
        StyleRole::SearchMatch,
        StyleRole::GitAdded,
        StyleRole::GitModified,
        StyleRole::GitDeleted,
    ];
    let mut output = String::new();
    output.push_str(match depth {
        ColorDepth::TrueColor => "truecolor\n",
        ColorDepth::Color256 => "color256\n",
        ColorDepth::Color16 => "color16\n",
    });
    for role in roles {
        let color = theme.color(role);
        let resolved = resolve_color(color, depth);
        let rendered = match resolved {
            terminal_backend::ResolvedColor::TrueColor(RgbColor { red, green, blue }) => {
                format!("rgb({red},{green},{blue})")
            }
            terminal_backend::ResolvedColor::Ansi256(index) => format!("ansi256({index})"),
            terminal_backend::ResolvedColor::Ansi16(index) => format!("ansi16({index})"),
        };
        output.push_str(role_name(role));
        output.push_str(" => ");
        output.push_str(&rendered);
        output.push('\n');
    }
    output
}

fn escape_symbol(symbol: &str) -> String {
    match symbol {
        " " | "" => "·".to_owned(),
        "\t" => "⇥".to_owned(),
        other => other.to_owned(),
    }
}

fn is_default_style(cell: &Cell) -> bool {
    !cell.bold
        && cell.foreground == StyleRole::EditorText
        && cell.background == StyleRole::EditorBackground
}

fn role_name(role: StyleRole) -> &'static str {
    match role {
        StyleRole::EditorText => "EditorText",
        StyleRole::EditorBackground => "EditorBackground",
        StyleRole::Selection => "Selection",
        StyleRole::CurrentLine => "CurrentLine",
        StyleRole::LineNumber => "LineNumber",
        StyleRole::Gutter => "Gutter",
        StyleRole::StatusBar => "StatusBar",
        StyleRole::Panel => "Panel",
        StyleRole::Error => "Error",
        StyleRole::Warning => "Warning",
        StyleRole::Information => "Information",
        StyleRole::Hint => "Hint",
        StyleRole::GitAdded => "GitAdded",
        StyleRole::GitModified => "GitModified",
        StyleRole::GitDeleted => "GitDeleted",
        StyleRole::SearchMatch => "SearchMatch",
        StyleRole::SyntaxKeyword => "SyntaxKeyword",
        StyleRole::SyntaxString => "SyntaxString",
        StyleRole::SyntaxComment => "SyntaxComment",
        StyleRole::SemanticType => "SemanticType",
        StyleRole::CurrentLineBackground => "CurrentLineBackground",
        StyleRole::SelectionForeground => "SelectionForeground",
        StyleRole::SelectionBackground => "SelectionBackground",
        StyleRole::SecondaryCursorForeground => "SecondaryCursorForeground",
        StyleRole::SecondaryCursorBackground => "SecondaryCursorBackground",
        StyleRole::LineNumberActive => "LineNumberActive",
        StyleRole::SplitSeparator => "SplitSeparator",
        StyleRole::MenuForeground => "MenuForeground",
        StyleRole::MenuBackground => "MenuBackground",
        StyleRole::MenuSelectedForeground => "MenuSelectedForeground",
        StyleRole::MenuSelectedBackground => "MenuSelectedBackground",
        StyleRole::ExplorerForeground => "ExplorerForeground",
        StyleRole::ExplorerBackground => "ExplorerBackground",
        StyleRole::ExplorerDirectory => "ExplorerDirectory",
        StyleRole::ExplorerSelectedForeground => "ExplorerSelectedForeground",
        StyleRole::ExplorerSelectedBackground => "ExplorerSelectedBackground",
        StyleRole::TabForeground => "TabForeground",
        StyleRole::TabBackground => "TabBackground",
        StyleRole::TabActiveForeground => "TabActiveForeground",
        StyleRole::TabActiveBackground => "TabActiveBackground",
        StyleRole::TabPreviewForeground => "TabPreviewForeground",
        StyleRole::TabPinnedForeground => "TabPinnedForeground",
        StyleRole::PanelForeground => "PanelForeground",
        StyleRole::PanelBackground => "PanelBackground",
        StyleRole::PanelTitle => "PanelTitle",
        StyleRole::InputForeground => "InputForeground",
        StyleRole::InputBackground => "InputBackground",
        StyleRole::StatusForeground => "StatusForeground",
        StyleRole::StatusBackground => "StatusBackground",
        StyleRole::Border => "Border",
    }
}

#[cfg(test)]
mod tests {
    use super::{CommandEntry, CommandPaletteState, frame_snapshot, semantic_palette_snapshot};
    use editor_types::{ColorDepth, CommandId, StyleRole};
    use terminal_backend::{Cell, Framebuffer};

    #[test]
    fn palette_filters_and_keeps_disabled_commands_visible() {
        let mut palette = CommandPaletteState::new(vec![
            CommandEntry::available("editor.open", "Open File"),
            CommandEntry::disabled("editor.save", "Save File"),
            CommandEntry::available("editor.close", "Close File"),
        ]);
        palette.set_query("file");
        let visible = palette.visible_commands();
        assert_eq!(visible.len(), 3);
        assert_eq!(
            palette.selected().expect("selection").command.id,
            CommandId::new("editor.open")
        );
        palette.move_selection(1);
        assert_eq!(
            palette.selected().expect("selection").command.id,
            CommandId::new("editor.close")
        );
        assert_eq!(
            palette.activate_selected(),
            Some(CommandId::new("editor.close"))
        );
    }

    #[test]
    fn frame_snapshot_records_semantic_styles() {
        let mut frame = Framebuffer::new(3, 1);
        let _ = frame.set(
            0,
            0,
            Cell {
                symbol: "A".to_owned(),
                foreground: StyleRole::Selection,
                background: StyleRole::EditorBackground,
                bold: false,
                continuation: false,
            },
        );
        assert!(frame_snapshot(&frame).contains("Selection/EditorBackground"));
    }

    #[test]
    fn palette_snapshot_captures_color_depth() {
        let truecolor = semantic_palette_snapshot(ColorDepth::TrueColor);
        let reduced = semantic_palette_snapshot(ColorDepth::Color16);
        assert_ne!(truecolor, reduced);
        assert!(truecolor.contains("rgb("));
        assert!(reduced.contains("ansi16("));
    }
}
