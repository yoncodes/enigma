use sonettobuf::Fight;

use crate::engine::fight::rules::{AdditionRuleType, OwnedBattleSkill};

#[derive(Debug, Clone, Default)]
pub struct BattleRuleManager {
    owned_skills: Vec<(i64, i32)>,
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
        Self { owned_skills }
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
}
