use std::{collections::BTreeSet, fs, path::Path};

use serde_json::Value;

use super::{find_fight, is_link_or_reparse, is_setup_name};

pub(super) type FightIdentity = (i32, i64, i64);
type IdentityBoundary = (String, Result<FightIdentity, String>);

pub(super) struct IdentityIndex {
    boundaries: Vec<IdentityBoundary>,
}

impl IdentityIndex {
    pub(super) fn collect(directory: &Path) -> Result<Self, String> {
        let mut boundaries = Vec::new();
        for entry in fs::read_dir(directory)
            .map_err(|error| format!("{}: {error}", directory.display()))?
            .flatten()
        {
            let name = entry.file_name().to_string_lossy().into_owned();
            if !is_setup_name(&name) {
                continue;
            }
            let metadata = match fs::symlink_metadata(entry.path()) {
                Ok(metadata) => metadata,
                Err(error) => {
                    boundaries.push((name.clone(), Err(format!("setup {name}: {error}"))));
                    continue;
                }
            };
            if !metadata.is_file() || is_link_or_reparse(&metadata) {
                continue;
            }
            let identity =
                read_identity(&entry.path()).map_err(|error| format!("setup {name}: {error}"));
            boundaries.push((name, identity));
        }
        boundaries.sort_by(|left, right| left.0.cmp(&right.0));
        Ok(Self { boundaries })
    }

    pub(super) fn response(&self, response: &Path) -> Result<Option<FightIdentity>, String> {
        let response_name = response
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("");
        if let Some(order) = prefix(response_name) {
            return self
                .boundaries
                .iter()
                .rev()
                .find(|(name, _)| prefix(name) == Some(order) && name.as_str() <= response_name)
                .map_or(Ok(None), |(_, identity)| identity.clone().map(Some));
        }
        if let Some(error) = self
            .boundaries
            .iter()
            .find_map(|(_, identity)| identity.as_ref().err())
        {
            return Err(error.clone());
        }
        let identities = self
            .boundaries
            .iter()
            .filter_map(|(_, identity)| identity.as_ref().ok().copied())
            .collect::<BTreeSet<_>>();
        match identities.len() {
            0 => Ok(None),
            1 => Ok(identities.into_iter().next()),
            _ => Err(format!(
                "multiple fight identities lack an ordered setup prefix: {}",
                self.boundaries
                    .iter()
                    .map(|(name, _)| name.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            )),
        }
    }
}

fn read_identity(path: &Path) -> Result<FightIdentity, String> {
    let contents = fs::read_to_string(path).map_err(|error| error.to_string())?;
    let value: Value = serde_json::from_str(&contents).map_err(|error| error.to_string())?;
    let fight = find_fight(&value).ok_or_else(|| "missing fight with version".to_owned())?;
    let field = |name| {
        fight
            .get(name)
            .and_then(Value::as_i64)
            .ok_or_else(|| format!("missing or noninteger fight.{name}"))
    };
    Ok((
        i32::try_from(field("version")?).map_err(|error| format!("fight.version: {error}"))?,
        field("episodeId")?,
        field("battleId")?,
    ))
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Prefix {
    Timestamp,
    Sequence,
}

fn prefix(name: &str) -> Option<Prefix> {
    let (first, rest) = name.split_once('_')?;
    if !first.bytes().all(|value| value.is_ascii_digit()) {
        return None;
    }
    if first.len() == 6 {
        return Some(Prefix::Sequence);
    }
    let time = rest.split('_').next()?;
    (first.len() == 8 && time.len() >= 6 && time.bytes().all(|value| value.is_ascii_digit()))
        .then_some(Prefix::Timestamp)
}

#[cfg(test)]
mod tests;
