use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

use tempfile::tempdir;
use workspace_core::{
    add_workspace_root, apply_replacement_plan, canonical_workspace_key, collect_search_results,
    create_file, delete_file, load_text_document, move_path, persist_recent_workspaces,
    persist_trust_store, plan_delete, plan_move, plan_rename, rename_path, save_text_document,
    search::parse_rg_json_lines, search_workspace, workspace_identity_key, DecodePolicy,
    DeletePlan, DocumentLoadOptions, DocumentSaveOptions, EncodingKind, ExplorerTree,
    FileChangeEvent, FileChangeTracker, LargeFileSettings, LineEndings, QuickOpenIndex,
    SearchBackendPreference, SearchHit, SearchOptions, TrustState, WorkspaceSet,
};

#[test]
fn workspace_roots_deduplicate_and_persistence_round_trip() {
    let dir = tempdir().expect("dir");
    let root = dir.path().to_path_buf();
    let alias = root.join(".");

    let mut roots = WorkspaceSet::default();
    let first = add_workspace_root(&mut roots, &root).expect("first");
    let second = add_workspace_root(&mut roots, &alias).expect("second");
    assert_eq!(first, second);
    assert_eq!(roots.len(), 1);

    let trust_path = dir.path().join("trust.json");
    let mut trust = workspace_core::TrustStore::default();
    trust.set_state(&root, TrustState::Trusted);
    persist_trust_store(&trust, &trust_path).expect("save");
    let loaded = workspace_core::load_trust_store(&trust_path).expect("load");
    assert_eq!(loaded.state_for_path(&alias), TrustState::Trusted);

    let recent_path = dir.path().join("recent.json");
    let mut recent = workspace_core::RecentWorkspaceStore::default();
    recent.touch(&root);
    persist_recent_workspaces(&recent, &recent_path).expect("save recent");
    let loaded_recent = workspace_core::load_recent_workspaces(&recent_path).expect("load recent");
    assert_eq!(loaded_recent.entries().len(), 1);
    assert_eq!(canonical_workspace_key(&root), workspace_identity_key(&alias));
}

#[test]
fn explorer_and_quick_open_respect_gitignore_and_excludes() {
    let dir = tempdir().expect("dir");
    fs::write(dir.path().join(".gitignore"), "ignored.txt\nignored-dir/\n").expect("gitignore");
    fs::create_dir_all(dir.path().join("ignored-dir")).expect("dir");
    fs::write(dir.path().join("included.txt"), "keep").expect("file");
    fs::write(dir.path().join("ignored.txt"), "skip").expect("file");
    fs::write(dir.path().join("ignored-dir").join("child.txt"), "skip").expect("file");
    fs::write(dir.path().join("visible.log"), "skip").expect("file");

    let tree = ExplorerTree::new(
        vec![workspace_core::CanonicalPath::new(dir.path().to_path_buf())],
        vec!["*.log".to_owned()],
    );
    let children = tree.children(dir.path()).expect("children");
    assert!(children.iter().any(|entry| entry.path.ends_with("included.txt")));
    assert!(!children.iter().any(|entry| entry.path.ends_with("ignored.txt")));
    assert!(!children.iter().any(|entry| entry.path.ends_with("visible.log")));

    let index = QuickOpenIndex::rebuild(&tree).expect("index");
    let entries = index.entries();
    assert!(entries.iter().any(|entry| entry.ends_with("included.txt")));
    assert!(!entries.iter().any(|entry| entry.ends_with("ignored.txt")));
    assert!(!entries.iter().any(|entry| entry.ends_with("visible.log")));
}

#[test]
fn symlink_loop_does_not_recurse_forever() {
    let dir = tempdir().expect("dir");
    let root = dir.path();
    fs::create_dir_all(root.join("a")).expect("dir");
    fs::write(root.join("a").join("file.txt"), "ok").expect("file");

    let loop_target = root.join("a").join("loop");
    let symlink_created = create_directory_symlink(&loop_target, root.join("a").as_path());
    if !symlink_created {
        return;
    }

    let tree = ExplorerTree::new(vec![workspace_core::CanonicalPath::new(root.to_path_buf())], Vec::new());
    let children = tree.children(root.join("a").as_path()).expect("children");
    assert!(children.iter().any(|entry| entry.path.ends_with("file.txt")));
    assert!(children.len() < 10);
}

#[test]
fn open_decode_and_atomic_save_round_trip() {
    let dir = tempdir().expect("dir");
    let source = dir.path().join("source.txt");
    let target = dir.path().join("saved.txt");
    let bytes = workspace_core::encode_text_document(
        "Hello\r\nWorld",
        &EncodingKind::Utf16Le,
        true,
        DecodePolicy::Strict,
    )
    .expect("encode");
    fs::write(&source, bytes).expect("write");

    let loaded = load_text_document(
        &source,
        &DocumentLoadOptions {
            fallback_encoding: EncodingKind::Utf8,
            malformed_input_policy: DecodePolicy::Strict,
            large_file_settings: LargeFileSettings::default(),
        },
    )
    .expect("load");
    assert_eq!(loaded.encoding, EncodingKind::Utf16Le);
    assert_eq!(loaded.line_endings, LineEndings::Crlf);
    assert_eq!(loaded.text, "Hello\r\nWorld");

    let saved = save_text_document(
        &target,
        &loaded.text,
        &DocumentSaveOptions {
            encoding: loaded.encoding.clone(),
            with_bom: true,
            line_endings: loaded.line_endings,
            malformed_input_policy: DecodePolicy::Strict,
        },
    )
    .expect("save");
    assert_eq!(saved.path, target);

    let round_trip = load_text_document(
        &target,
        &DocumentLoadOptions {
            fallback_encoding: EncodingKind::Utf8,
            malformed_input_policy: DecodePolicy::Strict,
            large_file_settings: LargeFileSettings::default(),
        },
    )
    .expect("reload");
    assert_eq!(round_trip.text, loaded.text);
}

#[test]
fn external_changes_reload_clean_buffers_and_conflict_dirty_buffers() {
    let dir = tempdir().expect("dir");
    let path = dir.path().join("watch.txt");
    fs::write(&path, "one").expect("seed");

    let clean = load_text_document(
        &path,
        &DocumentLoadOptions {
            fallback_encoding: EncodingKind::Utf8,
            malformed_input_policy: DecodePolicy::Strict,
            large_file_settings: LargeFileSettings::default(),
        },
    )
    .expect("load");
    let mut tracker = FileChangeTracker::new();
    tracker.watch(&clean, false).expect("watch");
    fs::write(&path, "two").expect("mutate");
    let events = tracker.poll().expect("poll");
    assert!(events.iter().any(|event| matches!(event, FileChangeEvent::Reload { .. })));

    fs::write(&path, "three").expect("mutate");
    let dirty = load_text_document(
        &path,
        &DocumentLoadOptions {
            fallback_encoding: EncodingKind::Utf8,
            malformed_input_policy: DecodePolicy::Strict,
            large_file_settings: LargeFileSettings::default(),
        },
    )
    .expect("load");
    let mut conflict_tracker = FileChangeTracker::new();
    conflict_tracker.watch(&dirty, true).expect("watch");
    fs::write(&path, "four").expect("mutate");
    let events = conflict_tracker.poll().expect("poll");
    assert!(events.iter().any(|event| matches!(event, FileChangeEvent::Conflict { .. })));
}

#[test]
fn cancellation_stops_streaming_search() {
    let dir = tempdir().expect("dir");
    for index in 0..100 {
        fs::write(dir.path().join(format!("file-{index}.txt")), "needle").expect("seed");
    }
    let session = search_workspace(SearchOptions {
        roots: vec![dir.path().to_path_buf()],
        pattern: "needle".to_owned(),
        literal: true,
        case_sensitive: true,
        whole_word: false,
        includes: Vec::new(),
        excludes: Vec::new(),
        backend: SearchBackendPreference::ForceRust,
        max_results: None,
    })
    .expect("search");
    session.cancel();
    let hits = collect_search_results(&session).expect("hits");
    assert!(hits.len() < 100);
}

#[test]
fn rg_fixture_and_backend_equivalence_cover_required_options() {
    let fixture = include_str!("../../../tests/fixtures/workspace-core/rg-output.jsonl");
    let lines: Vec<&str> = fixture.lines().collect();
    let parsed = parse_rg_json_lines(&lines).expect("parse");
    assert_eq!(parsed.len(), 1);
    assert_eq!(parsed[0].matched_text, "apple");

    if !rg_available_for_test() {
        return;
    }

    let dir = tempdir().expect("dir");
    fs::create_dir_all(dir.path().join("src")).expect("dir");
    fs::write(dir.path().join("src").join("alpha.rs"), "apple Banana\nneedle here\n").expect("file");
    fs::write(dir.path().join("src").join("beta.rs"), "apple\nbanana\n").expect("file");
    fs::write(dir.path().join("skip.log"), "apple\n").expect("file");

    let options = SearchOptions {
        roots: vec![dir.path().to_path_buf()],
        pattern: "apple".to_owned(),
        literal: true,
        case_sensitive: false,
        whole_word: true,
        includes: vec!["*.rs".to_owned()],
        excludes: vec!["*.log".to_owned()],
        backend: SearchBackendPreference::ForceRust,
        max_results: None,
    };
    let rust_hits = collect_search_results(&search_workspace(options.clone()).expect("rust"))
        .expect("rust results");
    let rg_options = SearchOptions {
        backend: SearchBackendPreference::ForceRg,
        ..options
    };
    let rg_hits = collect_search_results(&search_workspace(rg_options).expect("rg"))
        .expect("rg results");

    let rust_paths: Vec<_> = rust_hits.iter().map(hit_key).collect();
    let rg_paths: Vec<_> = rg_hits.iter().map(hit_key).collect();
    let mut rust_paths_sorted = rust_paths;
    let mut rg_paths_sorted = rg_paths;
    rust_paths_sorted.sort();
    rg_paths_sorted.sort();
    assert_eq!(rust_paths_sorted, rg_paths_sorted);
}

#[test]
fn replacement_plans_apply_and_report_failures() {
    let dir = tempdir().expect("dir");
    let a = dir.path().join("a.txt");
    let b = dir.path().join("b.txt");
    let missing = dir.path().join("missing.txt");
    fs::write(&a, "foo foo").expect("seed");
    fs::write(&b, "bar").expect("seed");

    let hits = vec![
        SearchHit {
            path: a.clone(),
            line_number: 1,
            line_text: "foo foo".to_owned(),
            byte_range: 0..3,
            matched_text: "foo".to_owned(),
        },
        SearchHit {
            path: a.clone(),
            line_number: 1,
            line_text: "foo foo".to_owned(),
            byte_range: 4..7,
            matched_text: "foo".to_owned(),
        },
        SearchHit {
            path: b.clone(),
            line_number: 1,
            line_text: "bar".to_owned(),
            byte_range: 0..3,
            matched_text: "bar".to_owned(),
        },
        SearchHit {
            path: missing,
            line_number: 1,
            line_text: "bar".to_owned(),
            byte_range: 0..3,
            matched_text: "bar".to_owned(),
        },
    ];
    let plan = workspace_core::plan_replacements(&hits, "zip");
    let report = apply_replacement_plan(&plan).expect("apply");
    assert!(report.modified_files.contains(&a));
    assert!(report.modified_files.contains(&b));
    assert!(report.failed_files.iter().any(|path| path.ends_with("missing.txt")));
    assert_eq!(fs::read_to_string(&a).expect("read"), "zip zip");
}

#[test]
fn rename_move_delete_plans_and_operations_work() {
    let dir = tempdir().expect("dir");
    let source = dir.path().join("source.txt");
    let renamed = dir.path().join("renamed.txt");
    let nested = dir.path().join("nested");
    let moved = nested.join("moved.txt");
    fs::create_dir_all(&nested).expect("dir");
    fs::write(&source, "content").expect("seed");

    let rename_plan = plan_rename(&source, &renamed).expect("plan");
    assert_eq!(rename_plan.source, source);
    rename_path(&source, &renamed).expect("rename");
    assert!(renamed.exists());

    let move_plan = plan_move(&renamed, &moved).expect("plan");
    assert_eq!(move_plan.source, renamed);
    move_path(&renamed, &moved).expect("move");
    assert!(moved.exists());

    let delete_plan = plan_delete(&moved).expect("plan");
    assert!(matches!(delete_plan, DeletePlan { is_directory: false, .. }));
    delete_file(&moved).expect("delete");
    assert!(!moved.exists());
}

#[test]
fn create_file_writes_new_content_atomically() {
    let dir = tempdir().expect("dir");
    let target = dir.path().join("created.txt");
    create_file(&target, b"hello").expect("create");
    assert_eq!(fs::read_to_string(&target).expect("read"), "hello");
}

fn create_directory_symlink(link: &Path, target: &Path) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::symlink;
        symlink(target, link).is_ok()
    }
    #[cfg(windows)]
    {
        std::os::windows::fs::symlink_dir(target, link).is_ok()
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = link;
        let _ = target;
        false
    }
}

fn rg_available_for_test() -> bool {
    Command::new("rg")
        .arg("--version")
        .output()
        .is_ok_and(|output| output.status.success())
}

fn hit_key(hit: &SearchHit) -> (PathBuf, usize, String) {
    (hit.path.clone(), hit.line_number, hit.matched_text.clone())
}
