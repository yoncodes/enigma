use sonettobuf::{BuffActInfo, BuffInfo, Fight, FightEntityInfo, FightTeam, HeroAttribute};

use crate::engine::{
    manager::{BattleManagers, hp::HpCommand},
    runtime::determinism::RoundDeterminism,
    skill::{
        action::SkillModifiers,
        behavior::{BehaviorOpContext, classify::BehaviorSpec},
        effect::ParsedBehavior,
        rule::output::{BattleCommand, RuleOp},
        target::{TargetContext, TargetPool},
    },
};

use super::*;

#[test]
fn fake_hp_heal_reads_remembrance_without_consuming_it() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![
                FightEntityInfo {
                    uid: Some(10),
                    team_type: Some(1),
                    current_hp: Some(1_000),
                    attr: Some(HeroAttribute {
                        hp: Some(1_000),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
                FightEntityInfo {
                    uid: Some(20),
                    team_type: Some(1),
                    current_hp: Some(8_117),
                    attr: Some(HeroAttribute {
                        hp: Some(10_903),
                        ..Default::default()
                    }),
                    buffs: vec![BuffInfo {
                        uid: Some(30),
                        buff_id: Some(31450143),
                        from_uid: Some(10),
                        layer: Some(1),
                        act_info: vec![BuffActInfo {
                            act_id: Some(1150),
                            param: vec![2_071, 8_108],
                            str_param: Some(String::new()),
                        }],
                        ..Default::default()
                    }],
                    ..Default::default()
                },
                FightEntityInfo {
                    uid: Some(30),
                    team_type: Some(1),
                    current_hp: Some(500),
                    attr: Some(HeroAttribute {
                        hp: Some(1_000),
                        ..Default::default()
                    }),
                    buffs: vec![BuffInfo {
                        uid: Some(40),
                        buff_id: Some(31450143),
                        from_uid: Some(10),
                        layer: Some(1),
                        act_info: vec![BuffActInfo {
                            act_id: Some(1150),
                            param: vec![1_000, 4_000],
                            str_param: Some(String::new()),
                        }],
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
    let mut determinism = RoundDeterminism::default();
    let mut modifiers = SkillModifiers::default();
    let mut target = TargetContext::default();
    let behavior = ParsedBehavior::from_spec(
        BehaviorSpec::new(60309, "FakeHpToHeal"),
        vec![200, 0],
        Vec::new(),
    );

    let ops = super::super::rule_ops(
        BehaviorOpContext {
            source_uid: 10,
            source_team: 1,
            target_uid: 10,
            active_skill_id: 31450151,
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
    .expect("FakeHpToHeal should be registered");

    assert!(matches!(
        ops.as_slice(),
        [
            RuleOp::Command(BattleCommand::Hp(HpCommand::Heal(heal))),
            RuleOp::BuffActInfoMarker(marker),
            RuleOp::Command(BattleCommand::Hp(HpCommand::Heal(other_heal))),
            RuleOp::BuffActInfoMarker(other_marker)
        ] if heal.source_uid == 10
            && heal.target_uid == 20
            && heal.amount == 414
            && heal.config_effect == 60309
            && marker.target_uid == 20
            && marker.buff_uid == 30
            && marker.act_id == 1150
            && marker.params == [2_071, 8_108]
            && other_heal.target_uid == 30
            && other_heal.amount == 200
            && other_marker.target_uid == 30
            && other_marker.buff_uid == 40
            && other_marker.params == [1_000, 4_000]
    ));
    let definition = super::super::registry::find(&behavior).expect("FakeHpToHeal definition");
    assert_eq!(
        definition.target_emission_mode,
        super::super::registry::TargetEmissionMode::Once
    );
    assert_eq!((definition.output_owner_for)(&behavior, &ops[0], 0), None);
    assert_eq!(
        (definition.output_owner_for)(&behavior, &ops[1], 1),
        Some(OutputOwner::Parent)
    );
    assert_eq!(
        crate::engine::skill::buff_act::lost_hp_to_fake_hp::state(&managers, 20)
            .unwrap()
            .unwrap()
            .current,
        2_071
    );

    for (rate, expected) in [(350, 724), (500, 1_035)] {
        let behavior = ParsedBehavior::from_spec(
            BehaviorSpec::new(60309, "FakeHpToHeal"),
            vec![rate, 0],
            Vec::new(),
        );
        let ops = super::super::rule_ops(
            BehaviorOpContext {
                source_uid: 10,
                source_team: 1,
                target_uid: 20,
                active_skill_id: 31450151,
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
        .expect("configured FakeHpToHeal rate should be supported");
        assert!(matches!(
            ops.first(),
            Some(RuleOp::Command(BattleCommand::Hp(HpCommand::Heal(heal))))
                if heal.amount == expected
        ));
    }
}
