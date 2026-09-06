//! Workspace view boundary and state composition.

mod model;
mod render;

pub use model::{
    ExplorerLine, ExplorerState, QuickOpenAction, QuickOpenCandidate, QuickOpenDisposition,
    QuickOpenState, RecentWorkspaceRow, ReplacePreview, SearchOptionsView, SearchResult,
    SearchState, SearchStatus, WorkspaceEntry, WorkspaceEntryKind, WorkspaceModelEvent,
    WorkspacePrompt, WorkspaceTrustState, WorkspaceUiState, infer_label, is_mouse_left_click,
    mouse_pick_from_row, quick_open_candidate,
};
pub use render::{
    draw_quick_open, draw_search, draw_workspace, explorer_summary, framebuffer_lines,
    open_disposition_label, selected_result_range,
};

use terminal_backend::Framebuffer;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkspaceView {
    Explorer,
    QuickOpen,
    Search,
    Trust,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceUi {
    pub view: WorkspaceView,
    pub state: WorkspaceUiState,
}

impl Default for WorkspaceUi {
    fn default() -> Self {
        Self {
            view: WorkspaceView::Explorer,
            state: WorkspaceUiState::default(),
        }
    }
}

impl WorkspaceUi {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn apply_event(&mut self, event: WorkspaceModelEvent) {
        self.state.apply_event(event);
    }

    pub fn set_view(&mut self, view: WorkspaceView) {
        self.view = view;
    }

    pub fn render(&self, frame: &mut Framebuffer) {
        match self.view {
            WorkspaceView::Explorer | WorkspaceView::Trust => draw_workspace(frame, &self.state),
            WorkspaceView::QuickOpen => draw_quick_open(frame, &self.state.quick_open),
            WorkspaceView::Search => draw_search(frame, &self.state.search),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::{Path, PathBuf},
    };

    use terminal_backend::Framebuffer;

    use super::*;

    fn fixture(path: &str) -> String {
        fs::read_to_string(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("..")
                .join("..")
                .join(path),
        )
        .expect("fixture")
    }

    fn snapshot(path: &str) -> String {
        fixture(path).trim_end_matches('\n').to_owned()
    }

    fn frame_lines(ui: &WorkspaceUi, columns: u16, rows: u16) -> String {
        let mut frame = Framebuffer::new(columns, rows);
        ui.render(&mut frame);
        framebuffer_lines(&frame).join("\n")
    }

    fn explorer_ui() -> WorkspaceUi {
        let mut ui = WorkspaceUi::new();
        ui.apply_event(WorkspaceModelEvent::RootsChanged {
            roots: vec![
                WorkspaceEntry::root("/workspace/alpha", "alpha").with_children(vec![
                    WorkspaceEntry::directory("/workspace/alpha/src", "src")
                        .with_expanded(true)
                        .with_children(vec![
                            WorkspaceEntry::file("/workspace/alpha/src/main.rs", "main.rs")
                                .with_dirty(true),
                            WorkspaceEntry::file("/workspace/alpha/src/lib.rs", "lib.rs"),
                        ]),
                    WorkspaceEntry::file("/workspace/alpha/README.md", "README.md"),
                ]),
                WorkspaceEntry::root("/workspace/beta", "beta").with_children(vec![
                    WorkspaceEntry::directory("/workspace/beta/docs", "docs")
                        .with_expanded(true)
                        .with_children(vec![WorkspaceEntry::file(
                            "/workspace/beta/docs/guide.md",
                            "guide.md",
                        )]),
                    WorkspaceEntry::directory("/workspace/beta/logs", "logs").with_children(vec![
                        WorkspaceEntry::file("/workspace/beta/logs/app.log", "app.log"),
                    ]),
                ]),
            ],
            recent: vec![
                PathBuf::from("/workspace/beta"),
                PathBuf::from("/workspace/alpha"),
            ],
            trust: WorkspaceTrustState::Untrusted,
        });
        ui.state.explorer.select("/workspace/alpha/src/main.rs");
        ui
    }

    #[test]
    fn explorer_tree_expansion_and_multi_root_rendering() {
        let ui = explorer_ui();
        let rendered = frame_lines(&ui, 64, 16);
        assert_eq!(
            rendered,
            snapshot("tests/snapshots/ui-workspace/explorer_multi_root.txt")
        );
    }

    #[test]
    fn compact_collapse_keeps_the_view_deterministic() {
        let ui = explorer_ui();
        let rendered = frame_lines(&ui, 40, 8);
        assert_eq!(
            rendered,
            snapshot("tests/snapshots/ui-workspace/explorer_compact_collapse.txt")
        );
    }

    #[test]
    fn file_operation_confirmations_render_prompt_details() {
        let mut ui = explorer_ui();
        ui.state.prompt = Some(WorkspacePrompt::Delete {
            path: PathBuf::from("/workspace/beta/logs"),
            recursive: true,
            byte_len: 4096,
        });
        let rendered = frame_lines(&ui, 64, 10);
        assert_eq!(
            rendered,
            snapshot("tests/snapshots/ui-workspace/delete_prompt.txt")
        );
    }

    #[test]
    fn quick_open_filtering_and_keyboard_mouse_selection_work() {
        let mut ui = WorkspaceUi::new();
        ui.set_view(WorkspaceView::QuickOpen);
        ui.state.quick_open.set_candidates(vec![
            quick_open_candidate("/workspace/alpha/src/main.rs", 0),
            quick_open_candidate("/workspace/alpha/src/lib.rs", 1),
            quick_open_candidate("/workspace/beta/README.md", 2),
        ]);
        ui.state.quick_open.set_query("main");
        let selected = ui.state.quick_open.selected_candidate().expect("candidate");
        assert_eq!(selected.path, PathBuf::from("/workspace/alpha/src/main.rs"));
        let picked = ui
            .state
            .quick_open
            .apply_action(QuickOpenAction::OpenToSide)
            .expect("command");
        assert_eq!(picked.0, QuickOpenDisposition::OpenToSide);
        assert_eq!(picked.1, PathBuf::from("/workspace/alpha/src/main.rs"));
        let rendered = frame_lines(&ui, 72, 10);
        assert_eq!(
            rendered,
            snapshot("tests/snapshots/ui-workspace/quick_open.txt")
        );
    }

    #[test]
    fn streamed_search_results_and_superseding_sessions_are_visible() {
        let mut ui = WorkspaceUi::new();
        ui.set_view(WorkspaceView::Search);
        ui.state.search.start_session(
            7,
            "needle",
            SearchOptionsView {
                literal: true,
                case_sensitive: false,
                whole_word: false,
                max_results: Some(50),
                ..SearchOptionsView::default()
            },
        );
        ui.state.search.push_result(
            7,
            SearchResult {
                path: PathBuf::from("/workspace/alpha/src/main.rs"),
                line_number: 12,
                line_text: "needle alpha".to_owned(),
                matched_text: "needle".to_owned(),
            },
        );
        ui.state.search.push_result(
            7,
            SearchResult {
                path: PathBuf::from("/workspace/alpha/src/lib.rs"),
                line_number: 22,
                line_text: "needle beta".to_owned(),
                matched_text: "needle".to_owned(),
            },
        );
        let rendered = frame_lines(&ui, 84, 12);
        assert_eq!(
            rendered,
            snapshot("tests/snapshots/ui-workspace/search_stream.txt")
        );

        ui.state.search.supersede(
            8,
            "needle",
            SearchOptionsView {
                literal: false,
                case_sensitive: true,
                whole_word: true,
                max_results: Some(10),
                ..SearchOptionsView::default()
            },
        );
        ui.state.search.push_result(
            7,
            SearchResult {
                path: PathBuf::from("/workspace/should/not/appear.txt"),
                line_number: 1,
                line_text: "stale".to_owned(),
                matched_text: "stale".to_owned(),
            },
        );
        let rendered = frame_lines(&ui, 84, 10);
        assert_eq!(
            rendered,
            snapshot("tests/snapshots/ui-workspace/search_superseded.txt")
        );
    }

    #[test]
    fn replace_preview_reports_counts_before_writes() {
        let mut ui = WorkspaceUi::new();
        ui.set_view(WorkspaceView::Search);
        ui.state.search.start_session(
            9,
            "foo",
            SearchOptionsView {
                literal: true,
                case_sensitive: true,
                whole_word: false,
                max_results: None,
                ..SearchOptionsView::default()
            },
        );
        ui.state.search.set_replace_preview(3, 9);
        let rendered = frame_lines(&ui, 76, 10);
        assert_eq!(
            rendered,
            snapshot("tests/snapshots/ui-workspace/replace_preview.txt")
        );
        ui.state.search.confirm_replace_preview();
        assert!(
            ui.state
                .search
                .replace_preview
                .as_ref()
                .expect("preview")
                .confirmed
        );
    }

    #[test]
    fn untrusted_and_trusted_workspace_states_render_persistently() {
        let mut ui = explorer_ui();
        let untrusted = frame_lines(&ui, 64, 8);
        assert_eq!(
            untrusted,
            snapshot("tests/snapshots/ui-workspace/trust_untrusted.txt")
        );
        ui.state.trust = WorkspaceTrustState::Trusted;
        let trusted = frame_lines(&ui, 64, 8);
        assert_eq!(
            trusted,
            snapshot("tests/snapshots/ui-workspace/trust_trusted.txt")
        );
    }

    #[test]
    fn fixture_parsing_supports_fake_workspace_models() {
        let data = fixture("tests/fixtures/app-ui/workspace/explorer_fixture.txt");
        assert!(data.contains("workspace/alpha"));
        assert!(data.contains("workspace/beta"));
    }
}
