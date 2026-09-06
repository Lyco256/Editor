//! Application shell view boundary.
//!
//! The shell assembles the persistent Explorer, tab strip, split editor tree, bottom panel,
//! command palette, and status bar into a single deterministic terminal layout. It turns state into
//! framebuffer output and normalized UI actions without performing any external I/O.
#![allow(
    clippy::bool_to_int_with_if,
    clippy::comparison_chain,
    clippy::if_not_else,
    clippy::large_enum_variant,
    clippy::manual_clamp,
    clippy::match_same_arms,
    clippy::uninlined_format_args,
    clippy::self_only_used_in_recursion
)]

use editor_types::{
    InputEvent, KeyCode, KeyEvent, Modifier, MouseAction, MouseButton, MouseEvent, ScreenCell,
    StyleRole, TerminalCapabilities,
};
use terminal_backend::{Framebuffer, truncate_display};

use crate::{
    editor::{EditorStatusData, EditorViewportState},
    widgets::{CommandPaletteState, Rect, cell, draw_border, fill_rect, write_text},
};

// Ownership-safe facades for the workbench slices. The implementation remains in this stable
// facade while the modules provide narrow paths for feature-specific tests and future extraction.
pub mod explorer;
pub mod geometry;
pub mod menu;
pub mod panes;
pub mod tabs;
pub mod workbench;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShellFocus {
    Menu,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TabDisposition {
    Preview,
    Open,
    Pinned,
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
        self.render_with_tabs(frame, rect, capabilities, None);
    }

    fn render_with_tabs(
        &self,
        frame: &mut Framebuffer,
        rect: Rect,
        capabilities: TerminalCapabilities,
        tabs: Option<&[TabEntry]>,
    ) {
        match self {
            Self::Leaf(viewport) => {
                let content = if tabs.is_some() && rect.height > 1 {
                    let strip = Rect::new(rect.x, rect.y, rect.width, 1);
                    render_tab_entries(frame, strip, tabs.unwrap_or(&[]), false);
                    Rect::new(
                        rect.x,
                        rect.y.saturating_add(1),
                        rect.width,
                        rect.height - 1,
                    )
                } else {
                    rect
                };
                viewport.render(frame, content, capabilities);
            }
            Self::Split {
                axis,
                ratio_percent,
                first,
                second,
            } => {
                let inner = rect;
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
                        first.render_with_tabs(frame, left, capabilities, tabs);
                        second.render_with_tabs(frame, right, capabilities, tabs);
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
                        first.render_with_tabs(frame, top, capabilities, tabs);
                        second.render_with_tabs(frame, bottom, capabilities, tabs);
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
    pub disposition: TabDisposition,
}

#[derive(Debug, Clone)]
pub struct ExplorerEntry {
    pub depth: u8,
    pub label: String,
    pub active: bool,
    pub expanded: bool,
    pub is_directory: bool,
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
    pub menu: Option<Rect>,
    pub explorer: Option<Rect>,
    pub tabs: Option<Rect>,
    pub editor: Rect,
    pub bottom: Option<Rect>,
    pub status: Option<Rect>,
    pub palette: Option<Rect>,
    pub compact: bool,
}

/// Geometry for one editor group, including the tab strip and all viewport subregions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditorGroupLayout {
    pub group_id: u32,
    pub tabs: Option<Rect>,
    pub content: Rect,
    pub line_number: Rect,
    pub gutter: Rect,
    pub text: Rect,
    pub overview: Rect,
}

/// Exact visible Explorer row geometry and its source entry identity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExplorerRowLayout {
    pub rect: Rect,
    pub entry_index: Option<usize>,
    pub is_directory: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditorPointerRegion {
    Text,
    LineNumber,
    Gutter,
    Overview,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MenuEntry {
    pub label: &'static str,
    pub command: &'static str,
}

const FILE_MENU: &[MenuEntry] = &[
    MenuEntry {
        label: "New File",
        command: "workbench.newFile",
    },
    MenuEntry {
        label: "Open File",
        command: "workbench.openFile",
    },
    MenuEntry {
        label: "Open Folder",
        command: "workbench.openFolder",
    },
    MenuEntry {
        label: "Add Folder to Workspace",
        command: "workspace.addRoot",
    },
    MenuEntry {
        label: "Open Recent",
        command: "workbench.openRecent",
    },
    MenuEntry {
        label: "Save",
        command: "editor.save",
    },
    MenuEntry {
        label: "Save As",
        command: "editor.saveAs",
    },
    MenuEntry {
        label: "Reopen with Encoding",
        command: "editor.reopenEncoding",
    },
    MenuEntry {
        label: "Save with Encoding",
        command: "editor.saveEncoding",
    },
    MenuEntry {
        label: "Change End of Line Sequence",
        command: "editor.setEolLf",
    },
    MenuEntry {
        label: "Close Editor",
        command: "workbench.closeActiveEditor",
    },
    MenuEntry {
        label: "Close Other Editors",
        command: "workbench.closeOtherEditors",
    },
    MenuEntry {
        label: "Exit",
        command: "editor.quit",
    },
];
const EDIT_MENU: &[MenuEntry] = &[
    MenuEntry {
        label: "Undo",
        command: "editor.undo",
    },
    MenuEntry {
        label: "Redo",
        command: "editor.redo",
    },
    MenuEntry {
        label: "Cut",
        command: "editor.cut",
    },
    MenuEntry {
        label: "Copy",
        command: "editor.copy",
    },
    MenuEntry {
        label: "Paste",
        command: "editor.paste",
    },
    MenuEntry {
        label: "Find",
        command: "editor.find",
    },
    MenuEntry {
        label: "Replace",
        command: "editor.replace",
    },
    MenuEntry {
        label: "Find in Files",
        command: "workspace.search",
    },
    MenuEntry {
        label: "Replace in Files",
        command: "workspace.replace",
    },
];
const SELECTION_MENU: &[MenuEntry] = &[
    MenuEntry {
        label: "Select All",
        command: "editor.selectAll",
    },
    MenuEntry {
        label: "Select Line",
        command: "editor.selectLine",
    },
    MenuEntry {
        label: "Expand Selection",
        command: "editor.expandSelection",
    },
    MenuEntry {
        label: "Add Cursor Above",
        command: "editor.addCursorAbove",
    },
    MenuEntry {
        label: "Add Cursor Below",
        command: "editor.addCursorBelow",
    },
    MenuEntry {
        label: "Add Selection to Next Find Match",
        command: "editor.selectNextOccurrence",
    },
    MenuEntry {
        label: "Skip Current Selection and Select Next Match",
        command: "editor.skipNextOccurrence",
    },
    MenuEntry {
        label: "Select All Occurrences",
        command: "editor.selectAllOccurrences",
    },
    MenuEntry {
        label: "Collapse to Single Cursor",
        command: "editor.collapseCursors",
    },
];
const VIEW_MENU: &[MenuEntry] = &[
    MenuEntry {
        label: "Command Palette",
        command: "workbench.commandPalette",
    },
    MenuEntry {
        label: "Toggle Explorer",
        command: "workbench.toggleExplorer",
    },
    MenuEntry {
        label: "Problems",
        command: "workbench.showProblems",
    },
    MenuEntry {
        label: "Output",
        command: "workbench.showOutput",
    },
    MenuEntry {
        label: "Source Control",
        command: "workbench.showGit",
    },
    MenuEntry {
        label: "Split Editor Right",
        command: "workbench.splitVertical",
    },
    MenuEntry {
        label: "Split Editor Down",
        command: "workbench.splitHorizontal",
    },
    MenuEntry {
        label: "Close Editor Group",
        command: "workbench.closeSplit",
    },
    MenuEntry {
        label: "Focus Left Group",
        command: "workbench.focusLeft",
    },
    MenuEntry {
        label: "Focus Right Group",
        command: "workbench.focusRight",
    },
    MenuEntry {
        label: "Focus Group Above",
        command: "workbench.focusAbove",
    },
    MenuEntry {
        label: "Focus Group Below",
        command: "workbench.focusBelow",
    },
    MenuEntry {
        label: "Toggle Line Numbers",
        command: "workbench.toggleLineNumbers",
    },
];
const GO_MENU: &[MenuEntry] = &[
    MenuEntry {
        label: "Go to Line/Column",
        command: "editor.goToLine",
    },
    MenuEntry {
        label: "Go to Definition",
        command: "language.goToDefinition",
    },
    MenuEntry {
        label: "Go to Declaration",
        command: "language.goToDeclaration",
    },
    MenuEntry {
        label: "Go to Implementation",
        command: "language.goToImplementation",
    },
    MenuEntry {
        label: "Find References",
        command: "language.findReferences",
    },
    MenuEntry {
        label: "Next Problem",
        command: "workbench.nextProblem",
    },
    MenuEntry {
        label: "Previous Problem",
        command: "workbench.previousProblem",
    },
];
const HELP_MENU: &[MenuEntry] = &[
    MenuEntry {
        label: "Show Keyboard Shortcuts",
        command: "workbench.keyboardShortcuts",
    },
    MenuEntry {
        label: "About Editor",
        command: "workbench.about",
    },
];

#[must_use]
pub fn menu_entries(category: usize) -> &'static [MenuEntry] {
    match category {
        0 => FILE_MENU,
        1 => EDIT_MENU,
        2 => SELECTION_MENU,
        3 => VIEW_MENU,
        4 => GO_MENU,
        _ => HELP_MENU,
    }
}

/// Authoritative geometry snapshot consumed by rendering and pointer hit-testing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkbenchLayoutSnapshot {
    pub terminal: Rect,
    pub menu: Option<Rect>,
    pub explorer: Option<Rect>,
    pub tabs: Option<Rect>,
    pub editor: Rect,
    pub bottom: Option<Rect>,
    pub status: Option<Rect>,
    pub palette: Option<Rect>,
    pub tab_rects: Vec<Rect>,
    pub editor_panes: Vec<(u32, Rect)>,
    pub editor_groups: Vec<EditorGroupLayout>,
    pub group_tab_rects: Vec<(u32, usize, Rect)>,
    pub explorer_rows: Vec<ExplorerRowLayout>,
    pub active_menu: Option<Rect>,
    pub menu_category: usize,
    pub compact: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PointerTarget {
    Menu(usize),
    MenuItem {
        category: usize,
        index: usize,
    },
    Tab(usize),
    Explorer(usize),
    ExplorerRow {
        entry_index: Option<usize>,
        is_directory: bool,
        toggle: bool,
    },
    Editor {
        pane_id: u32,
        local: ScreenCell,
        region: EditorPointerRegion,
        text_origin: u16,
        text_width: u16,
    },
    Bottom(usize),
    Status,
    Palette(usize),
    Outside,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PointerEvent {
    pub target: PointerTarget,
    pub screen: ScreenCell,
    pub action: MouseAction,
    pub modifiers: editor_types::Modifiers,
    pub click_count: u8,
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
    pub menu_open: bool,
    pub menu_category: usize,
    pub menu_item: usize,
}

impl ShellState {
    #[must_use]
    pub fn layout_snapshot(&self, area: Rect) -> WorkbenchLayoutSnapshot {
        let layout = self.layout(area);
        let mut editor_panes = Vec::new();
        collect_pane_rects(&self.root, layout.editor, &mut editor_panes);
        let mut editor_groups = Vec::new();
        collect_group_layouts(&self.root, layout.editor, &mut editor_groups);
        let group_tab_rects: Vec<(u32, usize, Rect)> = editor_groups
            .iter()
            .flat_map(|group| {
                group
                    .tabs
                    .map(|tabs| {
                        self.tab_rects(tabs, layout.compact)
                            .into_iter()
                            .enumerate()
                            .map(move |(index, rect)| (group.group_id, index, rect))
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default()
            })
            .collect();
        let tab_rects = group_tab_rects.iter().map(|(_, _, rect)| *rect).collect();
        let explorer_rows = layout
            .explorer
            .map_or_else(Vec::new, |explorer| self.explorer_rows(explorer));
        WorkbenchLayoutSnapshot {
            terminal: area,
            menu: layout.menu,
            explorer: layout.explorer,
            tabs: layout.tabs,
            editor: layout.editor,
            bottom: layout.bottom,
            status: layout.status,
            palette: layout.palette,
            active_menu: if self.menu_open && !layout.compact {
                menu_popup_rect(area, self.menu_category)
            } else {
                None
            },
            menu_category: self.menu_category,
            tab_rects,
            editor_panes,
            editor_groups,
            group_tab_rects,
            explorer_rows,
            compact: layout.compact,
        }
    }

    #[must_use]
    #[allow(clippy::too_many_lines)]
    pub fn layout(&self, area: Rect) -> ShellLayout {
        let mut status_height = u16::from(area.height > 0);
        let mut menu_height = u16::from(area.height > 5);
        // Tabs are owned by each editor group and rendered inside its pane. They are not a
        // global strip, so the outer workbench never reserves a second shared row.
        let mut tabs_height = 0;
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
            .saturating_sub(status_height + menu_height + tabs_height + bottom_height);
        editor.width = editor.width.saturating_sub(explorer_width);
        if editor.width < 20 || editor.height < 4 {
            bottom_height = 0;
            editor = area;
            editor.height = editor
                .height
                .saturating_sub(status_height + menu_height + tabs_height);
            editor.width = editor.width.saturating_sub(explorer_width);
        }
        if editor.width < 20 || editor.height < 4 {
            explorer_width = 0;
            editor = area;
            editor.height = editor
                .height
                .saturating_sub(status_height + menu_height + tabs_height + bottom_height);
            editor.width = editor.width.saturating_sub(explorer_width);
        }
        if editor.width < 20 || editor.height < 4 {
            compact = true;
            menu_height = 0;
            tabs_height = 0;
            status_height = 1.min(area.height);
            editor = area;
            editor.height = editor.height.saturating_sub(status_height + bottom_height);
            editor.width = editor.width.saturating_sub(explorer_width);
        }
        let explorer = if explorer_width > 0 {
            Some(Rect::new(
                area.x,
                area.y + menu_height + tabs_height,
                explorer_width,
                editor.height,
            ))
        } else {
            None
        };
        let tabs = if tabs_height > 0 {
            Some(Rect::new(
                area.x,
                area.y + menu_height,
                area.width,
                tabs_height,
            ))
        } else {
            None
        };
        let editor_rect = Rect::new(
            area.x + explorer_width,
            area.y + menu_height + tabs_height,
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
            menu: (menu_height > 0).then_some(Rect::new(area.x, area.y, area.width, menu_height)),
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
        let snapshot = self.layout_snapshot(area);
        self.render_snapshot(frame, &snapshot, capabilities);
    }

    /// Renders exactly the regions described by an already computed layout snapshot.
    pub fn render_snapshot(
        &self,
        frame: &mut Framebuffer,
        snapshot: &WorkbenchLayoutSnapshot,
        capabilities: TerminalCapabilities,
    ) {
        let area = snapshot.terminal;
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
        if let Some(menu) = snapshot.menu {
            self.render_menu(frame, menu);
        }
        if let Some(explorer) = snapshot.explorer {
            self.render_explorer(frame, explorer, snapshot.compact);
        }
        self.root
            .render_with_tabs(frame, snapshot.editor, capabilities, Some(&self.tabs));
        if let Some(bottom) = snapshot.bottom {
            self.render_bottom_panel(frame, bottom, snapshot.compact);
        }
        if let Some(status) = snapshot.status {
            self.render_status_bar(frame, status, snapshot.compact);
        }
        if let Some(palette) = snapshot.palette {
            self.render_palette(frame, palette);
        }
        if let Some(menu_popup) = snapshot.active_menu {
            self.render_menu_popup(frame, menu_popup);
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
            (KeyCode::Function(10), _, _) => actions.push(ShellAction::FocusMenu),
            (KeyCode::Character('f' | 'e' | 's' | 'v' | 'g' | 'h'), false, false)
                if event.modifiers.contains(Modifier::Alt) =>
            {
                actions.push(ShellAction::FocusMenu);
            }
            (KeyCode::Left, _, _) if matches!(self.focus, ShellFocus::Menu) => {
                actions.push(ShellAction::MenuPrevious);
            }
            (KeyCode::Right, _, _) if matches!(self.focus, ShellFocus::Menu) => {
                actions.push(ShellAction::MenuNext);
            }
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
        let area = Rect::new(
            0,
            0,
            event.position.column.saturating_add(1),
            event.position.row.saturating_add(1),
        );
        self.dispatch_mouse_at(event, area)
    }

    pub fn dispatch_mouse_at(&mut self, event: MouseEvent, area: Rect) -> Vec<ShellAction> {
        match event.action {
            MouseAction::Down(MouseButton::Left) => {
                let hit = self.hit_test(area, event.position.column, event.position.row);
                match hit {
                    ShellHit::Menu(index) => {
                        vec![ShellAction::FocusMenu, ShellAction::MenuSelected(index)]
                    }
                    ShellHit::Tab(index) => vec![ShellAction::SelectTab(index)],
                    ShellHit::Explorer(index) => vec![ShellAction::ExplorerSelect(index)],
                    ShellHit::Bottom(index) => vec![ShellAction::BottomPanelSelect(index)],
                    ShellHit::Palette(index) => {
                        self.palette.select_index(index);
                        vec![ShellAction::PaletteSelected(index)]
                    }
                    ShellHit::Editor(pane_id) => vec![ShellAction::FocusPane(pane_id)],
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
}

fn tab_label(tab: &TabEntry, compact: bool) -> String {
    let base = if compact {
        compact_label(&tab.title, 14)
    } else {
        tab.title.clone()
    };
    let base = match tab.disposition {
        TabDisposition::Preview => format!("<{base}>"),
        TabDisposition::Pinned => format!("^{base}"),
        TabDisposition::Open => base,
    };
    let labeled = if tab.dirty { format!("*{base}") } else { base };
    if tab.closeable {
        format!("{labeled} x")
    } else {
        labeled
    }
}

fn render_tab_entries(frame: &mut Framebuffer, rect: Rect, tabs: &[TabEntry], compact: bool) {
    let mut column = rect.x;
    let available = rect.right();
    for tab in tabs {
        if column >= available {
            break;
        }
        let tab_text = tab_label(tab, compact);
        let width = terminal_backend::display_width(&tab_text).saturating_add(2);
        let style = if tab.active {
            (
                StyleRole::TabActiveForeground,
                StyleRole::TabActiveBackground,
                true,
            )
        } else {
            let foreground = match tab.disposition {
                TabDisposition::Preview => StyleRole::TabPreviewForeground,
                TabDisposition::Pinned => StyleRole::TabPinnedForeground,
                TabDisposition::Open => StyleRole::TabForeground,
            };
            (foreground, StyleRole::TabBackground, false)
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
        let end = column.saturating_add(u16::try_from(width).unwrap_or(u16::MAX));
        if end < available {
            let _ = frame.set(end, rect.y, cell(" ", style.0, style.1, style.2));
        }
        column = end.saturating_add(1);
    }
}

impl ShellState {
    #[allow(clippy::unused_self)]
    fn render_menu(&self, frame: &mut Framebuffer, rect: Rect) {
        fill_rect(
            frame,
            rect,
            " ",
            StyleRole::MenuForeground,
            StyleRole::MenuBackground,
        );
        if rect.width < 48 {
            let _ = write_text(
                frame,
                rect.x,
                rect.y,
                "Menu",
                StyleRole::MenuForeground,
                StyleRole::MenuBackground,
            );
            return;
        }
        let labels = ["File", "Edit", "Selection", "View", "Go", "Help"];
        let mut column = rect.x.saturating_add(1);
        for (index, label) in labels.into_iter().enumerate() {
            if column >= rect.right() {
                break;
            }
            let width = u16::try_from(terminal_backend::display_width(label)).unwrap_or(0);
            let selected = self.menu_open && self.menu_category == index;
            let _ = write_text(
                frame,
                column,
                rect.y,
                label,
                if selected {
                    StyleRole::MenuSelectedForeground
                } else {
                    StyleRole::MenuForeground
                },
                if selected {
                    StyleRole::MenuSelectedBackground
                } else {
                    StyleRole::MenuBackground
                },
            );
            column = column.saturating_add(width.saturating_add(2));
        }
    }

    fn render_menu_popup(&self, frame: &mut Framebuffer, rect: Rect) {
        draw_border(frame, rect, StyleRole::Border, StyleRole::MenuBackground);
        for (index, entry) in menu_entries(self.menu_category).iter().enumerate() {
            let Some(row) = rect
                .y
                .checked_add(u16::try_from(index + 1).unwrap_or(u16::MAX))
            else {
                break;
            };
            if row >= rect.bottom().saturating_sub(1) {
                break;
            }
            let available = self
                .palette
                .commands()
                .iter()
                .find(|command| command.id.as_str() == entry.command)
                .is_some_and(|command| command.available);
            let selected = self.menu_item == index;
            let foreground = if selected {
                StyleRole::MenuSelectedForeground
            } else if available {
                StyleRole::MenuForeground
            } else {
                StyleRole::Hint
            };
            let background = if selected {
                StyleRole::MenuSelectedBackground
            } else {
                StyleRole::MenuBackground
            };
            let _ = write_text(
                frame,
                rect.x.saturating_add(1),
                row,
                &compact_label(entry.label, usize::from(rect.width.saturating_sub(2))),
                foreground,
                background,
            );
        }
    }

    fn tab_rects(&self, rect: Rect, compact: bool) -> Vec<Rect> {
        let mut column = rect.x;
        let available = rect.right();
        let tabs = if self.tabs.is_empty() {
            vec![TabEntry {
                title: String::from("untitled"),
                dirty: false,
                active: true,
                closeable: false,
                disposition: TabDisposition::Open,
            }]
        } else {
            self.tabs.clone()
        };
        let mut result = Vec::new();
        for tab in tabs {
            if column >= available {
                break;
            }
            let tab_text = tab_label(&tab, compact);
            let width = u16::try_from(terminal_backend::display_width(&tab_text).saturating_add(2))
                .unwrap_or(u16::MAX);
            result.push(Rect::new(
                column,
                rect.y,
                width.min(available.saturating_sub(column)),
                1,
            ));
            column = column.saturating_add(width.saturating_add(1));
        }
        result
    }

    fn explorer_rows(&self, rect: Rect) -> Vec<ExplorerRowLayout> {
        let mut rows = Vec::new();
        let mut row = rect.y.saturating_add(1);
        for _ in &self.explorer.roots {
            if row >= rect.bottom() {
                break;
            }
            rows.push(ExplorerRowLayout {
                rect: Rect::new(rect.x, row, rect.width, 1),
                entry_index: None,
                is_directory: true,
            });
            row = row.saturating_add(1);
        }
        for (index, entry) in self.explorer.entries.iter().enumerate() {
            if row >= rect.bottom() {
                break;
            }
            rows.push(ExplorerRowLayout {
                rect: Rect::new(rect.x, row, rect.width, 1),
                entry_index: Some(index),
                is_directory: entry.is_directory,
            });
            row = row.saturating_add(1);
        }
        rows
    }

    fn render_explorer(&self, frame: &mut Framebuffer, rect: Rect, compact: bool) {
        draw_border(
            frame,
            rect,
            StyleRole::Border,
            StyleRole::ExplorerBackground,
        );
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
            StyleRole::ExplorerForeground,
            StyleRole::ExplorerBackground,
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
                &format!("[+] {label}"),
                StyleRole::ExplorerDirectory,
                StyleRole::ExplorerBackground,
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
            if entry.is_directory {
                label.push_str(if entry.expanded { "[-]" } else { "[+]" });
                label.push(' ');
                label.push_str(&entry.label);
                label.push('/');
            } else {
                label.push_str("    ");
                label.push_str(&entry.label);
            }
            let style = if entry.active {
                (
                    StyleRole::ExplorerSelectedForeground,
                    StyleRole::ExplorerSelectedBackground,
                    true,
                )
            } else {
                (
                    StyleRole::ExplorerForeground,
                    StyleRole::ExplorerBackground,
                    false,
                )
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

    fn hit_test(&self, area: Rect, column: u16, row: u16) -> ShellHit {
        let layout = self.layout(area);
        if let Some(menu) = layout.menu {
            if menu.contains(column, row) {
                let labels = ["File", "Edit", "Selection", "View", "Go", "Help"];
                let mut cursor = menu.x.saturating_add(1);
                for (index, label) in labels.iter().enumerate() {
                    let width = u16::try_from(terminal_backend::display_width(label)).unwrap_or(0);
                    if column >= cursor && column < cursor.saturating_add(width) {
                        return ShellHit::Menu(index);
                    }
                    cursor = cursor.saturating_add(width.saturating_add(2));
                }
                return ShellHit::Menu(0);
            }
        }
        if let Some(palette) = layout.palette {
            if palette.contains(column, row) {
                let index = usize::from(row.saturating_sub(palette.y).saturating_sub(1));
                return ShellHit::Palette(index);
            }
        }
        if let Some(tabs) = layout.tabs {
            if tabs.contains(column, row) {
                if let Some(index) = self
                    .tab_rects(tabs, layout.compact)
                    .iter()
                    .position(|rect| rect.contains(column, row))
                {
                    return ShellHit::Tab(index);
                }
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
            return self.editor_hit(&self.root, layout.editor, column, row);
        }
        ShellHit::None
    }

    fn editor_hit(&self, node: &PaneNode, rect: Rect, column: u16, row: u16) -> ShellHit {
        if !rect.contains(column, row) {
            return ShellHit::None;
        }
        match node {
            PaneNode::Leaf(viewport) => ShellHit::Editor(viewport.pane_id),
            PaneNode::Split {
                axis,
                ratio_percent,
                first,
                second,
            } => {
                let inner = rect;
                match axis {
                    SplitAxis::Vertical => {
                        let left_width = inner
                            .width
                            .saturating_mul(*ratio_percent)
                            .saturating_div(100)
                            .max(1);
                        let divider = inner.x.saturating_add(left_width);
                        if column < divider {
                            self.editor_hit(
                                first,
                                Rect::new(inner.x, inner.y, left_width, inner.height),
                                column,
                                row,
                            )
                        } else if column > divider {
                            self.editor_hit(
                                second,
                                Rect::new(
                                    divider.saturating_add(1),
                                    inner.y,
                                    inner.width.saturating_sub(left_width).saturating_sub(1),
                                    inner.height,
                                ),
                                column,
                                row,
                            )
                        } else {
                            ShellHit::None
                        }
                    }
                    SplitAxis::Horizontal => {
                        let top_height = inner
                            .height
                            .saturating_mul(*ratio_percent)
                            .saturating_div(100)
                            .max(1);
                        let divider = inner.y.saturating_add(top_height);
                        if row < divider {
                            self.editor_hit(
                                first,
                                Rect::new(inner.x, inner.y, inner.width, top_height),
                                column,
                                row,
                            )
                        } else if row > divider {
                            self.editor_hit(
                                second,
                                Rect::new(
                                    inner.x,
                                    divider.saturating_add(1),
                                    inner.width,
                                    inner.height.saturating_sub(top_height).saturating_sub(1),
                                ),
                                column,
                                row,
                            )
                        } else {
                            ShellHit::None
                        }
                    }
                }
            }
        }
    }
}

fn collect_group_layouts(node: &PaneNode, rect: Rect, output: &mut Vec<EditorGroupLayout>) {
    match node {
        PaneNode::Leaf(viewport) => {
            let tabs = (rect.height > 1).then_some(Rect::new(rect.x, rect.y, rect.width, 1));
            let content = tabs.map_or(rect, |strip| {
                Rect::new(
                    strip.x,
                    strip.y.saturating_add(1),
                    strip.width,
                    strip.height,
                )
            });
            let geometry = viewport.geometry(content);
            output.push(EditorGroupLayout {
                group_id: viewport.pane_id,
                tabs,
                content,
                line_number: geometry.line_number,
                gutter: geometry.gutter,
                text: geometry.text,
                overview: geometry.overview,
            });
        }
        PaneNode::Split {
            axis,
            ratio_percent,
            first,
            second,
        } => {
            let left_or_top = match axis {
                SplitAxis::Vertical => rect
                    .width
                    .saturating_mul(*ratio_percent)
                    .saturating_div(100)
                    .max(1),
                SplitAxis::Horizontal => rect
                    .height
                    .saturating_mul(*ratio_percent)
                    .saturating_div(100)
                    .max(1),
            };
            match axis {
                SplitAxis::Vertical => {
                    let divider = rect.x.saturating_add(left_or_top);
                    collect_group_layouts(
                        first,
                        Rect::new(rect.x, rect.y, left_or_top, rect.height),
                        output,
                    );
                    collect_group_layouts(
                        second,
                        Rect::new(
                            divider.saturating_add(1),
                            rect.y,
                            rect.width.saturating_sub(left_or_top).saturating_sub(1),
                            rect.height,
                        ),
                        output,
                    );
                }
                SplitAxis::Horizontal => {
                    let divider = rect.y.saturating_add(left_or_top);
                    collect_group_layouts(
                        first,
                        Rect::new(rect.x, rect.y, rect.width, left_or_top),
                        output,
                    );
                    collect_group_layouts(
                        second,
                        Rect::new(
                            rect.x,
                            divider.saturating_add(1),
                            rect.width,
                            rect.height.saturating_sub(left_or_top).saturating_sub(1),
                        ),
                        output,
                    );
                }
            }
        }
    }
}

impl WorkbenchLayoutSnapshot {
    #[must_use]
    pub fn pointer_target(&self, column: u16, row: u16) -> Option<PointerTarget> {
        if let Some(menu) = self.menu {
            if menu.contains(column, row) {
                let labels = ["File", "Edit", "Selection", "View", "Go", "Help"];
                let mut cursor = menu.x.saturating_add(1);
                for (index, label) in labels.iter().enumerate() {
                    let width = u16::try_from(terminal_backend::display_width(label)).unwrap_or(0);
                    let end = cursor.saturating_add(width);
                    if column >= cursor && column < end {
                        return Some(PointerTarget::Menu(index));
                    }
                    cursor = end.saturating_add(2);
                }
                return Some(PointerTarget::Menu(0));
            }
        }
        if let Some(popup) = self.active_menu {
            if popup.contains(column, row) {
                let index = usize::from(row.saturating_sub(popup.y).saturating_sub(1));
                return Some(PointerTarget::MenuItem {
                    category: self.menu_category,
                    index,
                });
            }
        }
        if let Some(palette) = self.palette {
            if palette.contains(column, row) {
                return Some(PointerTarget::Palette(usize::from(
                    row.saturating_sub(palette.y).saturating_sub(1),
                )));
            }
        }
        for (_, index, rect) in &self.group_tab_rects {
            if rect.contains(column, row) {
                return Some(PointerTarget::Tab(*index));
            }
        }
        if let Some(explorer) = self.explorer {
            if explorer.contains(column, row) {
                if let Some(visible) = self
                    .explorer_rows
                    .iter()
                    .find(|visible| visible.rect.contains(column, row))
                {
                    let toggle = visible.is_directory && column <= visible.rect.x.saturating_add(4);
                    return Some(PointerTarget::ExplorerRow {
                        entry_index: visible.entry_index,
                        is_directory: visible.is_directory,
                        toggle,
                    });
                }
                return Some(PointerTarget::Explorer(usize::from(
                    row.saturating_sub(explorer.y),
                )));
            }
        }
        if let Some(bottom) = self.bottom {
            if bottom.contains(column, row) {
                return Some(PointerTarget::Bottom(usize::from(
                    row.saturating_sub(bottom.y),
                )));
            }
        }
        if let Some(status) = self.status {
            if status.contains(column, row) {
                return Some(PointerTarget::Status);
            }
        }
        self.editor_groups
            .iter()
            .find(|group| group.content.contains(column, row))
            .map(|group| {
                let region = if group.text.contains(column, row) {
                    EditorPointerRegion::Text
                } else if group.line_number.contains(column, row) {
                    EditorPointerRegion::LineNumber
                } else if group.gutter.contains(column, row) {
                    EditorPointerRegion::Gutter
                } else {
                    EditorPointerRegion::Overview
                };
                PointerTarget::Editor {
                    pane_id: group.group_id,
                    local: ScreenCell {
                        column: column.saturating_sub(group.content.x),
                        row: row.saturating_sub(group.content.y),
                    },
                    region,
                    text_origin: group.text.x.saturating_sub(group.content.x),
                    text_width: group.text.width,
                }
            })
    }
}

fn collect_pane_rects(node: &PaneNode, rect: Rect, output: &mut Vec<(u32, Rect)>) {
    match node {
        PaneNode::Leaf(viewport) => output.push((viewport.pane_id, rect)),
        PaneNode::Split {
            axis,
            ratio_percent,
            first,
            second,
        } => {
            let inner = rect;
            match axis {
                SplitAxis::Vertical => {
                    let left_width = inner
                        .width
                        .saturating_mul(*ratio_percent)
                        .saturating_div(100)
                        .max(1);
                    let divider = inner.x.saturating_add(left_width);
                    collect_pane_rects(
                        first,
                        Rect::new(inner.x, inner.y, left_width, inner.height),
                        output,
                    );
                    collect_pane_rects(
                        second,
                        Rect::new(
                            divider.saturating_add(1),
                            inner.y,
                            inner.width.saturating_sub(left_width).saturating_sub(1),
                            inner.height,
                        ),
                        output,
                    );
                }
                SplitAxis::Horizontal => {
                    let top_height = inner
                        .height
                        .saturating_mul(*ratio_percent)
                        .saturating_div(100)
                        .max(1);
                    let divider = inner.y.saturating_add(top_height);
                    collect_pane_rects(
                        first,
                        Rect::new(inner.x, inner.y, inner.width, top_height),
                        output,
                    );
                    collect_pane_rects(
                        second,
                        Rect::new(
                            inner.x,
                            divider.saturating_add(1),
                            inner.width,
                            inner.height.saturating_sub(top_height).saturating_sub(1),
                        ),
                        output,
                    );
                }
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShellAction {
    FocusMenu,
    MenuNext,
    MenuPrevious,
    MenuSelected(usize),
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
    FocusPane(u32),
    FocusStatusBar,
    ClearFocus,
    DragSplit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ShellHit {
    None,
    Menu(usize),
    Tab(usize),
    Explorer(usize),
    Bottom(usize),
    Palette(usize),
    Editor(u32),
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

fn menu_popup_rect(area: Rect, category: usize) -> Option<Rect> {
    let entries = menu_entries(category);
    let width = entries
        .iter()
        .map(|entry| terminal_backend::display_width(entry.label))
        .max()
        .unwrap_or(8)
        .saturating_add(2)
        .min(usize::from(area.width));
    let height = entries
        .len()
        .saturating_add(2)
        .min(usize::from(area.height));
    if width < 4 || height < 3 {
        return None;
    }
    let labels = ["File", "Edit", "Selection", "View", "Go", "Help"];
    let offset = labels
        .iter()
        .take(category.min(labels.len().saturating_sub(1)))
        .map(|label| terminal_backend::display_width(label).saturating_add(2))
        .sum::<usize>()
        .saturating_add(1);
    let x = area
        .x
        .saturating_add(u16::try_from(offset).unwrap_or(u16::MAX))
        .min(
            area.right()
                .saturating_sub(u16::try_from(width).unwrap_or(u16::MAX)),
        );
    let menu = Rect::new(
        x,
        area.y.saturating_add(1),
        u16::try_from(width).unwrap_or(u16::MAX),
        u16::try_from(height).unwrap_or(u16::MAX),
    );
    Some(menu)
}

fn compact_label(text: &str, width: usize) -> String {
    truncate_display(text, width)
}

fn compact_status(text: &str, width: usize) -> String {
    if width < 4 {
        return compact_label(text, width);
    }
    text.split(" | ")
        .map(|part| compact_label(part, 20))
        .collect::<Vec<_>>()
        .join(" | ")
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
        PointerTarget, ShellAction, ShellFocus, ShellState, SplitAxis, TabDisposition, TabEntry,
        compact_label, menu_entries,
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
            pane_id: 0,
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
                diagnostics: Vec::new(),
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
            overview_whole_document: false,
            inlay_hints: Vec::new(),
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
                        is_directory: true,
                    },
                    ExplorerEntry {
                        depth: 1,
                        label: String::from("main.rs"),
                        active: false,
                        expanded: false,
                        is_directory: false,
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
                    disposition: TabDisposition::Open,
                },
                TabEntry {
                    title: String::from("lib.rs"),
                    dirty: false,
                    active: false,
                    closeable: true,
                    disposition: TabDisposition::Preview,
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
            menu_open: false,
            menu_category: 0,
            menu_item: 0,
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
            click_count: 1,
        });
        assert!(!mouse.is_empty());
    }

    #[test]
    fn shell_snapshot_covers_tabs_panels_and_palette() {
        let state = shell_state(&read_fixture("shell-layout.rs"));
        let snapshot = render_shell(&state, 120, 40);
        assert!(snapshot.starts_with("[120x40]"));
        assert!(snapshot.contains("MenuForeground/MenuBackground"));
        assert!(snapshot.contains("ExplorerDirectory/ExplorerBackground"));
        assert!(snapshot.contains("ExplorerSelectedForeground/ExplorerSelectedBackground"));
        assert!(snapshot.contains("TabPreviewForeground/TabBackground"));
        assert!(snapshot.contains("Panel/EditorBackground"));
    }

    #[test]
    fn compact_labels_truncate_cleanly() {
        assert_eq!(compact_label("command-palette", 8), "comma...");
    }

    #[test]
    fn resize_matrix_keeps_snapshot_regions_in_frame() {
        let state = shell_state("fn main() {}\n");
        for (width, height) in [
            (40, 10),
            (48, 12),
            (55, 12),
            (80, 24),
            (100, 30),
            (120, 40),
            (160, 50),
            (220, 60),
        ] {
            let area = Rect::new(0, 0, width, height);
            let snapshot = state.layout_snapshot(area);
            assert_eq!(snapshot.terminal, area);
            for region in [
                snapshot.menu,
                snapshot.explorer,
                snapshot.bottom,
                snapshot.status,
            ]
            .into_iter()
            .flatten()
            {
                assert!(region.right() <= width && region.bottom() <= height);
            }
            assert!(snapshot.editor.right() <= width && snapshot.editor.bottom() <= height);
            assert!(snapshot.editor_groups.iter().all(|group| {
                group.content.right() <= width && group.content.bottom() <= height
            }));
        }
    }

    #[test]
    fn authoritative_snapshot_hit_testing_uses_current_geometry() {
        let state = shell_state("fn main() {}\n");
        let first = state.layout_snapshot(Rect::new(0, 0, 80, 24));
        let second = state.layout_snapshot(Rect::new(0, 0, 160, 50));
        assert_ne!(first.terminal, second.terminal);
        let status = second.status.expect("status row");
        assert_eq!(
            second.pointer_target(status.x, status.y),
            Some(PointerTarget::Status)
        );
        let editor = second.editor_groups.first().expect("editor group");
        assert!(matches!(
            second.pointer_target(editor.text.x, editor.text.y),
            Some(PointerTarget::Editor { .. })
        ));
    }

    #[test]
    fn variable_width_tab_hit_testing_and_menu_labels_are_exact() {
        let mut state = shell_state("fn main() {}\n");
        state.menu_open = true;
        let snapshot = state.layout_snapshot(Rect::new(0, 0, 120, 40));
        assert_eq!(menu_entries(0)[0].label, "New File");
        assert_eq!(menu_entries(1)[0].label, "Undo");
        assert_eq!(menu_entries(2)[0].label, "Select All");
        assert_eq!(menu_entries(3)[0].label, "Command Palette");
        assert_eq!(menu_entries(4)[0].label, "Go to Line/Column");
        assert_eq!(menu_entries(5)[0].label, "Show Keyboard Shortcuts");
        let tab = snapshot.group_tab_rects.first().expect("tab geometry");
        let center = (tab.2.x + tab.2.width / 2, tab.2.y);
        assert_eq!(
            snapshot.pointer_target(center.0, center.1),
            Some(PointerTarget::Tab(tab.1))
        );
        assert!(snapshot.active_menu.is_some());
    }

    #[test]
    fn chrome_policy_and_grapheme_metrics_preserve_data_cells() {
        let state = shell_state("界🙂e\u{301}0123456789\n");
        let rendered = render_shell(&state, 120, 40);
        assert!(!rendered.contains('▌'));
        assert!(rendered.contains("│") || rendered.contains("─"));
        assert_eq!(terminal_backend::display_width("界🙂e\u{301}"), 5);
    }

    #[test]
    fn production_pointer_routing_has_no_fixed_coordinate_fallback() {
        let root = repo_root();
        let state_source = fs::read_to_string(root.join("src/app/state.rs")).expect("state source");
        let runtime_source =
            fs::read_to_string(root.join("src/app/runtime.rs")).expect("runtime source");
        for forbidden in [
            "Rect::new(0, 0, 120, 40)",
            "line_text.find(",
            "CharacterOffset(current + 1)",
        ] {
            assert!(
                !state_source.contains(forbidden),
                "state contains forbidden fallback: {forbidden}"
            );
            assert!(
                !runtime_source.contains(forbidden),
                "runtime contains forbidden fallback: {forbidden}"
            );
        }
    }
}
