use sonettobuf::{Fight, FightEntityInfo};

use crate::engine::fight::defender::{Defender, monster_ids};

const DEFENDER_TEAM: i32 = 2;

#[derive(Debug, Clone, PartialEq)]
pub struct WaveAdvanced {
    pub wave: i32,
    pub entering_uids: Vec<i64>,
    pub fight: Fight,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct WaveRoster {
    pub wave: i32,
    pub entering_uids: Vec<i64>,
    pub entitys: Vec<FightEntityInfo>,
    pub sub_entitys: Vec<FightEntityInfo>,
}

#[derive(Debug, Clone, Default)]
pub struct WaveManager {
    group_ids: Vec<i32>,
    current_index: usize,
    monster_max: usize,
    next_uid_offset: usize,
}

impl WaveManager {
    pub fn seed_with_catalog(catalog: crate::catalog::BattleCatalog, fight: &Fight) -> Self {
        let db = catalog.game_data();
        let Some(battle) = db.battle.get(fight.battle_id.unwrap_or_default()) else {
            return Self::default();
        };
        let group_ids = ids(&battle.monster_group_ids);
        let current_index = fight.cur_wave.unwrap_or(1).max(1) as usize - 1;
        let configured_offset: usize = group_ids
            .iter()
            .take(current_index.saturating_add(1))
            .filter_map(|group_id| db.monster_group.get(*group_id))
            .map(|group| monster_ids(&group.monster).len())
            .sum();
        let occupied_offset =
            crate::engine::skill::target::TargetPool::from_fight_with_catalog(catalog, fight)
                .entities()
                .filter_map(|entity| (entity.uid < 0).then_some(entity.uid.unsigned_abs() as usize))
                .max()
                .unwrap_or_default();
        Self {
            group_ids,
            current_index,
            monster_max: battle.monster_max.max(0) as usize,
            next_uid_offset: configured_offset.max(occupied_offset),
        }
    }

    #[cfg(test)]
    pub fn seed(fight: &Fight) -> Self {
        Self::seed_with_catalog(
            crate::catalog::BattleCatalog::new(crate::test_support::game_data()),
            fight,
        )
    }

    pub(crate) fn advance(
        &mut self,
        catalog: crate::catalog::BattleCatalog,
    ) -> anyhow::Result<Option<WaveRoster>> {
        let next_index = self.current_index.saturating_add(1);
        let Some(&group_id) = self.group_ids.get(next_index) else {
            return Ok(None);
        };
        let (entitys, sub_entitys) = Defender::build_wave(
            catalog,
            group_id,
            self.monster_max,
            DEFENDER_TEAM,
            self.next_uid_offset,
        )?;
        let entering_uids = entitys
            .iter()
            .filter_map(|entity| entity.uid)
            .collect::<Vec<_>>();
        self.next_uid_offset += entitys.len() + sub_entitys.len();
        self.current_index = next_index;

        Ok(Some(WaveRoster {
            wave: next_index as i32 + 1,
            entering_uids,
            entitys,
            sub_entitys,
        }))
    }

    pub fn has_next_wave(&self) -> bool {
        self.current_index.saturating_add(1) < self.group_ids.len()
    }
}

fn ids(value: &str) -> Vec<i32> {
    value
        .split('#')
        .filter_map(|value| value.parse().ok())
        .collect()
}

pub fn entering_entities(change: &WaveAdvanced) -> impl Iterator<Item = &FightEntityInfo> {
    change
        .fight
        .defender
        .iter()
        .flat_map(|team| team.entitys.iter().chain(&team.sub_entitys))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::init_config;

    #[test]
    fn battle_config_owns_wave_order_and_monotonic_uids() {
        init_config();
        let fight = Fight {
            battle_id: Some(2514),
            cur_wave: Some(1),
            defender: Some(sonettobuf::FightTeam::default()),
            ..Default::default()
        };
        let mut waves = WaveManager::seed(&fight);
        let catalog = crate::catalog::BattleCatalog::new(crate::test_support::game_data());

        assert!(waves.has_next_wave());

        let second = waves.advance(catalog).unwrap().unwrap();
        assert_eq!(second.wave, 2);
        assert_eq!(second.entering_uids, vec![-3, -4]);
        assert_eq!(second.entitys.len(), 2);

        let third = waves.advance(catalog).unwrap().unwrap();
        assert_eq!(third.wave, 3);
        assert_eq!(third.entering_uids, vec![-5, -6]);
        assert!(waves.has_next_wave());
    }
}
