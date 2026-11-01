use super::*;

#[test]
fn team_buff_layers_scale_the_target_grant() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(100),
                team_type: Some(1),
                buffs: vec![BuffInfo {
                    uid: Some(11),
                    buff_id: Some(30920101),
                    layer: Some(3),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![
                FightEntityInfo {
                    uid: Some(-1),
                    current_hp: Some(100),
                    team_type: Some(2),
                    buffs: vec![BuffInfo {
                        uid: Some(21),
                        buff_id: Some(30920102),
                        layer: Some(20),
                        ..Default::default()
                    }],
                    ..Default::default()
                },
                FightEntityInfo {
                    uid: Some(-2),
                    current_hp: Some(100),
                    team_type: Some(2),
                    buffs: vec![BuffInfo {
                        uid: Some(22),
                        buff_id: Some(30920102),
                        layer: Some(10),
                        ..Default::default()
                    }],
                    ..Default::default()
                },
            ],
            ..Default::default()
        }),
        ..Default::default()
    };
    let managers = BattleManagers::seeded(&fight);
    let pool = TargetPool::from_fight(&fight);
    let behavior = ParsedBehavior::from_spec(
        crate::engine::skill::behavior::classify::BehaviorSpec::new(
            60261,
            "PerBuffTypeIdAddRateBuffId",
        ),
        vec![30920101, 80, 30920102],
        vec!["30920101".into(), "80".into(), "30920102".into()],
    );
    let mut determinism = RoundDeterminism::default();
    let mut modifiers = crate::engine::skill::action::SkillModifiers::default();
    let mut target = crate::engine::skill::target::TargetContext::default();

    let ops = super::super::super::rule_ops(
        BehaviorOpContext {
            source_uid: 10,
            source_team: 1,
            target_uid: -1,
            active_skill_id: 30920151,
            transfer_count: 1,
            event: None,
            managers: &managers,
            pool: &pool,
            determinism: &mut determinism,
            modifiers: &mut modifiers,
            target: &mut target,
        },
        &behavior,
    )
    .unwrap();

    assert!(matches!(
        ops.as_slice(),
        [RuleOp::Command(BattleCommand::Buff(BuffCommand::Grant(
            BuffGrant {
                target_uid: -1,
                buff_id: 30920102,
                amount: Some(7),
                ..
            }
        )))]
    ));
}
