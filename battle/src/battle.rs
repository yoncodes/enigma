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
    fn joe_euphoria_and_device_party_start_with_their_real_configured_skills() {
        use crate::dungeon::{BattleFighter, BattleRoster, FightOptions, HeroBuildInput};
        let catalog = BattleCatalog::new(crate::test_support::game_data());
        let fighters = [(3094, 309401, 4, 309401), (3149, 314901, 0, 0)]
            .into_iter().enumerate().map(|(index, (hero_id, skin, destiny_rank, destiny_stone))| BattleFighter {
                hero: HeroBuildInput { uid: 30_000_001 + index as i64, user_id: 1, hero_id, skin,
                    level: 180, rank: 4, ex_skill_level: 5, talent: 15, destiny_rank, destiny_stone,
                    ..Default::default() }, equips: vec![],
            }).collect::<Vec<_>>();
        let group = sonettobuf::FightGroup {
            hero_list: fighters.iter().map(|fighter| fighter.hero.uid).collect(), ..Default::default()
        };
        let roster = BattleRoster { user_id: 1, fighters, ..Default::default() };
        let built = crate::dungeon::build_fight(catalog, &roster, 20411, 2411,
            &group, FightOptions::default(), None).unwrap();
        let joe = built.fight.attacker.as_ref().unwrap().entitys.iter()
            .find(|hero| hero.model_id == Some(3094)).expect("J must be in the team");
        assert!(joe.passive_skill.iter().any(|id| [30941141, 30941142, 30941143].contains(id)));
        assert!(joe.skill_group2.contains(&30941124), "J must use the evolved shield card");
        let battle = Battle::start(catalog, built, 42).expect("J and the device team must enter battle");
        let joe = battle.entity_info(30_000_001).unwrap();
        assert!(joe.buffs.iter().any(|buff| buff.buff_id == Some(30940001)),
            "J's evolved burn protection must be applied by the real passive");
    }

    #[test]
    fn fifth_chapter_ninth_stage_starts_with_its_configured_enemies() {
        use crate::dungeon::{BattleFighter, BattleRoster, FightOptions, HeroBuildInput};
        let catalog = BattleCatalog::new(crate::test_support::game_data());
        let fighters = [(3079, 307901), (3053, 305301), (3039, 303901)]
            .into_iter().enumerate().map(|(index, (hero_id, skin))| BattleFighter {
                hero: HeroBuildInput { uid: 30_000_001 + index as i64, user_id: 1,
                    hero_id, skin, level: 60, rank: 2, ex_skill_level: 1, ..Default::default() },
                equips: vec![],
            }).collect::<Vec<_>>();
        let group = sonettobuf::FightGroup {
            hero_list: fighters.iter().map(|fighter| fighter.hero.uid).collect(),
            ..Default::default()
        };
        let roster = BattleRoster { user_id: 1, fighters, ..Default::default() };
        let built = crate::dungeon::build_fight(catalog, &roster, 10509, 1508,
            &group, FightOptions::default(), None).unwrap();
        let enemies = built.fight.defender.as_ref().unwrap().entitys.iter()
            .filter_map(|enemy| enemy.model_id).collect::<Vec<_>>();
        assert_eq!(enemies, vec![150801, 150802, 150803]);
        let battle = Battle::start(catalog, built, 42).expect("5TH-9 opening must not reject target 2001");
        assert_eq!(battle.current_round(), 1);
    }

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

    #[test]
    fn upgraded_an_an_party_can_start_on_a_server_sized_worker_stack() {
        std::thread::Builder::new().stack_size(32 * 1024 * 1024).spawn(|| {
            use crate::dungeon::{BattleFighter, BattleRoster, FightOptions, HeroBuildInput};
            let catalog = BattleCatalog::new(crate::test_support::game_data());
            let specs = [(3141, 0, 0, 314102), (3146, 0, 0, 314602),
                (3053, 4, 305301, 305304), (3039, 4, 303902, 303904)];
            let fighters = specs.into_iter().enumerate().map(|(index, (hero_id, destiny_rank, destiny_stone, skin))| {
                BattleFighter { hero: HeroBuildInput { uid: 20_000_001 + index as i64,
                    user_id: 1, hero_id, skin, level: 180, rank: 4, ex_skill_level: 5,
                    talent: 15, destiny_rank, destiny_stone, ..Default::default() }, equips: vec![] }
            }).collect::<Vec<_>>();
            let group = sonettobuf::FightGroup { hero_list: fighters.iter().map(|fighter| fighter.hero.uid).collect(),
                ..Default::default() };
            let roster = BattleRoster { user_id: 1, fighters, ..Default::default() };
            let built = crate::dungeon::build_fight(catalog, &roster, 2290109, 116380000,
                &group, FightOptions::default(), None).unwrap();
            let battle = Battle::start(catalog, built, 42).unwrap();
            assert_eq!(battle.current_round(), 1);
        }).unwrap().join().unwrap();
    }

    #[test]
    fn reported_four_hero_party_starts_a_configured_battle() {
        std::thread::Builder::new().stack_size(32 * 1024 * 1024).spawn(|| {
            use crate::dungeon::{BattleFighter, BattleRoster, FightOptions, HeroBuildInput};
            let catalog = BattleCatalog::new(crate::test_support::game_data());
            let specs = [(3120, 312001), (3088, 308801), (3126, 312601), (3125, 312501)];
            for (level, rank, ex_skill_level, talent) in [(1, 1, 1, 0), (180, 4, 5, 15)] {
                let fighters = specs.into_iter().enumerate().map(|(index, (hero_id, skin))| {
                    BattleFighter { hero: HeroBuildInput {
                        uid: 30_000_001 + index as i64, user_id: 1, hero_id, skin,
                        level, rank, ex_skill_level, talent, ..Default::default()
                    }, equips: vec![] }
                }).collect::<Vec<_>>();
                let group = sonettobuf::FightGroup {
                    hero_list: fighters.iter().map(|fighter| fighter.hero.uid).collect(),
                    cloth_id: Some(7), ..Default::default()
                };
                let roster = BattleRoster { user_id: 1, fighters, ..Default::default() };
                // This ordinary episode allows four player-controlled heroes.
                let built = crate::dungeon::build_fight(catalog, &roster, 20411, 2411,
                    &group, FightOptions::default(), None).unwrap();
                let models = built.fight.attacker.as_ref().unwrap().entitys.iter()
                    .filter_map(|entity| entity.model_id).collect::<Vec<_>>();
                for (hero_id, _) in specs { assert!(models.contains(&hero_id), "hero {hero_id} was not included"); }
                let battle = Battle::start(catalog, built, 42)
                    .unwrap_or_else(|error| panic!("rank {rank} team could not start: {error}"));
                assert_eq!(battle.current_round(), 1);
            }
        }).unwrap().join().unwrap();
    }

}
