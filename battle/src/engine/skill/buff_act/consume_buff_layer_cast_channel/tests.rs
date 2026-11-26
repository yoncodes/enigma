use super::*;
use crate::engine::skill::{rule::DefinitionKey, subscriber::BuffActSubscriber};

const FEATURE: &str = "31350131#202#0,31350141#10,31350142#20,31350143,31350141#30,31350143,31350142#40,31350143,31350143,31350141#50,31350143,31350143,31350142#60,31350143,31350143,31350143";

#[test]
fn structured_tiers_preserve_thresholds_and_skill_lists() {
    let parsed = parse(&FEATURE.split('#').map(str::to_owned).collect::<Vec<_>>()).unwrap();
    let spec = decode(&parsed).unwrap();

    assert_eq!(spec.resource_buff_id, 31350131);
    assert_eq!(spec.tiers.len(), 7);
    assert_eq!(spec.tiers[3].threshold, 30);
    assert_eq!(spec.tiers[3].skills, [31350143, 31350142]);
    assert_eq!(spec.tiers[6].skills, [31350143, 31350143, 31350143]);

    let mut unsupported = FEATURE.split('#').map(str::to_owned).collect::<Vec<_>>();
    unsupported[1] = "203".to_owned();
    assert!(parse(&unsupported).is_none());
}

#[test]
fn round_end_spends_the_selected_tier_and_ends_channel() {
    crate::test_support::init_config();
    let fight = sonettobuf::Fight {
        attacker: Some(sonettobuf::FightTeam {
            entitys: vec![sonettobuf::FightEntityInfo {
                uid: Some(10),
                current_hp: Some(1_000),
                buffs: vec![sonettobuf::BuffInfo {
                    uid: Some(1),
                    buff_id: Some(31350131),
                    layer: Some(35),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let managers = BattleManagers::seeded(&fight);
    let subscriber = BuffActSubscriber {
        owner_uid: 10,
        source_uid: 10,
        buff_uid: 2,
        buff_id: 31350121,
        team_type: 1,
        owner_alive: true,
        amount: 1,
        key: crate::engine::event::subscription::SubscriptionKey::new(
            EventKind::SmallRoundEnd,
            DefinitionKey::new(1054, "ConsumeBuffLayerCastChannel"),
        ),
        act_type: "ConsumeBuffLayerCastChannel".to_owned(),
        effect_time: 301,
        effect_condition: 0,
        args: parse(&FEATURE.split('#').map(str::to_owned).collect::<Vec<_>>()).unwrap(),
        raw: format!("1054#{FEATURE}"),
    };

    let ops = rule_ops(
        &managers,
        &SkillEffectCatalog::from_game_db(config::configs::get()),
        &subscriber,
        &BattleEvent::Kind(EventKind::SmallRoundEnd),
    )
    .unwrap();

    assert!(matches!(
        ops.as_slice(),
        [
            RuleOp::Command(BattleCommand::Buff(BuffCommand::Consume(BuffConsume {
                amount: 30,
                ..
            }))),
            RuleOp::Skill(SkillInvocation {
                plan: SkillRequest {
                    skill_id: 31350143,
                    ..
                },
                ..
            }),
            RuleOp::Skill(SkillInvocation {
                plan: SkillRequest {
                    skill_id: 31350142,
                    ..
                },
                ..
            }),
            RuleOp::Command(BattleCommand::Buff(BuffCommand::Remove(BuffRemove {
                selector: BuffRemoveSelector::Uid(2),
                ..
            })))
        ]
    ));
}
