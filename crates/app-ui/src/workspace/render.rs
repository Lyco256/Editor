//! Workspace framebuffer rendering for Explorer, Quick Open, search, and trust views.

use editor_types::{StyleRole, TextRange};
use terminal_backend::{Cell, Framebuffer};

use super::{
    ExplorerLine, ExplorerState, QuickOpenCandidate, QuickOpenDisposition, QuickOpenState,
    SearchState, SearchStatus, WorkspaceEntryKind, WorkspacePrompt, WorkspaceTrustState,
    WorkspaceUiState,
};

/// # Panics
///
/// The helper indexes the framebuffer using coordinates returned by `Framebuffer::size`, so the
/// internal `expect` is a programmer assertion rather than a recoverable error.
#[must_use]
pub fn framebuffer_lines(frame: &Framebuffer) -> Vec<String> {
    let (columns, rows) = frame.size();
    let mut lines = Vec::with_capacity(usize::from(rows));
    for row in 0..rows {
        let mut line = String::with_capacity(usize::from(columns));
        for column in 0..columns {
            let cell = frame.get(column, row).expect("frame coordinates are valid");
            if cell.continuation {
                continue;
            }
            line.push_str(if cell.symbol.is_empty() {
                " "
            } else {
                &cell.symbol
            });
        }
        while line.ends_with(' ') {
            line.pop();
        }
        lines.push(line);
    }
    while lines.last().is_some_and(String::is_empty) {
        lines.pop();
    }
    lines
}

pub fn draw_workspace(frame: &mut Framebuffer, state: &WorkspaceUiState) {
    clear_frame(frame);
    let (columns, rows) = frame.size();
    if columns == 0 || rows == 0 {
        return;
    }

    let header = format!(
        "Workspace | roots={} | recent={} | trust={}",
        state.explorer.roots.len(),
        state.recent_workspaces.len(),
        trust_label(state.trust)
    );
    write_line(frame, 0, &header, StyleRole::StatusBar);

    let body_top = 2;
    let mut body_bottom = rows;
    if state.prompt.is_some() {
        body_bottom = body_bottom.saturating_sub(2);
    }

    if state.explorer.roots.is_empty() {
        write_line(
            frame,
            body_top,
            "Explorer: empty workspace",
            StyleRole::Panel,
        );
    } else {
        draw_explorer(
            frame,
            &state.explorer,
            &state.recent_workspaces,
            state.trust,
            body_top,
            body_bottom,
        );
    }

    if let Some(prompt) = &state.prompt {
        draw_prompt(frame, prompt, rows.saturating_sub(2));
    }

    if matches!(state.trust, WorkspaceTrustState::Untrusted) {
        write_line(
            frame,
            1,
            "Trust: untrusted workspace - external processes blocked",
            StyleRole::Warning,
        );
    } else {
        write_line(
            frame,
            1,
            "Trust: trusted workspace - external processes permitted",
            StyleRole::Information,
        );
    }
}

pub fn draw_quick_open(frame: &mut Framebuffer, state: &QuickOpenState) {
    clear_frame(frame);
    let (columns, rows) = frame.size();
    if columns == 0 || rows == 0 {
        return;
    }
    write_line(
        frame,
        0,
        &format!("Quick Open | query={}", state.query),
        StyleRole::StatusBar,
    );
    write_line(
        frame,
        1,
        "Enter=current editor  Ctrl+Enter=to side  Mouse=select",
        StyleRole::Panel,
    );
    let mut row = 2;
    for (index, candidate) in state.results.iter().enumerate() {
        if row >= rows {
            break;
        }
        draw_quick_open_item(frame, row, index, candidate, index == state.selected);
        row += 1;
    }
    if state.results.is_empty() && row < rows {
        write_line(frame, row, "No quick-open matches", StyleRole::Warning);
    }
}

pub fn draw_search(frame: &mut Framebuffer, state: &SearchState) {
    clear_frame(frame);
    let (columns, rows) = frame.size();
    if columns == 0 || rows == 0 {
        return;
    }
    write_line(
        frame,
        0,
        &format!(
            "Search | session={} | query={}",
            state.session_id, state.query
        ),
        StyleRole::StatusBar,
    );
    let mut option_bits: Vec<String> = Vec::new();
    if state.options.literal {
        option_bits.push("literal".to_owned());
    }
    if state.options.case_sensitive {
        option_bits.push("case".to_owned());
    } else {
        option_bits.push("ignore-case".to_owned());
    }
    if state.options.whole_word {
        option_bits.push("whole-word".to_owned());
    }
    if let Some(max_results) = state.options.max_results {
        option_bits.push(format!("max={max_results}"));
    }
    write_line(
        frame,
        1,
        &format!("Options: {}", option_bits.join(" | ")),
        StyleRole::Panel,
    );

    let status = match state.status {
        SearchStatus::Idle => "idle",
        SearchStatus::Running => "streaming results",
        SearchStatus::Cancelled => "cancelled",
        SearchStatus::Superseded => "superseded by newer search",
        SearchStatus::Complete => "complete",
    };
    write_line(
        frame,
        2,
        &format!("Status: {status}"),
        StyleRole::Information,
    );

    if let Some(preview) = &state.replace_preview {
        write_line(
            frame,
            3,
            &format!(
                "Replace preview: {} file(s), {} match(es){}",
                preview.file_count,
                preview.match_count,
                if preview.confirmed {
                    " [confirmed]"
                } else {
                    ""
                }
            ),
            StyleRole::Warning,
        );
    }

    let mut row = if state.replace_preview.is_some() {
        4
    } else {
        3
    };
    for (index, result) in state.results.iter().enumerate() {
        if row >= rows {
            break;
        }
        let marker = if index == state.selected { ">" } else { " " };
        let line = format!(
            "{marker} {}:{} {}",
            result.path.display(),
            result.line_number,
            result.line_text
        );
        write_line(frame, row, &line, StyleRole::EditorText);
        row += 1;
    }
    if state.results.is_empty() && row < rows && state.replace_preview.is_none() {
        write_line(frame, row, "No search hits yet", StyleRole::Panel);
    }
}

fn draw_explorer(
    frame: &mut Framebuffer,
    state: &ExplorerState,
    recent_workspaces: &[std::path::PathBuf],
    trust: WorkspaceTrustState,
    top: u16,
    bottom: u16,
) {
    let mut row = top;
    write_line(frame, row, "Explorer", StyleRole::Panel);
    row = row.saturating_add(1);

    let entries = state.visible_entries();
    let mut visible_count = 0_u16;
    for entry in &entries {
        if row >= bottom {
            break;
        }
        draw_explorer_line(frame, row, entry);
        row = row.saturating_add(1);
        visible_count = visible_count.saturating_add(1);
    }

    if row < bottom {
        let hidden = entries.len().saturating_sub(usize::from(visible_count));
        if hidden > 0 {
            write_line(
                frame,
                row,
                &format!("... {hidden} more item(s) hidden"),
                StyleRole::Panel,
            );
            row = row.saturating_add(1);
        }
    }

    if row < bottom {
        write_line(frame, row, "Recent workspaces", StyleRole::Panel);
        row = row.saturating_add(1);
    }

    for recent in recent_workspaces {
        if row >= bottom {
            break;
        }
        write_line(
            frame,
            row,
            &format!("  {}", recent.display()),
            StyleRole::EditorText,
        );
        row = row.saturating_add(1);
    }

    if matches!(trust, WorkspaceTrustState::Trusted) && row < bottom {
        write_line(
            frame,
            row,
            "Workspace trust: trusted",
            StyleRole::Information,
        );
    }
}

fn draw_explorer_line(frame: &mut Framebuffer, row: u16, entry: &ExplorerLine) {
    let mut prefix = String::new();
    for _ in 0..entry.depth {
        prefix.push_str("  ");
    }
    let marker = if entry.selected { ">" } else { " " };
    let branch = if entry.expanded && entry.kind != WorkspaceEntryKind::File {
        "[-]"
    } else if entry.kind != WorkspaceEntryKind::File {
        "[+]"
    } else {
        "   "
    };
    let dirty = if entry.dirty { "*" } else { " " };
    let line = format!("{marker}{prefix}{branch}{dirty} {}", entry.label);
    let role = match entry.kind {
        WorkspaceEntryKind::Root | WorkspaceEntryKind::Directory => StyleRole::Panel,
        WorkspaceEntryKind::File => {
            if entry.dirty {
                StyleRole::Warning
            } else {
                StyleRole::EditorText
            }
        }
        WorkspaceEntryKind::Symlink => StyleRole::Information,
    };
    write_line(frame, row, &line, role);
}

fn draw_quick_open_item(
    frame: &mut Framebuffer,
    row: u16,
    index: usize,
    candidate: &QuickOpenCandidate,
    selected: bool,
) {
    let marker = if selected { ">" } else { " " };
    let disposition = match candidate.recent_rank {
        0 => "recent",
        1 => "active",
        _ => "other",
    };
    let line = format!(
        "{marker} {:>2}. {} ({disposition})",
        index + 1,
        candidate.path.display()
    );
    let role = if selected {
        StyleRole::Selection
    } else {
        StyleRole::EditorText
    };
    write_line(frame, row, &line, role);
}

fn draw_prompt(frame: &mut Framebuffer, prompt: &WorkspacePrompt, row: u16) {
    let title = match prompt {
        WorkspacePrompt::CreateFile { path } => {
            format!("Create file? {}", path.display())
        }
        WorkspacePrompt::Rename { source, target } => {
            format!("Rename? {} -> {}", source.display(), target.display())
        }
        WorkspacePrompt::Move { source, target } => {
            format!("Move? {} -> {}", source.display(), target.display())
        }
        WorkspacePrompt::Delete {
            path,
            recursive,
            byte_len,
        } => format!(
            "Delete? {}{} ({} byte(s))",
            path.display(),
            if *recursive { " [recursive]" } else { "" },
            byte_len
        ),
    };
    write_line(frame, row, &title, StyleRole::Error);
    write_line(
        frame,
        row.saturating_add(1),
        "[Enter] confirm  [Esc] cancel",
        StyleRole::Panel,
    );
}

fn clear_frame(frame: &mut Framebuffer) {
    let (columns, rows) = frame.size();
    for row in 0..rows {
        for column in 0..columns {
            let _ = frame.set(column, row, Cell::default());
        }
    }
}

fn write_line(frame: &mut Framebuffer, row: u16, text: &str, foreground: StyleRole) {
    let (columns, rows) = frame.size();
    if row >= rows {
        return;
    }
    let mut chars = text.chars();
    for column in 0..columns {
        let ch = chars.next().unwrap_or(' ');
        let _ = frame.set(
            column,
            row,
            Cell {
                symbol: ch.to_string(),
                foreground,
                background: StyleRole::EditorBackground,
                bold: false,
                continuation: false,
            },
        );
    }
}

fn trust_label(trust: WorkspaceTrustState) -> &'static str {
    match trust {
        WorkspaceTrustState::Trusted => "trusted",
        WorkspaceTrustState::Untrusted => "untrusted",
    }
}

#[must_use]
pub fn selected_result_range(state: &SearchState) -> Option<TextRange> {
    let result = state.selected_result()?;
    let start = result
        .line_text
        .get(..result.line_byte_range.start.min(result.line_text.len()))?
        .chars()
        .count();
    let length = result
        .line_text
        .get(
            result.line_byte_range.start.min(result.line_text.len())
                ..result.line_byte_range.end.min(result.line_text.len()),
        )?
        .chars()
        .count();
    Some(TextRange {
        start: editor_types::CharacterOffset(start),
        end: editor_types::CharacterOffset(start + length),
    })
}

#[must_use]
pub fn open_disposition_label(disposition: QuickOpenDisposition) -> &'static str {
    match disposition {
        QuickOpenDisposition::CurrentEditor => "current-editor",
        QuickOpenDisposition::OpenToSide => "open-to-side",
    }
}

#[must_use]
pub fn explorer_summary(state: &ExplorerState) -> String {
    let visible = state.visible_entries();
    format!("{} visible item(s)", visible.len())
}
