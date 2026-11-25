use anyhow::{Context, Result, ensure};

pub(crate) struct MonsterHealth {
    pub model_id: i32,
    pub current_hp: i32,
    pub max_hp: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct MonsterHealthRule {
    models: Vec<i32>,
    below_per_mille: i32,
}

impl MonsterHealthRule {
    pub(super) fn parse(models: Option<&str>, threshold: Option<&str>) -> Result<Self> {
        let models = models.context("MonsterBelowHp is missing model ids")?;
        let models = super::parse_model_ids(models)?;
        let below_per_mille = super::parse_i32(threshold, "health threshold")?;
        ensure!(
            (1..=1000).contains(&below_per_mille),
            "health threshold must be between 1 and 1000 per mille"
        );
        Ok(Self {
            models,
            below_per_mille,
        })
    }

    pub(super) fn reached(&self, monster: MonsterHealth) -> bool {
        self.models.contains(&monster.model_id)
            && monster.max_hp > 0
            && i64::from(monster.current_hp) * 1000
                < i64::from(monster.max_hp) * i64::from(self.below_per_mille)
    }
}
