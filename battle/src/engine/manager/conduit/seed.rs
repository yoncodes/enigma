use sonettobuf::{Fight, FightEntityInfo};

use super::*;

impl ConduitManager {
    pub(crate) fn configured(catalog: crate::catalog::BattleCatalog, fight: &Fight) -> Self {
        Self::from_fight(fight, |entity| catalog.conduit_device(entity))
    }

    pub fn seed_with_game_data(game_data: &config::GameDB, fight: &Fight) -> Self {
        Self::from_fight(fight, |entity| {
            crate::catalog::configured_conduit_device(game_data, entity)
        })
    }

    fn from_fight(
        fight: &Fight,
        configured: impl Fn(&FightEntityInfo) -> Result<Option<Vec<Vec<ConduitSkill>>>, ConduitError>,
    ) -> Self {
        let mut manager = Self::default();
        for (team, fight_team) in [(1, fight.attacker.as_ref()), (2, fight.defender.as_ref())] {
            let Some(fight_team) = fight_team else {
                continue;
            };
            for entity in &fight_team.entitys {
                manager.seed_entity(&configured, team, entity);
            }
        }
        manager
    }

    #[cfg(test)]
    pub fn seed(fight: &Fight) -> Self {
        Self::seed_with_game_data(crate::test_support::game_data(), fight)
    }

    fn seed_entity(
        &mut self,
        configured: &impl Fn(&FightEntityInfo) -> Result<Option<Vec<Vec<ConduitSkill>>>, ConduitError>,
        team: i32,
        entity: &FightEntityInfo,
    ) {
        let (Some(uid), Some(_model_id)) = (entity.uid, entity.model_id) else {
            return;
        };
        let skill_groups = match configured(entity) {
            Ok(Some(skill_groups)) => skill_groups,
            Ok(None) => return,
            Err(error) => {
                self.initialization_errors.push(error);
                return;
            }
        };
        self.areas
            .entry(team)
            .or_insert_with(|| ConduitArea {
                team,
                devices: Vec::new(),
                powers: Vec::new(),
            })
            .devices
            .push(ConduitDevice {
                uid,
                selected_group: 1,
                skill_groups,
            });
    }
}
