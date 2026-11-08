use super::*;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

fn temp_root() -> std::path::PathBuf {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    std::env::temp_dir().join(format!(
        "enigma_config_codegen_{}_{}_{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ))
}

#[test]
fn removes_only_stale_generated_modules() {
    let root = temp_root();
    let input = root.join("input");
    let output = root.join("output");
    fs::create_dir_all(&input).unwrap();
    fs::create_dir_all(&output).unwrap();
    fs::write(input.join("character.json"), "[\"character\",[]]").unwrap();
    fs::write(
        output.join("stale.rs"),
        "// Auto-generated from JSON data\r\n// Do not edit manually\r\n",
    )
    .unwrap();
    fs::write(output.join("manual.rs"), "// maintained by hand\n").unwrap();

    generate_rust_modules(input.to_str().unwrap(), output.to_str().unwrap()).unwrap();

    assert!(!output.join("stale.rs").exists());
    assert!(output.join("manual.rs").exists());
    assert!(output.join("character.rs").exists());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn numeric_table_names_use_valid_rust_identifiers() {
    let root = temp_root();
    let input = root.join("input");
    let output = root.join("output");
    fs::create_dir_all(&input).unwrap();
    fs::write(input.join("243_const.json"), r#"["243_const",[{"id":1}]]"#).unwrap();

    generate_rust_modules(input.to_str().unwrap(), output.to_str().unwrap()).unwrap();

    assert!(output.join("activity243_const.rs").exists());
    let root_module = fs::read_to_string(output.join("mod.rs")).unwrap();
    assert!(root_module.contains("pub mod activity243_const;"));
    assert!(root_module.contains("{}/243_const.json"));
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn empty_tables_preserve_the_existing_schema() {
    let root = temp_root();
    let input = root.join("input");
    let output = root.join("output");
    fs::create_dir_all(&input).unwrap();
    fs::create_dir_all(&output).unwrap();
    fs::write(input.join("teaching_card.json"), r#"["teaching_card",[]]"#).unwrap();
    fs::write(output.join("teaching_card.rs"), "existing schema").unwrap();

    generate_rust_modules(input.to_str().unwrap(), output.to_str().unwrap()).unwrap();

    assert_eq!(
        fs::read_to_string(output.join("teaching_card.rs")).unwrap(),
        "existing schema"
    );
    fs::remove_dir_all(root).unwrap();
}
