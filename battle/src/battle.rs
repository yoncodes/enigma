use sonettobuf::{
    AutoRoundReply, AutoRoundRequest, CardInfo, CardInfoPush, Fight, FightRound, FightStatistics,
    RedealCardInfoPush, UseClothSkillReply, UseClothSkillRequest,
};

use crate::{
    BattleOutcome,
    catalog::BattleCatalog,
    dungeon::BuiltFight,
    engine::runtime::{BattleRuntime, determinism::RoundDeterminism},
};

/// One authoritative battle simulation exposed through server-facing operations.
#[derive(Debug, Clone)]
pub struct Battle {
    runtime: BattleRuntime,
}

impl Battle {
    /// Builds and starts a configured fight with deterministic round state.
    pub fn start(catalog: BattleCatalog, built: BuiltFight, seed: u64) -> Result<Self, String> {
        let BuiltFight {
            fight,
            ex_attributes,
            sp_attributes,
            battle_rule_skills,
        } = built;
        let mut runtime =
            BattleRuntime::new_with_attributes(catalog, fight, ex_attributes, sp_attributes);
        runtime.extend_battle_rule_skills(battle_rule_skills);
        runtime.start_round_with_determinism(RoundDeterminism::with_seed(seed))?;
        Ok(Self { runtime })
    }

    pub fn outcome(&self) -> BattleOutcome {
        self.runtime.outcome()
    }

    pub fn current_round(&self) -> i32 {
        self.runtime.current_round()
    }

    pub fn fight_version(&self) -> i32 {
        self.runtime.fight_version()
    }

    pub fn reconnect_state(&self) -> (Fight, Option<FightRound>) {
        self.runtime.reconnect_state()
    }

    pub fn defeated_defender_count(&self) -> usize {
        self.runtime.defeated_defender_count()
    }

    pub fn card_deck(&self, team_type: i32) -> Option<&[CardInfo]> {
        self.runtime.card_deck(team_type)
    }

    pub fn entity_info(&self, uid: i64) -> Option<sonettobuf::FightEntityInfo> {
        self.runtime.entity_info(uid)
    }

    pub fn activity_score(&self) -> i32 {
        self.runtime.activity_score()
    }

    pub fn attack_statistics(&self) -> Vec<FightStatistics> {
        self.runtime.attack_statistics()
    }

    pub fn plan_auto_round(&self, request: &AutoRoundRequest) -> AutoRoundReply {
        self.runtime.plan_auto_round(request)
    }

    pub fn use_cloth_skill(&mut self, request: UseClothSkillRequest) -> Option<UseClothSkillReply> {
        self.runtime.use_cloth_skill(request)
    }

    pub fn take_redeal_card_push(&mut self) -> Option<RedealCardInfoPush> {
        self.runtime.take_redeal_card_push()
    }

    pub fn card_info_push(&self) -> CardInfoPush {
        self.runtime.card_info_push()
    }

    pub fn meets_advanced_condition(&self, type_id: i32, attr: i32) -> Option<bool> {
        self.runtime.meets_advanced_condition(type_id, attr)
    }

    pub(crate) fn runtime(&self) -> &BattleRuntime {
        &self.runtime
    }

    pub(crate) fn runtime_mut(&mut self) -> &mut BattleRuntime {
        &mut self.runtime
    }

    pub(crate) fn from_runtime(runtime: BattleRuntime) -> Self {
        Self { runtime }
    }
}

#[cfg(test)]
mod tests {
    use sonettobuf::{FightEntityInfo, FightTeam};

    use super::*;
    use crate::engine::fight::rules::{ATTACKER_SIDE_UID, OwnedBattleSkill};

    #[test]
    fn start_preserves_runtime_setup_order() {
        let catalog = BattleCatalog::new(crate::test_support::game_data());
        let fight = Fight {
            attacker: Some(FightTeam {
                entitys: vec![FightEntityInfo {
                    uid: Some(10),
                    current_hp: Some(100),
                    ..Default::default()
                }],
                ..Default::default()
            }),
            defender: Some(FightTeam {
                entitys: vec![FightEntityInfo {
                    uid: Some(-1),
                    current_hp: Some(100),
                    ..Default::default()
                }],
                ..Default::default()
            }),
            cur_round: Some(1),
            version: Some(7),
            ..Default::default()
        };
        let seed = 42;
        let rule_skill = OwnedBattleSkill {
            owner_uid: ATTACKER_SIDE_UID,
            skill_id: 1_182_004,
        };
        let mut expected = BattleRuntime::new_with_attributes(
            catalog,
            fight.clone(),
            std::iter::empty(),
            std::iter::empty(),
        );
        expected.extend_battle_rule_skills([rule_skill]);
        expected
            .start_round_with_determinism(RoundDeterminism::with_seed(seed))
            .unwrap();

        let actual = Battle::start(
            catalog,
            BuiltFight {
                fight,
                ex_attributes: Vec::new(),
                sp_attributes: Vec::new(),
                battle_rule_skills: vec![rule_skill],
            },
            seed,
        )
        .unwrap();

        assert_eq!(actual.reconnect_state(), expected.reconnect_state());
        assert_eq!(actual.outcome(), expected.outcome());
    }
}
