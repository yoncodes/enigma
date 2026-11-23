use super::*;

#[cfg(windows)]
fn link_directory(source: &Path, destination: &Path) -> std::io::Result<()> {
    std::os::windows::fs::symlink_dir(source, destination)
}

#[cfg(unix)]
fn link_directory(source: &Path, destination: &Path) -> std::io::Result<()> {
    std::os::unix::fs::symlink(source, destination)
}

fn temporary(name: &str) -> PathBuf {
    env::temp_dir().join(format!(
        "enigma-battle-report-{name}-{}",
        std::process::id()
    ))
}

#[test]
fn unowned_collision_is_preserved_and_blocks_generation() {
    let root = temporary("collision");
    fs::create_dir_all(root.join(data::ROOT)).unwrap();
    fs::write(root.join(data::ROOT).join("manifest.json"), "mine").unwrap();
    let staging = prepare_staging(&root).unwrap();

    assert!(data::write_manifest(&staging, BTreeMap::new(), &mut Vec::new()).is_err());
    assert_eq!(
        fs::read_to_string(root.join(data::ROOT).join("manifest.json")).unwrap(),
        "mine"
    );

    cleanup_owned_directory(&staging).unwrap();
    cleanup_owned_directory(&root).unwrap();
}

#[test]
fn prior_generated_files_are_not_copied_but_unowned_files_are() {
    let root = temporary("ownership");
    fs::create_dir_all(&root).unwrap();
    fs::write(root.join("battle-support.md"), "old").unwrap();
    fs::write(root.join("keep.txt"), "keep").unwrap();
    fs::write(root.join(MANIFEST), r#"["battle-support.md"]"#).unwrap();

    let staging = prepare_staging(&root).unwrap();
    assert!(!staging.join("battle-support.md").exists());
    assert_eq!(
        fs::read_to_string(staging.join("keep.txt")).unwrap(),
        "keep"
    );

    cleanup_owned_directory(&staging).unwrap();
    cleanup_owned_directory(&root).unwrap();
}

#[test]
fn report_output_rejects_a_link_or_reparse_point() {
    let output = temporary("linked-output");
    let outside = temporary("linked-outside");
    fs::create_dir_all(&outside).unwrap();
    if link_directory(&outside, &output).is_ok() {
        assert!(absolute_output(&output).is_err());
        fs::remove_dir(&output).unwrap();
    }
    cleanup_owned_directory(&outside).unwrap();
}

#[test]
fn report_parent_rejects_a_link_before_creating_beyond_it() {
    let link = temporary("linked-parent");
    let outside = temporary("linked-parent-outside");
    fs::create_dir_all(&outside).unwrap();
    if link_directory(&outside, &link).is_ok() {
        let output = link.join("must-not-exist").join("report");
        assert!(absolute_output(&output).is_err());
        assert!(!outside.join("must-not-exist").exists());
        fs::remove_dir(&link).unwrap();
    }
    cleanup_owned_directory(&outside).unwrap();
}

#[test]
fn report_output_rejects_current_directory_aliases() {
    assert!(absolute_output(Path::new("")).is_err());
    assert!(absolute_output(Path::new(".")).is_err());
    assert!(absolute_output(Path::new("reports/..")).is_err());
    assert!(absolute_output(Path::new("../battle-report-prototype")).is_err());
    assert!(absolute_output(&env::current_dir().unwrap()).is_err());
}

#[test]
fn report_output_accepts_a_new_directory_below_an_existing_parent() {
    let parent = temporary("new-output-parent");
    fs::create_dir_all(&parent).unwrap();
    let output = parent.join("nested").join("report");

    assert_eq!(absolute_output(&output).unwrap(), output);
    assert!(output.parent().unwrap().is_dir());

    cleanup_owned_directory(&parent).unwrap();
}

#[cfg(windows)]
#[test]
fn report_output_rejects_drive_relative_and_rooted_paths() {
    assert!(absolute_output(Path::new(r"C:report")).is_err());
    assert!(absolute_output(Path::new(r"\report")).is_err());
}
