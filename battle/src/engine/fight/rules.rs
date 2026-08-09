use sonettobuf::Fight;

pub const ATTACKER_SIDE_UID: i64 = 0;
pub const DEFENDER_SIDE_UID: i64 = -99_999;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct OwnedBattleSkill {
    pub owner_uid: i64,
    pub skill_id: i32,
}

pub fn is_side_uid(uid: i64) -> bool {
    matches!(uid, ATTACKER_SIDE_UID | DEFENDER_SIDE_UID)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdditionRuleType {
    Skill,
    Level,
    TimeLimit,
    AmountLimit,
    DeadLimit,
    FightSkill,
}

impl AdditionRuleType {
    pub(crate) fn from_id(id: i32) -> Option<Self> {
        Some(match id {
            1 => Self::Skill,
            2 => Self::Level,
            3 => Self::TimeLimit,
            4 => Self::AmountLimit,
            5 => Self::DeadLimit,
            6 => Self::FightSkill,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BattleRuleSide {
    Attacker,
    Defender,
    Both,
}

impl BattleRuleSide {
    pub(crate) fn from_id(id: i32) -> Option<Self> {
        Some(match id {
            1 => Self::Attacker,
            2 => Self::Defender,
            3 => Self::Both,
            _ => return None,
        })
    }

    pub fn owner_uids(self) -> &'static [i64] {
        match self {
            Self::Attacker => &[ATTACKER_SIDE_UID],
            Self::Defender => &[DEFENDER_SIDE_UID],
            Self::Both => &[ATTACKER_SIDE_UID, DEFENDER_SIDE_UID],
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConfiguredBattleRule {
    pub rule_id: i32,
    pub skill_id: i32,
    pub side: BattleRuleSide,
    pub rule_type: AdditionRuleType,
}

pub fn configured(fight: &Fight) -> Vec<ConfiguredBattleRule> {
    crate::catalog::BattleCatalog::try_global()
        .map(|catalog| catalog.battle_rules(fight))
        .unwrap_or_default()
}

pub fn configured_fight_skills(fight: &Fight) -> impl Iterator<Item = ConfiguredBattleRule> {
    configured(fight)
        .into_iter()
        .filter(|rule| rule.rule_type == AdditionRuleType::FightSkill)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_catalog_preserves_rule_side_and_order() {
        crate::test_support::init_config();
        let fight = Fight {
            battle_id: Some(9_000_303),
            ..Default::default()
        };
        let catalog = crate::catalog::BattleCatalog::new(crate::test_support::game_data());
        let rules = catalog.battle_rules(&fight);

        assert_eq!(configured(&fight), rules);
        assert_eq!(
            rules
                .iter()
                .map(|rule| (rule.side, rule.rule_id, rule.skill_id, rule.rule_type))
                .collect::<Vec<_>>(),
            vec![
                (
                    BattleRuleSide::Defender,
                    370_003_003,
                    370_003_003,
                    AdditionRuleType::Skill
                ),
                (
                    BattleRuleSide::Attacker,
                    22_301_961,
                    22_301_961,
                    AdditionRuleType::Skill
                ),
                (
                    BattleRuleSide::Attacker,
                    90_120_002,
                    90_120_002,
                    AdditionRuleType::Skill
                ),
                (
                    BattleRuleSide::Defender,
                    22_301_962,
                    22_301_962,
                    AdditionRuleType::Skill
                ),
                (
                    BattleRuleSide::Defender,
                    370_003_013,
                    370_003_013,
                    AdditionRuleType::Skill
                ),
            ]
        );
    }
}
