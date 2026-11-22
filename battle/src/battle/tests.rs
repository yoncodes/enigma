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
    )
    .unwrap();
    expected.extend_battle_rule_skills([rule_skill]);
    expected
        .start_round_with_determinism(RoundDeterminism::with_seed(seed))
        .unwrap();

    let actual = Battle::start(
        catalog,
        BuiltFight {
            fight,
            player_level: 0,
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
