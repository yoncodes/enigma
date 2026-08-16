use std::{
    collections::BTreeSet,
    env, fs,
    path::{Component, Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result, bail};

use crate::options::{Options, ReportKind};

mod capture;
mod collect;
mod model;
mod render;

const MANIFEST: &str = ".battle-report-generated.json";

pub(crate) fn generate(
    options: &Options,
    db: &config::GameDB,
    battle_catalog: battle::catalog::BattleCatalog,
) -> Result<()> {
    let kind = options.battle_report.expect("report mode selected");
    let output = absolute_output(
        options
            .report_dir
            .as_deref()
            .expect("report directory selected"),
    )?;
    let staging = prepare_staging(&output)?;
    let result = generate_into(&staging, kind, options, db, battle_catalog, &output)
        .and_then(|()| promote(&output, &staging));
    if result.is_err() && staging.exists() {
        cleanup_owned_directory(&staging).context("clean failed report staging directory")?;
    }
    result?;
    println!("battle-report output={}", output.display());
    Ok(())
}

fn generate_into(
    output: &Path,
    kind: ReportKind,
    options: &Options,
    db: &config::GameDB,
    battle_catalog: battle::catalog::BattleCatalog,
    final_output: &Path,
) -> Result<()> {
    let mut generated = vec!["battle-support.md".to_owned()];
    let evidence = capture::Evidence::collect(&options.capture_roots);
    let wire_evidence = crate::wire_evidence::Evidence::collect(db, &options.capture_roots);

    let include_heroes = matches!(kind, ReportKind::All | ReportKind::Hero);
    let include_psychubes = matches!(kind, ReportKind::All | ReportKind::Psychube);
    let include_stages = matches!(kind, ReportKind::All | ReportKind::Stage);

    if matches!(kind, ReportKind::All) {
        render::overview(output, true, true, true)?;
    }
    if include_heroes {
        let subjects = collect::heroes(
            db,
            battle_catalog,
            &options.hero_ids,
            &evidence,
            &wire_evidence,
        )?;
        render::heroes(output, &subjects)?;
        generated.push("hero-support.md".to_owned());
        generated.extend(
            subjects
                .iter()
                .map(|subject| format!("hero-support/{}-{}.md", subject.id, subject.slug)),
        );
        println!("battle-report heroes={}", subjects.len());
    }
    if include_psychubes {
        let subjects = collect::psychubes(
            db,
            battle_catalog,
            options.psychube_id,
            &evidence,
            &wire_evidence,
        )?;
        render::psychubes(output, &subjects)?;
        generated.push("psychube-support.md".to_owned());
        generated.extend(
            subjects
                .iter()
                .map(|subject| format!("psychube-support/{}-{}.md", subject.id, subject.slug)),
        );
        println!("battle-report psychubes={}", subjects.len());
    }
    if include_stages {
        let subjects = collect::stages(
            db,
            battle_catalog,
            options.episode_id,
            &evidence,
            &wire_evidence,
        )?;
        render::stages(output, &subjects)?;
        generated.push("stage-support.md".to_owned());
        generated.extend(
            subjects
                .iter()
                .map(|subject| format!("stage-support/{}-{}.md", subject.id, subject.slug)),
        );
        println!("battle-report stages={}", subjects.len());
    }
    if !matches!(kind, ReportKind::All) {
        render::overview(output, include_heroes, include_psychubes, include_stages)?;
    }
    generated.sort();
    generated.dedup();
    fs::write(
        output.join(MANIFEST),
        serde_json::to_string_pretty(&generated)?,
    )
    .context("write generated report manifest")?;
    validate_tree(output)?;
    if output == final_output {
        bail!("report staging directory aliases the final output");
    }
    Ok(())
}

fn absolute_output(output: &Path) -> Result<PathBuf> {
    if output.as_os_str().is_empty()
        || output.file_name().is_none()
        || output
            .components()
            .any(|component| matches!(component, Component::CurDir | Component::ParentDir))
    {
        bail!("report output must name a dedicated directory");
    }
    let current = env::current_dir()
        .context("resolve current directory")?
        .canonicalize()
        .context("canonicalize current directory")?;
    let output = if output.is_absolute() {
        output.to_owned()
    } else {
        current.join(output)
    };
    if output.file_name().is_none() {
        bail!("report output must not be a filesystem root");
    }
    let parent = output
        .parent()
        .context("report output has no parent directory")?;
    ensure_normal_directory(parent)?;
    if output.exists() {
        if output
            .canonicalize()
            .context("canonicalize report output")?
            == current
        {
            bail!("report output must not be the current working directory");
        }
        let metadata = fs::symlink_metadata(&output)
            .with_context(|| format!("inspect report output {}", output.display()))?;
        if !metadata.is_dir() || crate::wire_evidence::is_link_or_reparse(&metadata) {
            bail!("report output must be a normal directory");
        }
        validate_tree(&output)?;
    }
    Ok(output)
}

fn prepare_staging(output: &Path) -> Result<PathBuf> {
    let staging = sibling(output, "staging")?;
    fs::create_dir(&staging)
        .with_context(|| format!("create report staging directory {}", staging.display()))?;
    let result = if output.exists() {
        load_owned(output).and_then(|owned| copy_unowned(output, &staging, Path::new(""), &owned))
    } else {
        Ok(())
    };
    if let Err(error) = result {
        cleanup_owned_directory(&staging)?;
        return Err(error);
    }
    Ok(staging)
}

fn load_owned(output: &Path) -> Result<BTreeSet<PathBuf>> {
    let manifest = output.join(MANIFEST);
    if !manifest.is_file() {
        return Ok(BTreeSet::new());
    }
    let paths: Vec<String> = serde_json::from_str(
        &fs::read_to_string(&manifest).context("read generated report manifest")?,
    )
    .context("parse generated report manifest")?;
    paths
        .into_iter()
        .map(|path| {
            let path = PathBuf::from(path);
            validate_relative(&path)?;
            Ok(path)
        })
        .collect()
}

fn copy_unowned(
    source_root: &Path,
    destination_root: &Path,
    relative: &Path,
    owned: &BTreeSet<PathBuf>,
) -> Result<()> {
    let source = source_root.join(relative);
    for entry in fs::read_dir(&source)
        .with_context(|| format!("read report directory {}", source.display()))?
    {
        let entry = entry?;
        let child_relative = relative.join(entry.file_name());
        if child_relative == Path::new(MANIFEST) || owned.contains(&child_relative) {
            continue;
        }
        let metadata = fs::symlink_metadata(entry.path())?;
        if crate::wire_evidence::is_link_or_reparse(&metadata) {
            bail!("report output contains a link or reparse point");
        }
        let destination = destination_root.join(&child_relative);
        if metadata.is_dir() {
            fs::create_dir(&destination)
                .with_context(|| format!("create preserved directory {}", destination.display()))?;
            copy_unowned(source_root, destination_root, &child_relative, owned)?;
        } else if metadata.is_file() {
            fs::copy(entry.path(), &destination)
                .with_context(|| format!("preserve report file {}", child_relative.display()))?;
        } else {
            bail!("report output contains an unsupported filesystem entry");
        }
    }
    Ok(())
}

fn promote(output: &Path, staging: &Path) -> Result<()> {
    validate_tree(staging)?;
    if !output.exists() {
        return fs::rename(staging, output).context("install generated report");
    }
    let backup = sibling(output, "backup")?;
    fs::rename(output, &backup).context("move previous report to backup")?;
    if let Err(error) = fs::rename(staging, output) {
        fs::rename(&backup, output).context("restore previous report after install failure")?;
        return Err(error).context("install generated report");
    }
    cleanup_owned_directory(&backup).context("remove previous report backup")
}

fn sibling(output: &Path, purpose: &str) -> Result<PathBuf> {
    let parent = output.parent().context("report output has no parent")?;
    let name = output
        .file_name()
        .and_then(|name| name.to_str())
        .context("report output name is not valid Unicode")?;
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .context("system clock predates Unix epoch")?
        .as_nanos();
    let path = parent.join(format!(
        ".{name}.battle-report-{purpose}-{}-{nonce}",
        std::process::id()
    ));
    if path.exists() {
        bail!("temporary report path already exists");
    }
    Ok(path)
}

fn cleanup_owned_directory(path: &Path) -> Result<()> {
    let metadata = fs::symlink_metadata(path)
        .with_context(|| format!("inspect owned report directory {}", path.display()))?;
    if !metadata.is_dir() || crate::wire_evidence::is_link_or_reparse(&metadata) {
        bail!("refusing to remove unsafe report directory");
    }
    validate_tree(path)?;
    fs::remove_dir_all(path)
        .with_context(|| format!("remove owned report directory {}", path.display()))
}

fn ensure_normal_directory(path: &Path) -> Result<()> {
    let mut current = PathBuf::new();
    for component in path.components() {
        current.push(component.as_os_str());
        if !current.exists() {
            fs::create_dir(&current)
                .with_context(|| format!("create report path component {}", current.display()))?;
        }
        let metadata = fs::symlink_metadata(&current)
            .with_context(|| format!("inspect report path component {}", current.display()))?;
        if crate::wire_evidence::is_link_or_reparse(&metadata) {
            bail!("report path contains a link or reparse point");
        }
        if !metadata.is_dir() {
            bail!("report path contains a non-directory component");
        }
    }
    Ok(())
}

fn validate_tree(path: &Path) -> Result<()> {
    let metadata = fs::symlink_metadata(path)?;
    if crate::wire_evidence::is_link_or_reparse(&metadata) {
        bail!("report tree contains a link or reparse point");
    }
    if metadata.is_dir() {
        for entry in fs::read_dir(path)? {
            validate_tree(&entry?.path())?;
        }
    }
    Ok(())
}

fn validate_relative(path: &Path) -> Result<()> {
    if path.as_os_str().is_empty()
        || path.is_absolute()
        || path
            .components()
            .any(|part| !matches!(part, Component::Normal(_)))
    {
        bail!("unsafe path in generated report manifest");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
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
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("battle-support.md"), "mine").unwrap();
        let staging = prepare_staging(&root).unwrap();

        assert!(render::overview(&staging, true, false, false).is_err());
        assert_eq!(
            fs::read_to_string(root.join("battle-support.md")).unwrap(),
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
}
