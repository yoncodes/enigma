use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
};

#[test]
fn battle_crates_keep_their_dependency_boundary() {
    let workspace = workspace_root();
    assert_dependencies(
        &workspace.join("battle/Cargo.toml"),
        &[
            "anyhow",
            "config",
            "protocol",
            "rand",
            "serde",
            "serde_json",
            "tracing",
        ],
    );
    assert_dependencies(
        &workspace.join("battle_check/Cargo.toml"),
        &[
            "anyhow",
            "battle",
            "battle_preview",
            "config",
            "protocol",
            "serde_json",
        ],
    );
    assert_dependencies(
        &workspace.join("battle_preview/Cargo.toml"),
        &[
            "anyhow",
            "base64",
            "battle",
            "config",
            "flate2",
            "prost",
            "protocol",
            "serde_json",
        ],
    );
}

#[test]
fn runtime_uses_the_catalog_instead_of_raw_config() {
    let root = workspace_root().join("battle/src/engine/runtime");
    let mut violations = Vec::new();
    for path in production_rust_files(&root) {
        let source = fs::read_to_string(&path).unwrap();
        for (line_index, line) in source.lines().enumerate() {
            if line.contains("config::") || line.contains(".game_data()") {
                violations.push(format!("{}:{}: {line}", path.display(), line_index + 1));
            }
        }
    }
    assert!(
        violations.is_empty(),
        "runtime must receive normalized battle catalogs:\n{}",
        violations.join("\n")
    );
}

#[test]
fn battle_does_not_include_sources_from_consumer_crates() {
    let root = workspace_root().join("battle/src");
    let mut violations = Vec::new();
    for path in rust_files(&root) {
        let source = fs::read_to_string(&path).unwrap();
        if source.contains("battle_preview/src") {
            violations.push(path.display().to_string());
        }
    }
    assert!(
        violations.is_empty(),
        "battle must not include source files from consumer crates:\n{}",
        violations.join("\n")
    );
}

#[test]
fn dependency_parser_includes_target_and_dotted_tables() {
    let manifest = r#"
[dependencies]
plain.workspace = true

[dependencies.dotted]
version = "1"

[target.'cfg(unix)'.dependencies] # unix only
platform.workspace = true

[target.'cfg(windows)'.dependencies.target_dotted]
version = "1"

[dev-dependencies]
test_only.workspace = true

[build-dependencies]
build_only.workspace = true

[package.metadata.dependencies]
metadata_only.workspace = true

[package.metadata.dependencies.nested]
version = "1"
"#;
    assert_eq!(
        dependency_names(manifest),
        ["dotted", "plain", "platform", "target_dotted"]
            .into_iter()
            .map(str::to_owned)
            .collect()
    );
}

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_owned()
}

fn assert_dependencies(manifest: &Path, expected: &[&str]) {
    let source = fs::read_to_string(manifest).unwrap();
    let actual = dependency_names(&source);
    let expected = expected
        .iter()
        .map(|name| (*name).to_owned())
        .collect::<BTreeSet<_>>();
    assert_eq!(
        actual,
        expected,
        "unexpected dependencies in {}",
        manifest.display()
    );
}

fn dependency_names(source: &str) -> BTreeSet<String> {
    let mut list_dependencies = false;
    let mut actual = BTreeSet::new();
    for raw in source.lines() {
        let line = raw.trim();
        if let Some(section) = table_header(line) {
            let target = section.starts_with("target.");
            list_dependencies =
                section == "dependencies" || (target && section.ends_with(".dependencies"));
            if target && let Some((_, dependency)) = section.rsplit_once(".dependencies.") {
                actual.insert(dependency.split('.').next().unwrap().to_owned());
            } else if let Some(dependency) = section.strip_prefix("dependencies.") {
                actual.insert(dependency.split('.').next().unwrap().to_owned());
            }
            continue;
        }
        if list_dependencies
            && !line.is_empty()
            && !line.starts_with('#')
            && let Some((name, _)) = line.split_once('=')
        {
            actual.insert(name.trim().split('.').next().unwrap().to_owned());
        }
    }
    actual
}

fn table_header(line: &str) -> Option<&str> {
    let line = line.strip_prefix('[')?;
    let (section, trailing) = line.split_once(']')?;
    let trailing = trailing.trim();
    (trailing.is_empty() || trailing.starts_with('#')).then_some(section)
}

fn production_rust_files(root: &Path) -> Vec<PathBuf> {
    rust_files_filtered(root, true)
}

fn rust_files(root: &Path) -> Vec<PathBuf> {
    rust_files_filtered(root, false)
}

fn rust_files_filtered(root: &Path, production_only: bool) -> Vec<PathBuf> {
    let mut pending = vec![root.to_owned()];
    let mut files = Vec::new();
    while let Some(path) = pending.pop() {
        for entry in fs::read_dir(path).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                if !production_only
                    || !matches!(
                        path.file_name().and_then(|name| name.to_str()),
                        Some("test" | "tests")
                    )
                {
                    pending.push(path);
                }
            } else if path.extension().and_then(|extension| extension.to_str()) == Some("rs")
                && (!production_only
                    || !matches!(
                        path.file_name().and_then(|name| name.to_str()),
                        Some("test.rs" | "tests.rs")
                    ))
            {
                files.push(path);
            }
        }
    }
    files.sort();
    files
}
