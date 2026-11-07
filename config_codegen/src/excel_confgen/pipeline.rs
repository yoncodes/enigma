use anyhow::{Context, Result, anyhow};
use serde_json::Value;
use std::{collections::HashSet, fs, path::Path};
use walkdir::WalkDir;

use crate::excel_confgen::{
    emit_file::emit_table_file, emit_root::emit_root_module, tables::FILTER_TABLES,
};

fn rust_table_name(source: &str) -> String {
    if source.starts_with(char::is_numeric) {
        format!("activity{source}")
    } else {
        source.to_owned()
    }
}

fn write_if_changed(path: &Path, content: &str) -> Result<()> {
    if let Ok(existing) = fs::read_to_string(path)
        && existing == content
    {
        return Ok(());
    }
    fs::write(path, content).with_context(|| format!("write {}", path.display()))?;
    Ok(())
}

fn remove_stale_modules(output: &Path, active: &HashSet<&str>) -> Result<()> {
    for entry in fs::read_dir(output).with_context(|| format!("read_dir {}", output.display()))? {
        let path = entry?.path();
        let Some(stem) = path.file_stem().and_then(|value| value.to_str()) else {
            continue;
        };
        if path.extension().and_then(|value| value.to_str()) != Some("rs")
            || stem == "mod"
            || active.contains(stem)
        {
            continue;
        }
        let content =
            fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
        let mut lines = content.lines();
        if lines.next() == Some("// Auto-generated from JSON data")
            && lines.next() == Some("// Do not edit manually")
        {
            fs::remove_file(&path).with_context(|| format!("remove {}", path.display()))?;
        }
    }
    Ok(())
}

pub fn generate_rust_modules(json_dir: &str, output_dir: &str) -> Result<()> {
    let output = Path::new(output_dir);
    fs::create_dir_all(output).with_context(|| format!("create_dir_all {}", output.display()))?;

    let filter: HashSet<&str> = FILTER_TABLES.iter().copied().collect();
    let mut tables = Vec::new();

    for entry in WalkDir::new(json_dir).into_iter().filter_map(Result::ok) {
        if !entry.file_type().is_file() {
            continue;
        }
        if entry.path().extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }

        let raw = entry
            .path()
            .file_stem()
            .and_then(|s| s.to_str())
            .ok_or_else(|| anyhow!("Invalid filename"))?;

        if !filter.contains(raw) {
            continue;
        }

        let rust_name = rust_table_name(raw);
        tables.push((raw.to_string(), rust_name.clone()));

        let json = fs::read_to_string(entry.path())
            .with_context(|| format!("read {}", entry.path().display()))?;
        let data: Value = serde_json::from_str(&json)?;

        let records = match data {
            Value::Array(ref a) if a.len() == 2 => a[1].as_array().cloned().unwrap_or_default(),
            Value::Array(a) => a,
            _ => vec![data],
        };

        let out_file = output.join(format!("{rust_name}.rs"));
        if records.is_empty() && out_file.exists() {
            continue;
        }
        let content = emit_table_file(&rust_name, &records);
        write_if_changed(&out_file, &content)?;
    }

    tables.sort_by(|left, right| left.1.cmp(&right.1));
    remove_stale_modules(
        output,
        &tables
            .iter()
            .map(|(_, rust_name)| rust_name.as_str())
            .collect(),
    )?;
    let mod_file = output.join("mod.rs");
    let mod_content = emit_root_module(&tables);
    write_if_changed(&mod_file, &mod_content)?;
    Ok(())
}

#[cfg(test)]
#[path = "pipeline/tests.rs"]
mod tests;
