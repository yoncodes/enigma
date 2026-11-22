use sonettobuf::{BuffInfo, Fight, FightEntityInfo, FightTeam, HeroAttribute};

use super::*;
use crate::engine::skill::rule::{DefinitionKey, RuleDomain};

#[test]
fn repeated_settlements_compound_from_the_exact_buff_act_state() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                team_type: Some(1),
                current_hp: Some(1_000),
                attr: Some(HeroAttribute {
                    attack: Some(1_000),
                    ..Default::default()
                }),
                ..Default::default()
            }],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                team_type: Some(2),
                current_hp: Some(10_000),
                buffs: vec![BuffInfo {
                    uid: Some(30),
                    buff_id: Some(31040001),
                    from_uid: Some(10),
                    layer: Some(1),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let managers = BattleManagers::seeded(&fight);
    let feature = managers
        .buff
        .active_features(&managers.hp)
        .into_iter()
        .find(|feature| buff_act::is_kind(feature, BuffActKind::DeadlyPoison))
        .unwrap();
    let origin = CommandOrigin {
        domain: RuleDomain::Behavior,
        key: DefinitionKey::new(60111, "ConsumePoisonSettleDeadlyPoison"),
    };

    let ops = settlement_ops(&managers, &feature, origin, 2).unwrap();

    assert!(matches!(
        ops.as_slice(),
        [
            RuleOp::Command(BattleCommand::Hp(HpCommand::Lose(HpLoss {
                amount: 200,
                ..
            }))),
            RuleOp::Command(BattleCommand::Buff(BuffCommand::AccumulateActValue(_))),
            RuleOp::Command(BattleCommand::Hp(HpCommand::Lose(HpLoss {
                amount: 280,
                ..
            }))),
            RuleOp::Command(BattleCommand::Buff(BuffCommand::AccumulateActValue(_)))
        ]
    ));
}
