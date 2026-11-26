use sonettobuf::{BuffInfo, Fight, FightEntityInfo, FightTeam, HeroAttribute};

use crate::engine::{
    event::{kind::EventKind, payload::BattleEvent},
    manager::{
        BattleManagers,
        card::CARD_PLAY_ORIGIN,
        ex_point::{ExPointChange, ExPointCommand, ExPointKind},
    },
    runtime::{determinism::RoundDeterminism, drain},
    skill::{
        action::{ActionEvent, SkillLifecycle, SkillRequest},
        effect::SkillEffectCatalog,
        rule::output::RuleOp,
        target::{TargetContext, TargetPool},
    },
};

#[test]
fn tidal_tempo_settles_paid_ultimate_moxie_for_the_current_round() {
    crate::test_support::init_config();
    let buff = |uid, buff_id, layer, duration| BuffInfo {
        uid: Some(uid),
        buff_id: Some(buff_id),
        from_uid: Some(10),
        layer,
        duration,
        ..Default::default()
    };
    let fight = Fight {
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: vec![
                FightEntityInfo {
                    uid: Some(10),
                    team_type: Some(1),
                    current_hp: Some(1_000),
                    ex_point: Some(5),
                    attr: Some(HeroAttribute {
                        attack: Some(1_000),
                        hp: Some(1_000),
                        ..Default::default()
                    }),
                    buffs: vec![
                        buff(20, 437625, None, Some(2)),
                        buff(25, 5081, None, Some(2)),
                    ],
                    ..Default::default()
                },
                FightEntityInfo {
                    uid: Some(11),
                    team_type: Some(1),
                    current_hp: Some(1_000),
                    ex_point: Some(8),
                    ex_point_max: Some(8),
                    ex_point_type: Some(ExPointKind::Faith.as_wire()),
                    ..Default::default()
                },
            ],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                team_type: Some(2),
                current_hp: Some(10_000),
                ex_point: Some(5),
                attr: Some(HeroAttribute {
                    hp: Some(10_000),
                    ..Default::default()
                }),
                buffs: vec![
                    buff(21, 31570012, None, None),
                    buff(22, 31570010, Some(1), Some(1)),
                    buff(23, 31570010, Some(1), Some(2)),
                    buff(24, 31574010, Some(1), Some(2)),
                ],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let catalog = SkillEffectCatalog::from_fight(config::configs::get(), &fight);
    let mut managers = BattleManagers::seeded(&fight);
    let mut determinism = RoundDeterminism::default();
    let mut run = |managers: &mut BattleManagers, ops| {
        drain::run_command_group(
            managers,
            &pool,
            &catalog,
            &mut determinism,
            TargetContext::default(),
            ops,
        )
        .unwrap()
    };
    let change = |uid, delta| ExPointChange {
        origin: CARD_PLAY_ORIGIN,
        source_uid: uid,
        target_uid: uid,
        delta,
        config_effect: 0,
        effect_type: 0,
    };
    let action = |source_uid, skill_id| SkillRequest {
        source_uid,
        skill_id,
    };
    run(
        &mut managers,
        vec![RuleOp::DebitUltimate {
            action: action(10, 31571131),
            cost: ExPointCommand::Change(change(10, -5)),
        }],
    );
    assert_eq!(managers.ex_point.get(10), 5);
    assert_eq!(managers.ex_point.round_ultimate_spend(1), 0);

    run(
        &mut managers,
        vec![
            RuleOp::PaySkillAction {
                action: action(10, 31571131),
                cost: ExPointCommand::Spend(change(10, -8)),
            },
            RuleOp::PaySkillAction {
                action: action(-1, 31571131),
                cost: ExPointCommand::Spend(change(-1, -5)),
            },
            RuleOp::PaySkillAction {
                action: action(11, 31200133),
                cost: ExPointCommand::Spend(change(11, -8)),
            },
            RuleOp::SkillLifecycle(SkillLifecycle::ActionCompleted(ActionEvent {
                source_uid: 10,
                skill_id: 31571131,
                moxie_spent: 5,
                ..Default::default()
            })),
        ],
    );
    assert_eq!(managers.ex_point.get(11), 0);
    assert_eq!(managers.ex_point.round_ultimate_spend(1), 5);
    assert_eq!(managers.ex_point.round_ultimate_spend(2), 5);
    let settlement = || vec![RuleOp::Publish(BattleEvent::Kind(EventKind::RoundEnd))];
    run(&mut managers, settlement());
    assert_eq!(managers.hp.current(-1), 8_260);

    managers.begin_reaction_round();
    run(&mut managers, settlement());
    assert_eq!(managers.hp.current(-1), 8_260);
}
