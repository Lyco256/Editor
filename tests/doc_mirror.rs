use std::{
    fs,
    path::{Path, PathBuf},
};

fn collect_files(root: &Path, extension: &str, output: &mut Vec<PathBuf>) {
    let entries = fs::read_dir(root)
        .unwrap_or_else(|error| panic!("could not read {}: {error}", root.display()));
    for entry in entries {
        let path = entry.expect("directory entry should be readable").path();
        if path.is_dir() {
            collect_files(&path, extension, output);
        } else if path.extension().and_then(|value| value.to_str()) == Some(extension) {
            output.push(path);
        }
    }
}

fn source_roots(repository: &Path) -> Vec<PathBuf> {
    let mut roots = vec![repository.join("src")];
    let crates = repository.join("crates");
    for entry in fs::read_dir(crates).expect("crates directory should exist") {
        let source = entry
            .expect("crate entry should be readable")
            .path()
            .join("src");
        if source.is_dir() {
            roots.push(source);
        }
    }
    roots
}

#[test]
fn every_production_rust_source_has_exact_documentation_mirror() {
    let repository = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let mut missing = Vec::new();

    for root in source_roots(&repository) {
        let mut sources = Vec::new();
        collect_files(&root, "rs", &mut sources);
        for source in sources {
            let relative = source
                .strip_prefix(&repository)
                .expect("source should be inside repository");
            let mirror = repository.join("docs").join(relative).with_extension("md");
            if !mirror.is_file() {
                missing.push(format!("{} -> {}", source.display(), mirror.display()));
            }
        }
    }

    assert!(
        missing.is_empty(),
        "production sources without mirrored docs:\n{}",
        missing.join("\n")
    );
}

#[test]
fn every_source_mirror_points_to_a_production_rust_file() {
    let repository = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let documentation = repository.join("docs");
    let mut mirrors = Vec::new();
    for root in [documentation.join("src"), documentation.join("crates")] {
        if root.is_dir() {
            collect_files(&root, "md", &mut mirrors);
        }
    }

    let mut orphaned = Vec::new();
    for mirror in mirrors {
        let relative = mirror
            .strip_prefix(&documentation)
            .expect("mirror should be below docs");
        let source = repository.join(relative).with_extension("rs");
        if !source.is_file() {
            orphaned.push(format!("{} -> {}", mirror.display(), source.display()));
        }
    }

    assert!(
        orphaned.is_empty(),
        "orphaned source documentation:\n{}",
        orphaned.join("\n")
    );
}
