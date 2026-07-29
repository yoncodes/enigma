use crate::{GameDB, summon::Summon};

impl GameDB {
    pub fn summon_entries(&self, pool_id: i32) -> impl Iterator<Item = &Summon> {
        self.summon.iter().filter(move |row| row.id == pool_id)
    }
}
