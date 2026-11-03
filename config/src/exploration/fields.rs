use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Context, Result, ensure};
use serde_json::Value;

pub(super) struct Fields(BTreeMap<String, usize>);

impl Fields {
    pub fn read(input: &str) -> Result<Self> {
        let mut fields = BTreeMap::new();
        let mut positions = BTreeSet::new();
        for line in input.lines() {
            let Some(declaration) = line.trim().strip_prefix("ExploreUnitMoFieldEnum.") else {
                continue;
            };
            let (name, position) = declaration
                .split_once('=')
                .context("invalid map field declaration")?;
            let position = position
                .trim()
                .parse::<usize>()?
                .checked_sub(1)
                .context("map field positions must be positive")?;
            ensure!(positions.insert(position), "duplicate map field position");
            ensure!(
                fields.insert(name.trim().to_owned(), position).is_none(),
                "duplicate map field name"
            );
        }
        ensure!(!fields.is_empty(), "map field declarations are missing");
        Ok(Self(fields))
    }

    pub fn unit(&self, row: Vec<Value>) -> Result<super::Unit> {
        let values = self
            .0
            .iter()
            .filter_map(|(name, &index)| row.get(index).cloned().map(|value| (name.clone(), value)))
            .collect();
        serde_json::from_value(Value::Object(values)).context("invalid exploration unit data")
    }
}
