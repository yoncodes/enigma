use std::collections::HashSet;

use sonettobuf::Fight;

use crate::engine::fight::rules::{AdditionRuleType, OwnedBattleSkill};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MaxRoundCommand {
    pub increment: i32,
    pub cap: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ImmediateReservePromotionCommand {
    pub team_type: i32,
}

#[derive(Debug, Clone, Default)]
pub struct BattleRuleManager {
    owned_skills: Vec<(i64, i32)>,
    max_round: Option<i32>,
    immediate_reserve_promotion_teams: HashSet<i32>,
}

impl BattleRuleManager {
    pub fn seed(fight: &Fight) -> Self {
        let Some(catalog) = crate::catalog::BattleCatalog::try_global() else {
            return Self::default();
        };
        Self::seed_with_catalog(catalog, fight)
    }

    pub(crate) fn seed_with_catalog(catalog: crate::catalog::BattleCatalog, fight: &Fight) -> Self {
        let rules = catalog.battle_rules(fight);
        let owned_skills = rules
            .iter()
            .filter(|rule| rule.rule_type == AdditionRuleType::FightSkill)
            .flat_map(|rule| {
                rule.side
                    .owner_uids()
                    .iter()
                    .map(move |owner_uid| (*owner_uid, rule.skill_id))
            })
            .collect::<Vec<_>>();
        Self {
            owned_skills,
            max_round: catalog.battle_max_round(fight.battle_id.unwrap_or_default()),
            immediate_reserve_promotion_teams: HashSet::new(),
        }
    }

    pub fn owned_skills(&self) -> impl Iterator<Item = (i64, i32)> + '_ {
        self.owned_skills.iter().copied()
    }

    pub fn extend_owned_skills(&mut self, skills: impl IntoIterator<Item = OwnedBattleSkill>) {
        for skill in skills {
            let owned = (skill.owner_uid, skill.skill_id);
            if !self.owned_skills.contains(&owned) {
                self.owned_skills.push(owned);
            }
        }
    }

    pub fn max_round(&self) -> Option<i32> {
        self.max_round
    }

    pub fn enable_immediate_reserve_promotion(
        &mut self,
        command: ImmediateReservePromotionCommand,
    ) {
        self.immediate_reserve_promotion_teams
            .insert(command.team_type);
    }

    pub fn promotes_reserves_immediately(&self, team_type: i32) -> bool {
        self.immediate_reserve_promotion_teams.contains(&team_type)
    }

    pub fn add_max_round(
        &mut self,
        command: MaxRoundCommand,
    ) -> Option<crate::engine::skill::rule::output::EffectMarker> {
        let current = self.max_round?;
        let next = current.saturating_add(command.increment).min(command.cap);
        let applied = next.saturating_sub(current);
        if applied <= 0 {
            return None;
        }
        self.max_round = Some(next);
        Some(crate::engine::skill::rule::output::EffectMarker {
            target_uid: 0,
            effect_type: sonettobuf::effect_type_enum::EffectType::Addmaxround as i32,
            effect_num: applied,
            effect_num1: None,
            config_effect: 0,
            reserve_id: None,
            reserve_str: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extending_rules_preserves_configured_order_and_deduplicates() {
        let mut manager = BattleRuleManager::default();
        manager.extend_owned_skills([
            OwnedBattleSkill {
                owner_uid: 0,
                skill_id: 20,
            },
            OwnedBattleSkill {
                owner_uid: 0,
                skill_id: 10,
            },
            OwnedBattleSkill {
                owner_uid: 0,
                skill_id: 20,
            },
        ]);

        assert_eq!(
            manager.owned_skills().collect::<Vec<_>>(),
            vec![(0, 20), (0, 10)]
        );
    }

    #[test]
    fn max_round_increase_uses_the_battle_base_and_configured_cap() {
        crate::test_support::init_config();
        let catalog = crate::catalog::BattleCatalog::new(crate::test_support::game_data());
        let fight = Fight {
            battle_id: Some(1_211),
            ..Default::default()
        };
        let mut manager = BattleRuleManager::seed_with_catalog(catalog, &fight);

        let marker = manager
            .add_max_round(MaxRoundCommand {
                increment: 1,
                cap: 30,
            })
            .unwrap();
        assert_eq!(manager.max_round(), Some(21));
        assert_eq!(marker.effect_num, 1);
        assert_eq!(marker.config_effect, 0);

        manager.add_max_round(MaxRoundCommand {
            increment: 20,
            cap: 30,
        });
        assert_eq!(manager.max_round(), Some(30));
    }
}
