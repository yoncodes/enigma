use crate::{GameDB, summon::Summon, summon_pool::SummonPool};

impl GameDB {
    pub fn summon_entries(&self, pool_id: i32) -> impl Iterator<Item = &Summon> {
        self.summon.iter().filter(move |row| row.id == pool_id)
    }

    pub fn current_summon_pools(&self) -> impl Iterator<Item = &SummonPool> {
        let version = self
            .summon_pool
            .iter()
            .max_by_key(|pool| pool.id)
            .and_then(|pool| summon_version(&pool.prefab_path));

        self.summon_pool
            .iter()
            .filter(move |pool| version.is_some() && summon_version(&pool.prefab_path) == version)
    }
}

fn summon_version(prefab_path: &str) -> Option<&str> {
    prefab_path
        .split(['/', '\\'])
        .next()
        .filter(|version| version.starts_with("version_"))
}
