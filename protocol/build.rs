use std::path::Path;

fn main() {
    println!("cargo::rustc-check-cfg=cfg(rust_analyzer)");

    let proto_files = ["sonetto.proto", "cmd_id.proto"];

    if proto_files.iter().all(|f| Path::new(f).exists()) {
        for proto in &proto_files {
            println!("cargo::rerun-if-changed={proto}");
        }
        prost_build::Config::new()
            .type_attribute(
                ".",
                "#[derive(serde::Serialize, serde::Deserialize)]\n#[serde(rename_all = \"camelCase\")]",
            )
            .message_attribute(".", r#"#[serde(default)]"#)
            .field_attribute("*.type", "#[serde(rename = \"type\")]")
            .out_dir("include/")
            .compile_protos(&proto_files, &["."])
            .expect("Failed to compile proto files");
    } else {
        // Ignored proto sources are absent in normal worktrees; watching a missing file is always dirty.
        println!("cargo::rerun-if-changed=.");
    }
}
