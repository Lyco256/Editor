//! Application shell view boundary.
//!
//! The shell assembles the persistent Explorer, tab strip, split editor tree, bottom panel,
//! command palette, and status bar into a single deterministic terminal layout. It turns state into
//! framebuffer output and normalized UI actions without performing any external I/O.
#![allow(
    clippy::bool_to_int_with_if,
    clippy::if_not_else,
    clippy::large_enum_variant,
    clippy::manual_clamp,
    clippy::match_same_arms,
    clippy::uninlined_format_args
)]

use editor_types::{
    InputEvent, KeyCode, KeyEvent, Modifier, MouseAction, MouseButton, MouseEvent, StyleRole,
    TerminalCapabilities,
};
use terminal_backend::Framebuffer;

use crate::{
    editor::{EditorStatusData, EditorViewportState},
    widgets::{CommandPaletteState, Rect, cell, draw_border, fill_rect, write_text},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShellFocus {
    Explorer,
    Editor,
    BottomPanel,
    StatusBar,
    CommandPalette,
    Tabs,
    SplitHandle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SplitAxis {
    Horizontal,
    Vertical,
}

#[derive(Debug, Clone)]
pub enum PaneNode {
    Leaf(EditorViewportState),
    Split {
        axis: SplitAxis,
        ratio_percent: u16,
        first: Box<PaneNode>,
        second: Box<PaneNode>,
    },
}

impl PaneNode {
    #[must_use]
    pub fn leaf(viewport: EditorViewportState) -> Self {
        Self::Leaf(viewport)
    }

    #[must_use]
    pub fn split(axis: SplitAxis, ratio_percent: u16, first: PaneNode, second: PaneNode) -> Self {
        Self::Split {
            axis,
            ratio_percent: ratio_percent.min(90).max(10),
            first: Box::new(first),
            second: Box::new(second),
        }
    }

    pub fn render(&self, frame: &mut Framebuffer, rect: Rect, capabilities: TerminalCapabilities) {
        match self {
            Self::Leaf(viewport) => viewport.render(frame, rect, capabilities),
            Self::Split {
                axis,
                ratio_percent,
                first,
                second,
            } => {
                draw_border(frame, rect, StyleRole::Gutter, StyleRole::EditorBackground);
                let inner = rect.inset(1, 1).unwrap_or(rect);
                if inner.is_empty() {
                    return;
                }
                match axis {
                    SplitAxis::Vertical => {
                        let divider = inner.x.saturating_add(
                            inner
                                .width
                                .saturating_mul(*ratio_percent)
                                .saturating_div(100),
                        );
                        let left_width = divider.saturating_sub(inner.x).max(1);
                        let right_width = inner.width.saturating_sub(left_width).saturating_sub(1);
                        let left = Rect::new(inner.x, inner.y, left_width, inner.height);
                        let right = Rect::new(
                            divider.saturating_add(1),
                            inner.y,
                            right_width,
                            inner.height,
                        );
                        first.render(frame, left, capabilities);
                        second.render(frame, right, capabilities);
                        for row in inner.y..inner.bottom() {
                            let _ = frame.set(
                                divider,
                                row,
                                cell("│", StyleRole::Gutter, StyleRole::EditorBackground, false),
                            );
                        }
                    }
                    SplitAxis::Horizontal => {
                        let divider = inner.y.saturating_add(
                            inner
                                .height
                                .saturating_mul(*ratio_percent)
                                .saturating_div(100),
                        );
                        let top_height = divider.saturating_sub(inner.y).max(1);
                        let bottom_height =
                            inner.height.saturating_sub(top_height).saturating_sub(1);
                        let top = Rect::new(inner.x, inner.y, inner.width, top_height);
                        let bottom = Rect::new(
                            inner.x,
                            divider.saturating_add(1),
                            inner.width,
                            bottom_height,
                        );
                        first.render(frame, top, capabilities);
                        second.render(frame, bottom, capabilities);
                        for column in inner.x..inner.right() {
                            let _ = frame.set(
                                column,
                                divider,
                                cell("─", StyleRole::Gutter, StyleRole::EditorBackground, false),
                            );
                        }
                    }
                }
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct TabEntry {
    pub title: String,
    pub dirty: bool,
    pub active: bool,
    pub closeable: bool,
}

#[derive(Debug, Clone)]
pub struct ExplorerEntry {
    pub depth: u8,
    pub label: String,
    pub active: bool,
    pub expanded: bool,
}

#[derive(Debug, Clone)]
pub struct PanelEntry {
    pub label: String,
    pub detail: Option<String>,
    pub level: StyleRole,
}

#[derive(Debug, Clone)]
pub struct BottomPanelState {
    pub title: String,
    pub entries: Vec<PanelEntry>,
    pub visible: bool,
}

#[derive(Debug, Clone)]
pub struct ExplorerState {
    pub roots: Vec<String>,
    pub entries: Vec<ExplorerEntry>,
    pub visible: bool,
}

#[derive(Debug, Clone)]
pub struct ShellLayout {
    pub explorer: Option<Rect>,
    pub tabs: Option<Rect>,
    pub editor: Rect,
    pub bottom: Option<Rect>,
    pub status: Option<Rect>,
    pub palette: Option<Rect>,
    pub compact: bool,
}

#[derive(Debug, Clone)]
pub struct ShellState {
    pub explorer: ExplorerState,
    pub tabs: Vec<TabEntry>,
    pub root: PaneNode,
    pub bottom: BottomPanelState,
    pub palette: CommandPaletteState,
    pub focus: ShellFocus,
    pub status: EditorStatusData,
}

impl ShellState {
    #[must_use]
    pub fn layout(&self, area: Rect) -> ShellLayout {
        let mut status_height = u16::from(area.height > 0);
        let mut tabs_height = u16::from(area.height > 4);
        let mut bottom_height = if self.bottom.visible && area.height >= 16 {
            6.min(area.height / 3)
        } else {
            0
        };
        let mut explorer_width = if self.explorer.visible && area.width >= 60 {
            24.min(area.width / 3)
        } else {
            0
        };

        let mut compact = false;
        let mut editor = area;
        editor.height = editor
            .height
            .saturating_sub(status_height + tabs_height + bottom_height);
        editor.width = editor.width.saturating_sub(explorer_width);
        if editor.width < 20 || editor.height < 4 {
            bottom_height = 0;
            editor = area;
            editor.height = editor.height.saturating_sub(status_height + tabs_height);
            editor.width = editor.width.saturating_sub(explorer_width);
        }
        if editor.width < 20 || editor.height < 4 {
            explorer_width = 0;
            editor = area;
            editor.height = editor
                .height
                .saturating_sub(status_height + tabs_height + bottom_height);
            editor.width = editor.width.saturating_sub(explorer_width);
        }
        if editor.width < 20 || editor.height < 4 {
            compact = true;
            tabs_height = 0;
            status_height = 1.min(area.height);
            editor = area;
            editor.height = editor.height.saturating_sub(status_height + bottom_height);
            editor.width = editor.width.saturating_sub(explorer_width);
        }
        let explorer = if explorer_width > 0 {
            Some(Rect::new(
                area.x,
                area.y + tabs_height,
                explorer_width,
                editor.height,
            ))
        } else {
            None
        };
        let tabs = if tabs_height > 0 {
            Some(Rect::new(area.x, area.y, area.width, tabs_height))
        } else {
            None
        };
        let editor_rect = Rect::new(
            area.x + explorer_width,
            area.y + tabs_height,
            area.width.saturating_sub(explorer_width),
            editor.height,
        );
        let bottom = if bottom_height > 0 {
            Some(Rect::new(
                area.x + explorer_width,
                editor_rect.bottom(),
                area.width.saturating_sub(explorer_width),
                bottom_height,
            ))
        } else {
            None
        };
        let status = if status_height > 0 {
            Some(Rect::new(
                area.x,
                area.bottom().saturating_sub(status_height),
                area.width,
                status_height,
            ))
        } else {
            None
        };
        let palette = if matches!(self.focus, ShellFocus::CommandPalette)
            || !self.palette.query().is_empty()
        {
            palette_rect(area, compact)
        } else {
            None
        };
        ShellLayout {
            explorer,
            tabs,
            editor: editor_rect,
            bottom,
            status,
            palette,
            compact,
        }
    }

    pub fn render(&self, frame: &mut Framebuffer, area: Rect, capabilities: TerminalCapabilities) {
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
        let layout = self.layout(area);
        if let Some(tabs) = layout.tabs {
            self.render_tabs(frame, tabs, layout.compact);
        }
        if let Some(explorer) = layout.explorer {
            self.render_explorer(frame, explorer, layout.compact);
        }
        self.root.render(frame, layout.editor, capabilities);
        if let Some(bottom) = layout.bottom {
            self.render_bottom_panel(frame, bottom, layout.compact);
        }
        if let Some(status) = layout.status {
            self.render_status_bar(frame, status, layout.compact);
        }
        if let Some(palette) = layout.palette {
            self.render_palette(frame, palette);
        }
    }

    pub fn dispatch(&mut self, input: InputEvent) -> Vec<ShellAction> {
        match input {
            InputEvent::Key(event) => self.dispatch_key(event),
            InputEvent::Mouse(event) => self.dispatch_mouse(event),
            InputEvent::Paste(_) | InputEvent::Resize { .. } => Vec::new(),
        }
    }

    pub fn dispatch_key(&mut self, event: KeyEvent) -> Vec<ShellAction> {
        let mut actions = Vec::new();
        match (
            event.code,
            event.modifiers.contains(Modifier::Control),
            event.modifiers.contains(Modifier::Shift),
        ) {
            (KeyCode::Character('p'), true, false) => actions.push(ShellAction::ShowPalette),
            (KeyCode::Character('b'), true, false) => actions.push(ShellAction::ToggleExplorer),
            (KeyCode::Character('j'), true, false) => actions.push(ShellAction::ToggleBottomPanel),
            (KeyCode::Tab, true, false) => actions.push(ShellAction::SelectNextTab),
            (KeyCode::Tab, true, true) => actions.push(ShellAction::SelectPreviousTab),
            (KeyCode::Enter, _, _) if matches!(self.focus, ShellFocus::CommandPalette) => {
                if let Some(command) = self.palette.activate_selected() {
                    actions.push(ShellAction::RunCommand(command));
                }
            }
            (KeyCode::Escape, _, _) => {
                if self.palette.query().is_empty() {
                    actions.push(ShellAction::ClearFocus);
                } else {
                    self.palette.clear_query();
                    actions.push(ShellAction::HidePalette);
                }
            }
            (KeyCode::Up, _, _) if matches!(self.focus, ShellFocus::CommandPalette) => {
                self.palette.move_selection(-1);
                actions.push(ShellAction::PaletteMoved(-1));
            }
            (KeyCode::Down, _, _) if matches!(self.focus, ShellFocus::CommandPalette) => {
                self.palette.move_selection(1);
                actions.push(ShellAction::PaletteMoved(1));
            }
            (KeyCode::Character(character), false, false)
                if matches!(self.focus, ShellFocus::CommandPalette) =>
            {
                self.palette.push_query(character);
                actions.push(ShellAction::PaletteQueryChanged(
                    self.palette.query().to_owned(),
                ));
            }
            (KeyCode::Backspace, _, _) if matches!(self.focus, ShellFocus::CommandPalette) => {
                self.palette.pop_query();
                actions.push(ShellAction::PaletteQueryChanged(
                    self.palette.query().to_owned(),
                ));
            }
            _ => {}
        }
        actions
    }

    pub fn dispatch_mouse(&mut self, event: MouseEvent) -> Vec<ShellAction> {
        match event.action {
            MouseAction::Down(MouseButton::Left) => {
                let hit = self.hit_test(event.position.column, event.position.row);
                match hit {
                    ShellHit::Tab(index) => vec![ShellAction::SelectTab(index)],
                    ShellHit::Explorer(index) => vec![ShellAction::ExplorerSelect(index)],
                    ShellHit::Bottom(index) => vec![ShellAction::BottomPanelSelect(index)],
                    ShellHit::Palette(index) => {
                        self.palette.select_index(index);
                        vec![ShellAction::PaletteSelected(index)]
                    }
                    ShellHit::Editor => vec![ShellAction::FocusEditor],
                    ShellHit::StatusBar => vec![ShellAction::FocusStatusBar],
                    ShellHit::None => Vec::new(),
                }
            }
            MouseAction::Down(MouseButton::Right) => vec![ShellAction::ShowPalette],
            MouseAction::Drag(MouseButton::Left) => vec![ShellAction::DragSplit],
            MouseAction::Down(_)
            | MouseAction::Drag(_)
            | MouseAction::Up(_)
            | MouseAction::Move
            | MouseAction::ScrollLines(_) => Vec::new(),
        }
    }

    fn render_tabs(&self, frame: &mut Framebuffer, rect: Rect, compact: bool) {
        let mut column = rect.x;
        let available = rect.right();
        let tabs = if self.tabs.is_empty() {
            vec![TabEntry {
                title: String::from("untitled"),
                dirty: false,
                active: true,
                closeable: false,
            }]
        } else {
            self.tabs.clone()
        };
        for tab in tabs {
            if column >= available {
                break;
            }
            let label = if compact {
                compact_label(&tab.title, 14)
            } else {
                tab.title.clone()
            };
            let tab_text = if tab.dirty {
                format!("*{label}")
            } else {
                label
            };
            let width = tab_text.chars().count().saturating_add(2);
            let style = if tab.active {
                (StyleRole::EditorText, StyleRole::StatusBar, true)
            } else {
                (StyleRole::EditorText, StyleRole::Panel, false)
            };
            let _ = frame.set(column, rect.y, cell(" ", style.0, style.1, style.2));
            let _ = write_text(
                frame,
                column.saturating_add(1),
                rect.y,
                &tab_text,
                style.0,
                style.1,
            );
            let end = column.saturating_add(u16::try_from(width).unwrap_or(0));
            if end < available {
                let _ = frame.set(end, rect.y, cell(" ", style.0, style.1, style.2));
            }
            column = end.saturating_add(1);
        }
    }

    fn render_explorer(&self, frame: &mut Framebuffer, rect: Rect, compact: bool) {
        draw_border(frame, rect, StyleRole::Gutter, StyleRole::EditorBackground);
        let title = if compact {
            "Explorer"
        } else {
            "Explorer / roots"
        };
        let _ = write_text(
            frame,
            rect.x.saturating_add(1),
            rect.y,
            title,
            StyleRole::Gutter,
            StyleRole::EditorBackground,
        );
        let mut row = rect.y.saturating_add(1);
        for root in &self.explorer.roots {
            if row >= rect.bottom() {
                break;
            }
            let label = if compact {
                compact_label(root, 18)
            } else {
                root.clone()
            };
            let _ = write_text(
                frame,
                rect.x.saturating_add(1),
                row,
                &format!("▸ {label}"),
                StyleRole::EditorText,
                StyleRole::EditorBackground,
            );
            row = row.saturating_add(1);
        }
        for entry in &self.explorer.entries {
            if row >= rect.bottom() {
                break;
            }
            let mut label = String::new();
            for _ in 0..entry.depth {
                label.push_str("  ");
            }
            label.push(if entry.expanded { '▾' } else { '▸' });
            label.push(' ');
            label.push_str(&entry.label);
            let style = if entry.active {
                (StyleRole::Selection, StyleRole::CurrentLine, true)
            } else {
                (StyleRole::EditorText, StyleRole::EditorBackground, false)
            };
            let _ = write_text(
                frame,
                rect.x.saturating_add(1),
                row,
                &compact_label(&label, usize::from(rect.width.saturating_sub(2))),
                style.0,
                style.1,
            );
            row = row.saturating_add(1);
        }
    }

    fn render_bottom_panel(&self, frame: &mut Framebuffer, rect: Rect, compact: bool) {
        draw_border(frame, rect, StyleRole::Gutter, StyleRole::EditorBackground);
        let title = if compact {
            compact_label(
                &self.bottom.title,
                usize::from(rect.width.saturating_sub(2)),
            )
        } else {
            self.bottom.title.clone()
        };
        let _ = write_text(
            frame,
            rect.x.saturating_add(1),
            rect.y,
            &title,
            StyleRole::Panel,
            StyleRole::EditorBackground,
        );
        let mut row = rect.y.saturating_add(1);
        for entry in &self.bottom.entries {
            if row >= rect.bottom() {
                break;
            }
            let detail = entry.detail.clone().unwrap_or_default();
            let text = if detail.is_empty() {
                entry.label.clone()
            } else {
                format!("{}: {}", entry.label, detail)
            };
            let _ = write_text(
                frame,
                rect.x.saturating_add(1),
                row,
                &compact_label(&text, usize::from(rect.width.saturating_sub(2))),
                entry.level,
                StyleRole::EditorBackground,
            );
            row = row.saturating_add(1);
        }
    }

    fn render_status_bar(&self, frame: &mut Framebuffer, rect: Rect, compact: bool) {
        fill_rect(
            frame,
            rect,
            " ",
            StyleRole::EditorText,
            StyleRole::StatusBar,
        );
        let mut text = self.status.summary();
        if compact {
            text = compact_status(&text, usize::from(rect.width));
        }
        let _ = write_text(
            frame,
            rect.x,
            rect.y,
            &compact_label(&text, usize::from(rect.width)),
            StyleRole::EditorText,
            StyleRole::StatusBar,
        );
    }

    fn render_palette(&self, frame: &mut Framebuffer, rect: Rect) {
        draw_border(frame, rect, StyleRole::Gutter, StyleRole::Panel);
        let query = self.palette.query().to_owned();
        let _ = write_text(
            frame,
            rect.x.saturating_add(1),
            rect.y,
            &format!("Command Palette: {query}"),
            StyleRole::Panel,
            StyleRole::EditorBackground,
        );
        let visible = self.palette.visible_commands();
        let mut row = rect.y.saturating_add(1);
        for (index, item) in visible.iter().enumerate() {
            if row >= rect.bottom() {
                break;
            }
            let prefix = if index == 0 { ">" } else { " " };
            let availability = if item.command.available {
                ""
            } else {
                " (disabled)"
            };
            let text = format!("{prefix} {}{}", item.command.label, availability);
            let _ = write_text(
                frame,
                rect.x.saturating_add(1),
                row,
                &compact_label(&text, usize::from(rect.width.saturating_sub(2))),
                if item.command.available {
                    StyleRole::EditorText
                } else {
                    StyleRole::Hint
                },
                StyleRole::Panel,
            );
            row = row.saturating_add(1);
        }
    }

    fn hit_test(&self, column: u16, row: u16) -> ShellHit {
        let layout = self.layout(Rect::new(0, 0, 120, 40));
        if let Some(palette) = layout.palette {
            if palette.contains(column, row) {
                let index = usize::from(row.saturating_sub(palette.y).saturating_sub(1));
                return ShellHit::Palette(index);
            }
        }
        if let Some(tabs) = layout.tabs {
            if tabs.contains(column, row) {
                let index = usize::from(column.saturating_sub(tabs.x) / 12);
                return ShellHit::Tab(index);
            }
        }
        if let Some(explorer) = layout.explorer {
            if explorer.contains(column, row) {
                let index = usize::from(row.saturating_sub(explorer.y));
                return ShellHit::Explorer(index);
            }
        }
        if let Some(bottom) = layout.bottom {
            if bottom.contains(column, row) {
                let index = usize::from(row.saturating_sub(bottom.y));
                return ShellHit::Bottom(index);
            }
        }
        if let Some(status) = layout.status {
            if status.contains(column, row) {
                return ShellHit::StatusBar;
            }
        }
        if layout.editor.contains(column, row) {
            return ShellHit::Editor;
        }
        ShellHit::None
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShellAction {
    ShowPalette,
    HidePalette,
    ToggleExplorer,
    ToggleBottomPanel,
    SelectNextTab,
    SelectPreviousTab,
    SelectTab(usize),
    ExplorerSelect(usize),
    BottomPanelSelect(usize),
    PaletteSelected(usize),
    PaletteMoved(isize),
    PaletteQueryChanged(String),
    RunCommand(editor_types::CommandId),
    FocusEditor,
    FocusStatusBar,
    ClearFocus,
    DragSplit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ShellHit {
    None,
    Tab(usize),
    Explorer(usize),
    Bottom(usize),
    Palette(usize),
    Editor,
    StatusBar,
}

fn palette_rect(area: Rect, compact: bool) -> Option<Rect> {
    let width = if compact {
        area.width.min(40)
    } else {
        area.width.min(56)
    };
    let height = if compact {
        area.height.min(8)
    } else {
        area.height.min(12)
    };
    if width < 20 || height < 4 {
        return None;
    }
    let x = area.x + (area.width.saturating_sub(width)) / 2;
    let y = area.y + (area.height.saturating_sub(height)) / 2;
    Some(Rect::new(x, y, width, height))
}

fn compact_label(text: &str, width: usize) -> String {
    if text.chars().count() <= width {
        return text.to_owned();
    }
    if width <= 1 {
        return text
            .chars()
            .next()
            .map_or_else(String::new, |character| character.to_string());
    }
    let mut output = String::new();
    for character in text.chars().take(width.saturating_sub(1)) {
        output.push(character);
    }
    output.push('…');
    output
}

fn compact_status(text: &str, width: usize) -> String {
    if width < 4 {
        return compact_label(text, width);
    }
    text.split(" | ")
        .map(|part| compact_label(part, 20))
        .collect::<Vec<_>>()
        .join(" • ")
}

#[cfg(test)]
mod tests {
    use std::{fs, path::PathBuf};

    use editor_core::{FoldRegion, Selection, SelectionSet, TextBuffer};
    use editor_types::{
        CharacterOffset, ColorDepth, KeyCode, KeyEvent, Modifiers, MouseAction, MouseButton,
        MouseEvent, ScreenCell, StyleRole, TerminalCapabilities,
    };

    use super::{
        BottomPanelState, EditorStatusData, ExplorerEntry, ExplorerState, PaneNode, PanelEntry,
        ShellAction, ShellFocus, ShellState, SplitAxis, TabEntry, compact_label,
    };
    use crate::{
        editor::{
            DiagnosticCounts, EditorViewportState, MarkerKind, MarkerSpan, SemanticMarkerSet,
            TextViewport,
        },
        widgets::{CommandEntry, CommandPaletteState, Rect, frame_snapshot},
    };
    use terminal_backend::Framebuffer;

    fn repo_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("crates dir")
            .parent()
            .expect("repo root")
            .to_path_buf()
    }

    fn snapshot_path(name: &str) -> PathBuf {
        repo_root()
            .join("tests/snapshots/ui-shell-editor")
            .join(format!("{name}.txt"))
    }

    fn fixture_path(name: &str) -> PathBuf {
        repo_root()
            .join("tests/fixtures/app-ui/shell-editor")
            .join(name)
    }

    fn read_fixture(name: &str) -> String {
        let path = fixture_path(name);
        fs::read_to_string(&path).expect("fixture read")
    }

    fn editor_state(text: &str) -> EditorViewportState {
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
        let mut folds = editor_core::FoldSet::default();
        folds.set_regions(vec![FoldRegion::new(1, 3).expect("fold")]);
        let mut collapsed = folds.clone();
        collapsed.toggle_at(1);
        EditorViewportState {
            title: String::from("main.rs"),
            snapshot: buffer.snapshot(),
            viewport: TextViewport::default(),
            selections,
            folds: collapsed,
            markers: SemanticMarkerSet {
                language: vec![MarkerSpan {
                    range: editor_core::TextRange {
                        start: CharacterOffset(0),
                        end: CharacterOffset(4),
                    },
                    kind: MarkerKind::Warning,
                }],
                git: vec![MarkerSpan {
                    range: editor_core::TextRange {
                        start: CharacterOffset(16),
                        end: CharacterOffset(22),
                    },
                    kind: MarkerKind::Modified,
                }],
                search: vec![MarkerSpan {
                    range: editor_core::TextRange {
                        start: CharacterOffset(24),
                        end: CharacterOffset(28),
                    },
                    kind: MarkerKind::Match,
                }],
            },
            syntax_spans: Vec::new(),
            search_matches: vec![editor_core::TextRange {
                start: CharacterOffset(24),
                end: CharacterOffset(28),
            }],
            bracket_matches: vec![editor_core::TextRange {
                start: CharacterOffset(9),
                end: CharacterOffset(10),
            }],
            status: EditorStatusData {
                file_name: String::from("main.rs"),
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
                    information: 1,
                    hint: 0,
                },
                language_server: String::from("running"),
                trust: String::from("trusted"),
            },
            show_line_numbers: true,
            tab_width: 4,
        }
    }

    fn shell_state(text: &str) -> ShellState {
        let viewport = editor_state(text);
        ShellState {
            explorer: ExplorerState {
                roots: vec![String::from("workspace")],
                entries: vec![
                    ExplorerEntry {
                        depth: 0,
                        label: String::from("src"),
                        active: true,
                        expanded: true,
                    },
                    ExplorerEntry {
                        depth: 1,
                        label: String::from("main.rs"),
                        active: false,
                        expanded: false,
                    },
                ],
                visible: true,
            },
            tabs: vec![
                TabEntry {
                    title: String::from("main.rs"),
                    dirty: true,
                    active: true,
                    closeable: true,
                },
                TabEntry {
                    title: String::from("lib.rs"),
                    dirty: false,
                    active: false,
                    closeable: true,
                },
            ],
            root: PaneNode::split(
                SplitAxis::Vertical,
                55,
                PaneNode::leaf(viewport.clone()),
                PaneNode::split(
                    SplitAxis::Horizontal,
                    50,
                    PaneNode::leaf(viewport.clone()),
                    PaneNode::leaf(viewport),
                ),
            ),
            bottom: BottomPanelState {
                title: String::from("Problems"),
                entries: vec![
                    PanelEntry {
                        label: String::from("error"),
                        detail: Some(String::from("missing semicolon")),
                        level: StyleRole::Error,
                    },
                    PanelEntry {
                        label: String::from("warning"),
                        detail: Some(String::from("unused import")),
                        level: StyleRole::Warning,
                    },
                ],
                visible: true,
            },
            palette: CommandPaletteState::new(vec![
                CommandEntry::available("editor.save", "Save File"),
                CommandEntry::disabled("editor.format", "Format Document"),
                CommandEntry::available("workbench.toggleExplorer", "Toggle Explorer"),
            ]),
            focus: ShellFocus::Editor,
            status: EditorStatusData {
                file_name: String::from("main.rs"),
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
                    information: 1,
                    hint: 0,
                },
                language_server: String::from("running"),
                trust: String::from("trusted"),
            },
        }
    }

    fn render_shell(state: &ShellState, width: u16, height: u16) -> String {
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

    #[test]
    fn shell_layout_compacts_when_space_is_tight() {
        let state = shell_state(&read_fixture("compact-shell.rs"));
        let layout = state.layout(Rect::new(0, 0, 80, 24));
        assert!(layout.editor.width > 0);
        assert!(layout.status.is_some());
    }

    #[test]
    fn shell_dispatches_palette_and_mouse_actions() {
        let mut state = shell_state("fn main() {}\n");
        let actions = state.dispatch_key(KeyEvent {
            code: KeyCode::Character('p'),
            modifiers: Modifiers::from_modifiers([editor_types::Modifier::Control]),
            repeat: false,
        });
        assert!(actions.contains(&ShellAction::ShowPalette));
        let mouse = state.dispatch_mouse(MouseEvent {
            position: ScreenCell { row: 0, column: 0 },
            action: MouseAction::Down(MouseButton::Left),
            modifiers: Modifiers::default(),
        });
        assert!(!mouse.is_empty());
    }

    #[test]
    fn shell_snapshot_covers_tabs_panels_and_palette() {
        let state = shell_state(&read_fixture("shell-layout.rs"));
        let snapshot = render_shell(&state, 120, 40);
        let expected = fs::read_to_string(snapshot_path("shell_120x40"))
            .unwrap_or_else(|error| panic!("missing snapshot shell_120x40: {error}\n{snapshot}"));
        assert_eq!(snapshot, expected);
    }

    #[test]
    fn compact_labels_truncate_cleanly() {
        assert_eq!(compact_label("command-palette", 8), "command…");
    }
}
